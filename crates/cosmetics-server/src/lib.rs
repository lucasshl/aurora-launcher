//! Servidor de cosméticos do Aurora Launcher.
//!
//! Fala o protocolo **CustomSkinAPI**, que o CustomSkinLoader entende nativamente, então
//! qualquer jogador com o launcher vê skin e capa dos outros, inclusive em contas offline:
//!
//! ```text
//! GET  /csl/{nick}.json          → {"username","skins":{"default"|"slim":hash},"cape":hash}
//! GET  /csl/textures/{hash}      → PNG
//! POST /api/register {username}  → {"token"}  (primeiro a registrar o nick é o dono)
//! PUT  /api/profile  (Bearer)    → envia skin/capa em base64
//! GET  /health
//! GET  /api/gallery?q=&offset=&limit=   → galeria pública de skins compartilhadas
//! POST /api/gallery          (Bearer)   → compartilha uma skin {username,name,skin,model}
//! POST /api/gallery/{id}/use            → conta um download e devolve o item
//! DELETE /api/gallery/{id}   (Bearer)   → remove (só o autor)
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path as UrlPath, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub name: String,
    pub token_sha256: String,
    pub skin: Option<String>,
    #[serde(default)]
    pub slim: bool,
    pub cape: Option<String>,
    /// Cosméticos 3D do mod Aurora Cosmetics (ids do cosmetics.json).
    #[serde(default)]
    pub items3d: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItem {
    pub id: String,
    pub name: String,
    pub author: String,
    pub hash: String,
    pub slim: bool,
    pub downloads: u64,
    pub created_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Db {
    users: HashMap<String, UserRecord>,
    #[serde(default)]
    gallery: Vec<GalleryItem>,
}

const MAX_SHARED_PER_USER: usize = 50;

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct AppState {
    dir: PathBuf,
    db: RwLock<Db>,
}

impl AppState {
    pub async fn open(dir: impl Into<PathBuf>) -> std::io::Result<Arc<Self>> {
        let dir = dir.into();
        tokio::fs::create_dir_all(dir.join("textures")).await?;
        let db = match tokio::fs::read(dir.join("users.json")).await {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
            Err(_) => Db::default(),
        };
        Ok(Arc::new(Self { dir, db: RwLock::new(db) }))
    }

    async fn persist(&self, db: &Db) -> std::io::Result<()> {
        let tmp = self.dir.join("users.json.tmp");
        tokio::fs::write(&tmp, serde_json::to_vec_pretty(db)?).await?;
        tokio::fs::rename(&tmp, self.dir.join("users.json")).await
    }

    fn texture_path(&self, hash: &str) -> PathBuf {
        self.dir.join("textures").join(format!("{hash}.png"))
    }
}

type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/csl/{file}", get(csl_profile))
        .route("/csl/textures/{hash}", get(texture))
        .route("/api/register", post(register))
        .route("/api/profile", put(update_profile))
        .route("/api/gallery", get(gallery_list).post(gallery_publish))
        .route("/api/cosmetics3d/{name}", get(cosmetics3d))
        .route("/api/gallery/{id}/use", post(gallery_use))
        .route("/api/gallery/{id}", axum::routing::delete(gallery_delete))
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

pub fn valid_username(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn sha256_hex(data: &[u8]) -> String {
    hex::encode(Sha256::digest(data))
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    Some((u32::from_be_bytes(bytes[16..20].try_into().ok()?), u32::from_be_bytes(bytes[20..24].try_into().ok()?)))
}

fn valid_skin(b: &[u8]) -> bool {
    matches!(png_dimensions(b), Some((w, h)) if w.is_power_of_two() && (64..=1024).contains(&w) && (h == w || h * 2 == w))
}

fn valid_cape(b: &[u8]) -> bool {
    matches!(png_dimensions(b), Some((w, h)) if (w == h * 2 && (64..=2048).contains(&w)) || (w * 17 == h * 22 && w >= 22))
}

async fn csl_profile(State(s): State<Shared>, UrlPath(file): UrlPath<String>) -> Response {
    let Some(name) = file.strip_suffix(".json") else { return err(StatusCode::NOT_FOUND, "not found") };
    let db = s.db.read().await;
    let Some(u) = db.users.get(&name.to_ascii_lowercase()) else { return err(StatusCode::NOT_FOUND, "not found") };
    let mut skins = serde_json::Map::new();
    if let Some(h) = &u.skin {
        skins.insert(if u.slim { "slim" } else { "default" }.into(), h.clone().into());
    }
    let mut body = json!({ "username": u.name, "skins": skins });
    if let Some(c) = &u.cape {
        body["cape"] = c.clone().into();
    }
    Json(body).into_response()
}

async fn texture(State(s): State<Shared>, UrlPath(hash): UrlPath<String>) -> Response {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return err(StatusCode::BAD_REQUEST, "hash inválido");
    }
    match tokio::fs::read(s.texture_path(&hash)).await {
        Ok(bytes) => (
            [(header::CONTENT_TYPE, "image/png"), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")],
            bytes,
        )
            .into_response(),
        Err(_) => err(StatusCode::NOT_FOUND, "not found"),
    }
}

#[derive(Deserialize)]
struct RegisterReq {
    username: String,
}

async fn register(State(s): State<Shared>, Json(req): Json<RegisterReq>) -> Response {
    if !valid_username(&req.username) {
        return err(StatusCode::BAD_REQUEST, "nick inválido");
    }
    let key = req.username.to_ascii_lowercase();
    let mut db = s.db.write().await;
    if db.users.contains_key(&key) {
        return err(StatusCode::CONFLICT, "nick já registrado");
    }
    let token: [u8; 32] = rand::random();
    let token = hex::encode(token);
    db.users.insert(
        key,
        UserRecord {
            name: req.username,
            token_sha256: sha256_hex(token.as_bytes()),
            skin: None,
            slim: false,
            cape: None,
            items3d: Vec::new(),
        },
    );
    if let Err(e) = s.persist(&db).await {
        tracing::error!("falha ao gravar users.json: {e}");
        return err(StatusCode::INTERNAL_SERVER_ERROR, "erro de armazenamento");
    }
    Json(json!({ "token": token })).into_response()
}

#[derive(Deserialize)]
struct ProfileReq {
    username: String,
    skin: Option<String>,
    #[serde(default)]
    model: Option<String>,
    cape: Option<String>,
    #[serde(default)]
    items3d: Option<Vec<String>>,
}

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ")
}

async fn store_texture(s: &AppState, b64: &str, valid: fn(&[u8]) -> bool) -> Result<String, Response> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| err(StatusCode::BAD_REQUEST, "base64 inválido"))?;
    if bytes.len() > 512 * 1024 || !valid(&bytes) {
        return Err(err(StatusCode::BAD_REQUEST, "PNG com tamanho inválido"));
    }
    let hash = sha256_hex(&bytes);
    let path = s.texture_path(&hash);
    if !path.exists() {
        tokio::fs::write(&path, &bytes).await.map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "erro ao salvar"))?;
    }
    Ok(hash)
}

async fn update_profile(State(s): State<Shared>, headers: HeaderMap, Json(req): Json<ProfileReq>) -> Response {
    let Some(token) = bearer(&headers) else { return err(StatusCode::UNAUTHORIZED, "token ausente") };
    let key = req.username.to_ascii_lowercase();
    {
        let db = s.db.read().await;
        match db.users.get(&key) {
            Some(u) if u.token_sha256 == sha256_hex(token.as_bytes()) => {}
            Some(_) => return err(StatusCode::FORBIDDEN, "token não pertence a este nick"),
            None => return err(StatusCode::NOT_FOUND, "nick não registrado"),
        }
    }
    let skin = match &req.skin {
        Some(b) => match store_texture(&s, b, valid_skin).await {
            Ok(h) => Some(h),
            Err(r) => return r,
        },
        None => None,
    };
    let cape = match &req.cape {
        Some(b) => match store_texture(&s, b, valid_cape).await {
            Ok(h) => Some(h),
            Err(r) => return r,
        },
        None => None,
    };
    let mut db = s.db.write().await;
    if let Some(u) = db.users.get_mut(&key) {
        u.skin = skin;
        u.cape = cape;
        u.slim = req.model.as_deref() == Some("slim");
        if let Some(items) = &req.items3d {
            u.items3d = items
                .iter()
                .filter(|i| {
                    (1..=32).contains(&i.len())
                        && i.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                })
                .take(8)
                .cloned()
                .collect();
        }
    }
    if s.persist(&db).await.is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "erro de armazenamento");
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Itens 3D equipados por um jogador (consultado pelo mod dentro do jogo).
async fn cosmetics3d(State(s): State<Shared>, UrlPath(name): UrlPath<String>) -> Response {
    let db = s.db.read().await;
    let items = db.users.get(&name.to_ascii_lowercase()).map(|u| u.items3d.clone()).unwrap_or_default();
    ([(header::CACHE_CONTROL, "public, max-age=60")], Json(json!({ "items": items }))).into_response()
}

/// Valida o token Bearer para o nick; devolve a chave (nick em minúsculas).
async fn authorize(s: &AppState, headers: &HeaderMap, username: &str) -> Result<String, Response> {
    let token = bearer(headers).ok_or_else(|| err(StatusCode::UNAUTHORIZED, "token ausente"))?;
    let key = username.to_ascii_lowercase();
    let db = s.db.read().await;
    match db.users.get(&key) {
        Some(u) if u.token_sha256 == sha256_hex(token.as_bytes()) => Ok(key),
        Some(_) => Err(err(StatusCode::FORBIDDEN, "token não pertence a este nick")),
        None => Err(err(StatusCode::NOT_FOUND, "nick não registrado")),
    }
}

#[derive(Deserialize)]
struct GalleryQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    limit: Option<usize>,
}

async fn gallery_list(
    State(s): State<Shared>,
    axum::extract::Query(q): axum::extract::Query<GalleryQuery>,
) -> Response {
    let db = s.db.read().await;
    let needle = q.q.to_lowercase();
    let mut items: Vec<&GalleryItem> = db
        .gallery
        .iter()
        .filter(|i| {
            needle.is_empty() || i.name.to_lowercase().contains(&needle) || i.author.to_lowercase().contains(&needle)
        })
        .collect();
    // Mais usadas primeiro, depois as mais novas
    items.sort_by(|a, b| b.downloads.cmp(&a.downloads).then(b.created_at.cmp(&a.created_at)));
    let total = items.len();
    let limit = q.limit.unwrap_or(24).clamp(1, 60);
    let page: Vec<&GalleryItem> = items.into_iter().skip(q.offset).take(limit).collect();
    Json(json!({ "items": page, "total": total })).into_response()
}

#[derive(Deserialize)]
struct PublishReq {
    username: String,
    name: String,
    skin: String,
    #[serde(default)]
    model: Option<String>,
}

async fn gallery_publish(State(s): State<Shared>, headers: HeaderMap, Json(req): Json<PublishReq>) -> Response {
    let key = match authorize(&s, &headers, &req.username).await {
        Ok(k) => k,
        Err(r) => return r,
    };
    let name: String = req.name.trim().chars().take(40).collect();
    if name.is_empty() {
        return err(StatusCode::BAD_REQUEST, "dê um nome à skin");
    }
    let hash = match store_texture(&s, &req.skin, valid_skin).await {
        Ok(h) => h,
        Err(r) => return r,
    };
    let mut db = s.db.write().await;
    let author = db.users.get(&key).map(|u| u.name.clone()).unwrap_or(req.username.clone());
    if let Some(existing) = db.gallery.iter().find(|i| i.hash == hash) {
        return Json(existing.clone()).into_response();
    }
    if db.gallery.iter().filter(|i| i.author.eq_ignore_ascii_case(&author)).count() >= MAX_SHARED_PER_USER {
        return err(StatusCode::TOO_MANY_REQUESTS, "limite de skins compartilhadas atingido");
    }
    let item = GalleryItem {
        id: hex::encode(rand::random::<[u8; 8]>()),
        name,
        author,
        hash,
        slim: req.model.as_deref() == Some("slim"),
        downloads: 0,
        created_at: now_secs(),
    };
    db.gallery.push(item.clone());
    if s.persist(&db).await.is_err() {
        return err(StatusCode::INTERNAL_SERVER_ERROR, "erro de armazenamento");
    }
    (StatusCode::CREATED, Json(item)).into_response()
}

async fn gallery_use(State(s): State<Shared>, UrlPath(id): UrlPath<String>) -> Response {
    let mut db = s.db.write().await;
    let Some(item) = db.gallery.iter_mut().find(|i| i.id == id) else {
        return err(StatusCode::NOT_FOUND, "não encontrada");
    };
    item.downloads += 1;
    let item = item.clone();
    let _ = s.persist(&db).await;
    Json(item).into_response()
}

#[derive(Deserialize)]
struct DeleteQuery {
    username: String,
}

async fn gallery_delete(
    State(s): State<Shared>,
    headers: HeaderMap,
    UrlPath(id): UrlPath<String>,
    axum::extract::Query(q): axum::extract::Query<DeleteQuery>,
) -> Response {
    if let Err(r) = authorize(&s, &headers, &q.username).await {
        return r;
    }
    let mut db = s.db.write().await;
    let before = db.gallery.len();
    db.gallery.retain(|i| !(i.id == id && i.author.eq_ignore_ascii_case(&q.username)));
    if db.gallery.len() == before {
        return err(StatusCode::NOT_FOUND, "não encontrada ou não é sua");
    }
    let _ = s.persist(&db).await;
    StatusCode::NO_CONTENT.into_response()
}

pub fn data_dir_from_env() -> PathBuf {
    std::env::var_os("COSMETICS_DATA").map(PathBuf::from).unwrap_or_else(|| Path::new("data").to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn png(w: u32, h: u32) -> String {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        base64::engine::general_purpose::STANDARD.encode(v)
    }

    async fn call(app: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    fn json_req(method: &str, uri: &str, body: serde_json::Value, token: Option<&str>) -> Request<Body> {
        let mut b = Request::builder().method(method).uri(uri).header("content-type", "application/json");
        if let Some(t) = token {
            b = b.header("authorization", format!("Bearer {t}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn full_flow() {
        let dir = std::env::temp_dir().join(format!("cos-srv-{}", rand::random::<u64>()));
        let app = router(AppState::open(&dir).await.unwrap());

        let (st, body) = call(&app, json_req("POST", "/api/register", json!({"username":"Lucas"}), None)).await;
        assert_eq!(st, StatusCode::OK);
        let token = body["token"].as_str().unwrap().to_owned();

        let (st, _) = call(&app, json_req("POST", "/api/register", json!({"username":"lucas"}), None)).await;
        assert_eq!(st, StatusCode::CONFLICT, "nick é único sem diferenciar maiúsculas");

        let (st, _) = call(
            &app,
            json_req(
                "PUT",
                "/api/profile",
                json!({"username":"Lucas","skin":png(64,64),"model":"slim","cape":png(64,32)}),
                Some("errado"),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN);

        let (st, _) = call(
            &app,
            json_req(
                "PUT",
                "/api/profile",
                json!({"username":"Lucas","skin":png(64,64),"model":"slim","cape":png(64,32),"items3d":["asas_anjo","aureola","../hack"]}),
                Some(&token),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        let (_, c3d) = call(&app, Request::get("/api/cosmetics3d/lucas").body(Body::empty()).unwrap()).await;
        assert_eq!(c3d["items"], json!(["asas_anjo", "aureola"]), "ids inválidos são descartados");

        let (st, prof) = call(&app, Request::get("/csl/LUCAS.json").body(Body::empty()).unwrap()).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(prof["username"], "Lucas");
        let skin_hash = prof["skins"]["slim"].as_str().unwrap().to_owned();
        assert!(prof["cape"].is_string());

        let resp = app
            .clone()
            .oneshot(Request::get(format!("/csl/textures/{skin_hash}")).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["content-type"], "image/png");

        let (st, _) =
            call(&app, json_req("PUT", "/api/profile", json!({"username":"Lucas","skin":png(100,100)}), Some(&token)))
                .await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "PNG fora do padrão é recusado");

        let (st, _) = call(&app, Request::get("/csl/ninguem.json").body(Body::empty()).unwrap()).await;
        assert_eq!(st, StatusCode::NOT_FOUND);

        // Galeria: publicar, listar, buscar, usar, apagar
        let (st, item) = call(
            &app,
            json_req(
                "POST",
                "/api/gallery",
                json!({"username":"Lucas","name":"Cavaleiro","skin":png(64,64),"model":"classic"}),
                Some(&token),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::CREATED);
        let id = item["id"].as_str().unwrap().to_owned();
        let (_, list) = call(&app, Request::get("/api/gallery?q=cava").body(Body::empty()).unwrap()).await;
        assert_eq!(list["total"], 1);
        assert_eq!(list["items"][0]["author"], "Lucas");
        let (st, used) = call(&app, Request::post(format!("/api/gallery/{id}/use")).body(Body::empty()).unwrap()).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(used["downloads"], 1);
        let (st, _) = call(
            &app,
            Request::delete(format!("/api/gallery/{id}?username=Lucas"))
                .header("authorization", "Bearer errado")
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        let resp = app
            .clone()
            .oneshot(
                Request::delete(format!("/api/gallery/{id}?username=Lucas"))
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
        tokio::fs::remove_dir_all(dir).await.ok();
    }
}
