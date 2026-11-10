//! Guarda-roupa de cosméticos (skins e capas).
//!
//! - **Biblioteca local**: PNGs validados em `cosmetics/`, equipados por conta.
//! - **Contas offline**: aplicados via CustomSkinLoader (LocalSkin) em cada instância.
//! - **Contas Microsoft**: upload de skin e troca de capa direto na API oficial da Mojang.
//! - **Servidor de cosméticos** (crate `cosmetics-server`): sincroniza skin/capa para que
//!   todos os jogadores do launcher vejam os cosméticos uns dos outros pelo CSL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::auth::accounts::{Account, AccountKind};
use crate::auth::microsoft::now_secs;
use crate::auth::secure_store;
use crate::error::{LauncherError, Result};
use crate::http::{check_status, with_retry};
use crate::paths::{write_atomic, LauncherPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CosmeticKind {
    Skin,
    Cape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SkinModel {
    #[default]
    Classic,
    Slim,
}

impl SkinModel {
    fn mojang(self) -> &'static str {
        match self {
            SkinModel::Classic => "classic",
            SkinModel::Slim => "slim",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cosmetic {
    pub id: String,
    pub kind: CosmeticKind,
    pub name: String,
    #[serde(default)]
    pub model: SkinModel,
    pub sha256: String,
    pub added_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Equipped {
    pub skin: Option<String>,
    pub cape: Option<String>,
    /// Itens 3D (mod Aurora Cosmetics), no máximo um por slot.
    #[serde(default)]
    pub items3d: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wardrobe {
    pub items: Vec<Cosmetic>,
    pub equipped: HashMap<String, Equipped>,
}

fn root(paths: &LauncherPaths) -> PathBuf {
    paths.root().join("cosmetics")
}

fn file_of(paths: &LauncherPaths, c: &Cosmetic) -> PathBuf {
    root(paths).join(format!("{}.png", c.id))
}

pub async fn load(paths: &LauncherPaths) -> Wardrobe {
    match tokio::fs::read(root(paths).join("wardrobe.json")).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => Wardrobe::default(),
    }
}

async fn save(paths: &LauncherPaths, w: &Wardrobe) -> Result<()> {
    write_atomic(&root(paths).join("wardrobe.json"), &serde_json::to_vec_pretty(w)?).await
}

/// Lê largura/altura do chunk IHDR do PNG.
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

pub fn validate_texture(kind: CosmeticKind, bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() > 512 * 1024 {
        return Err(LauncherError::Other("a imagem passa de 512 KB".into()));
    }
    let (w, h) = png_dimensions(bytes).ok_or_else(|| LauncherError::Other("o arquivo não é um PNG válido".into()))?;
    let ok = match kind {
        // 64x64 (moderno), 64x32 (legado) e versões HD em potências de 2
        CosmeticKind::Skin => w.is_power_of_two() && (64..=1024).contains(&w) && (h == w || h * 2 == w),
        // Capas: proporção 2:1 (64x32, 128x64…) ou o formato antigo 22x17 escalado
        CosmeticKind::Cape => (w == h * 2 && (64..=2048).contains(&w)) || (w * 17 == h * 22 && w >= 22),
    };
    if !ok {
        let expected = if kind == CosmeticKind::Skin { "64x64 ou 64x32" } else { "64x32 (ou múltiplos)" };
        return Err(LauncherError::Other(format!("tamanho {w}x{h} inválido; o esperado é {expected}")));
    }
    Ok((w, h))
}

pub async fn import(
    paths: &LauncherPaths,
    source: &Path,
    kind: CosmeticKind,
    name: &str,
    model: Option<SkinModel>,
) -> Result<Cosmetic> {
    let bytes = tokio::fs::read(source).await?;
    import_bytes(paths, &bytes, kind, name, model).await
}

pub async fn import_bytes(
    paths: &LauncherPaths,
    bytes: &[u8],
    kind: CosmeticKind,
    name: &str,
    model: Option<SkinModel>,
) -> Result<Cosmetic> {
    validate_texture(kind, bytes)?;
    let sha256 = hex::encode(Sha256::digest(bytes));
    let mut w = load(paths).await;
    if let Some(existing) = w.items.iter().find(|c| c.sha256 == sha256 && c.kind == kind) {
        return Ok(existing.clone());
    }
    let item = Cosmetic {
        id: uuid::Uuid::new_v4().simple().to_string(),
        kind,
        name: if name.trim().is_empty() { "Sem nome".into() } else { name.trim().chars().take(40).collect() },
        model: model.unwrap_or_default(),
        sha256,
        added_at: now_secs(),
    };
    write_atomic(&file_of(paths, &item), bytes).await?;
    w.items.push(item.clone());
    save(paths, &w).await?;
    Ok(item)
}

/// Importa um PNG em base64 (skins criadas no editor ou cosméticos gerados pela UI).
pub async fn import_base64(
    paths: &LauncherPaths,
    png_base64: &str,
    kind: CosmeticKind,
    name: &str,
    model: Option<SkinModel>,
) -> Result<Cosmetic> {
    let raw = png_base64.split_once("base64,").map(|(_, b)| b).unwrap_or(png_base64);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw.trim())
        .map_err(|_| LauncherError::Other("imagem em base64 inválida".into()))?;
    import_bytes(paths, &bytes, kind, name, model).await
}

pub async fn remove(paths: &LauncherPaths, id: &str) -> Result<Wardrobe> {
    let mut w = load(paths).await;
    if let Some(item) = w.items.iter().find(|c| c.id == id).cloned() {
        let _ = tokio::fs::remove_file(file_of(paths, &item)).await;
    }
    w.items.retain(|c| c.id != id);
    for eq in w.equipped.values_mut() {
        if eq.skin.as_deref() == Some(id) {
            eq.skin = None;
        }
        if eq.cape.as_deref() == Some(id) {
            eq.cape = None;
        }
    }
    save(paths, &w).await?;
    Ok(w)
}

pub async fn set_model(paths: &LauncherPaths, id: &str, model: SkinModel) -> Result<Wardrobe> {
    let mut w = load(paths).await;
    if let Some(item) = w.items.iter_mut().find(|c| c.id == id) {
        item.model = model;
    }
    save(paths, &w).await?;
    Ok(w)
}

pub async fn equip(paths: &LauncherPaths, account_id: &str, kind: CosmeticKind, id: Option<&str>) -> Result<Wardrobe> {
    let mut w = load(paths).await;
    if let Some(id) = id {
        if !w.items.iter().any(|c| c.id == id && c.kind == kind) {
            return Err(LauncherError::Other("cosmético não encontrado".into()));
        }
    }
    let eq = w.equipped.entry(account_id.to_owned()).or_default();
    match kind {
        CosmeticKind::Skin => eq.skin = id.map(str::to_owned),
        CosmeticKind::Cape => eq.cape = id.map(str::to_owned),
    }
    save(paths, &w).await?;
    Ok(w)
}

/// Define os itens 3D equipados (ids do cosmetics.json do mod).
pub async fn set_items3d(paths: &LauncherPaths, account_id: &str, items: Vec<String>) -> Result<Wardrobe> {
    let mut w = load(paths).await;
    let clean: Vec<String> = items
        .into_iter()
        .filter(|i| {
            (1..=32).contains(&i.len()) && i.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
        .take(8)
        .collect();
    w.equipped.entry(account_id.to_owned()).or_default().items3d = clean;
    save(paths, &w).await?;
    Ok(w)
}

/// Versão do Minecraft para a qual o mod Aurora Cosmetics é compilado.
pub const COSMETICS_MOD_MC: &str = "26.3";

pub fn bundled_mod_path(paths: &LauncherPaths) -> PathBuf {
    paths.root().join("bundled-mods").join("aurora-cosmetics.jar")
}

/// Copia o mod de cosméticos 3D (gerado por compilar-mod.bat) para a instância, se compatível.
/// Retorna `Ok(true)` quando o mod está presente na instância.
pub async fn install_cosmetics_mod(
    paths: &LauncherPaths,
    game_dir: &Path,
    loader_is_fabric: bool,
    mc_version: &str,
) -> Result<bool> {
    if !loader_is_fabric || mc_version != COSMETICS_MOD_MC {
        return Ok(false);
    }
    let Ok(bytes) = tokio::fs::read(bundled_mod_path(paths)).await else { return Ok(false) };
    let target = game_dir.join("mods").join("aurora-cosmetics.jar");
    if tokio::fs::read(&target).await.ok().as_deref() != Some(bytes.as_slice()) {
        write_atomic(&target, &bytes).await?;
    }
    Ok(true)
}

/// Escreve config/aurora-cosmetics.json, lido pelo mod (seu jogador + servidor para os outros).
pub async fn write_mod_config(game_dir: &Path, server: Option<&str>, username: &str, items3d: &[String]) -> Result<()> {
    let body = json!({
        "server": server.map(|s| s.trim().trim_end_matches('/')).filter(|s| !s.is_empty()),
        "players": { username: items3d },
    });
    write_atomic(&game_dir.join("config").join("aurora-cosmetics.json"), &serde_json::to_vec_pretty(&body)?).await
}

/// PNG como data URL (para o preview 3D na UI).
pub async fn data_url(paths: &LauncherPaths, id: &str) -> Result<String> {
    let w = load(paths).await;
    let item =
        w.items.iter().find(|c| c.id == id).ok_or_else(|| LauncherError::Other("cosmético não encontrado".into()))?;
    let bytes = tokio::fs::read(file_of(paths, item)).await?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn equipped_items<'a>(w: &'a Wardrobe, account_id: &str) -> (Option<&'a Cosmetic>, Option<&'a Cosmetic>) {
    let Some(eq) = w.equipped.get(account_id) else { return (None, None) };
    let find = |id: &Option<String>| id.as_ref().and_then(|id| w.items.iter().find(|c| &c.id == id));
    (find(&eq.skin), find(&eq.cape))
}

/// Copia skin/capa equipadas para `CustomSkinLoader/LocalSkin/` da instância (contas offline).
/// A capa também vale como textura da élitra.
pub async fn apply_to_instance(paths: &LauncherPaths, account: &Account, game_dir: &Path) -> Result<()> {
    let w = load(paths).await;
    let (skin, cape) = equipped_items(&w, &account.id);
    let base = game_dir.join("CustomSkinLoader").join("LocalSkin");
    let name = format!("{}.png", account.username);
    for (sub, item) in [("skins", skin), ("capes", cape), ("elytras", cape)] {
        let target = base.join(sub).join(&name);
        match item {
            Some(c) => {
                let bytes = tokio::fs::read(file_of(paths, c)).await?;
                write_atomic(&target, &bytes).await?;
            }
            None => {
                let _ = tokio::fs::remove_file(&target).await;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- Mojang (contas originais)

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MojangCape {
    pub id: String,
    pub state: String,
    pub url: String,
    #[serde(default)]
    pub alias: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProfileCapes {
    #[serde(default)]
    capes: Vec<MojangCape>,
}

const PROFILE: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Envia uma skin para a conta Microsoft (aparece em qualquer servidor original).
pub async fn upload_skin_mojang(client: &Client, token: &str, png: Vec<u8>, model: SkinModel) -> Result<()> {
    validate_texture(CosmeticKind::Skin, &png)?;
    let png = &png;
    with_retry(2, || async move {
        let part = reqwest::multipart::Part::bytes(png.clone()).file_name("skin.png").mime_str("image/png")?;
        let form = reqwest::multipart::Form::new().text("variant", model.mojang()).part("file", part);
        let r = client.post(format!("{PROFILE}/skins")).bearer_auth(token).multipart(form).send().await?;
        if r.status().as_u16() == 400 {
            return Err(LauncherError::Other(format!(
                "a Mojang recusou a skin: {}",
                r.text().await.unwrap_or_default()
            )));
        }
        check_status(r)?;
        Ok(())
    })
    .await
}

pub async fn mojang_capes(client: &Client, token: &str) -> Result<Vec<MojangCape>> {
    with_retry(3, || async move {
        let r = client.get(PROFILE).bearer_auth(token).send().await?;
        Ok(check_status(r)?.json::<ProfileCapes>().await?.capes)
    })
    .await
}

/// Ativa uma capa oficial que a conta possui (`None` esconde a capa).
pub async fn set_mojang_cape(client: &Client, token: &str, cape_id: Option<&str>) -> Result<()> {
    let url = format!("{PROFILE}/capes/active");
    let r = match cape_id {
        Some(id) => client.put(&url).bearer_auth(token).json(&json!({ "capeId": id })).send().await?,
        None => client.delete(&url).bearer_auth(token).send().await?,
    };
    check_status(r)?;
    Ok(())
}

// ---------------------------------------------------------------- servidor de cosméticos

fn server_key(server: &str, username: &str) -> String {
    let host = server.trim_end_matches('/').rsplit("//").next().unwrap_or(server);
    format!("cos:{host}:{}", username.to_ascii_lowercase())
}

#[derive(Deserialize)]
struct RegisterResponse {
    token: String,
}

async fn server_token(client: &Client, server: &str, username: &str) -> Result<String> {
    let key = server_key(server, username);
    let k = key.clone();
    if let Some(t) = tokio::task::spawn_blocking(move || secure_store::load_secret(&k)).await?? {
        return Ok(t);
    }
    let r = client
        .post(format!("{}/api/register", server.trim_end_matches('/')))
        .json(&json!({ "username": username }))
        .send()
        .await?;
    if r.status().as_u16() == 409 {
        return Err(LauncherError::Other(format!(
            "o nick {username} já foi registrado neste servidor de cosméticos por outra pessoa"
        )));
    }
    let token = check_status(r)?.json::<RegisterResponse>().await?.token;
    let t = token.clone();
    tokio::task::spawn_blocking(move || secure_store::store_secret(&key, &t)).await??;
    Ok(token)
}

/// Publica skin + capa equipadas no servidor de cosméticos.
pub async fn sync_to_server(client: &Client, paths: &LauncherPaths, server: &str, account: &Account) -> Result<()> {
    let w = load(paths).await;
    let (skin, cape) = equipped_items(&w, &account.id);
    let enc = |b: Vec<u8>| base64::engine::general_purpose::STANDARD.encode(b);
    let skin_b64 = match skin {
        Some(c) => Some(enc(tokio::fs::read(file_of(paths, c)).await?)),
        None => None,
    };
    let cape_b64 = match cape {
        Some(c) => Some(enc(tokio::fs::read(file_of(paths, c)).await?)),
        None => None,
    };
    let token = server_token(client, server, &account.username).await?;
    let body = json!({
        "username": account.username,
        "skin": skin_b64,
        "model": skin.map(|s| s.model).unwrap_or_default(),
        "cape": cape_b64,
        "items3d": w.equipped.get(&account.id).map(|e| e.items3d.clone()).unwrap_or_default(),
    });
    let r = client
        .put(format!("{}/api/profile", server.trim_end_matches('/')))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;
    check_status(r)?;
    Ok(())
}

// ---------------------------------------------------------------- galeria da comunidade

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteGalleryItem {
    id: String,
    name: String,
    author: String,
    hash: String,
    slim: bool,
    downloads: u64,
}

#[derive(Debug, Deserialize)]
struct RemoteGalleryPage {
    items: Vec<RemoteGalleryItem>,
    total: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItem {
    pub id: String,
    pub name: String,
    pub author: String,
    pub model: SkinModel,
    pub downloads: u64,
    /// PNG como data URL (evita depender do domínio do servidor na CSP da UI).
    pub texture: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryPage {
    pub items: Vec<GalleryItem>,
    pub total: u64,
}

fn base_url(server: &str) -> String {
    server.trim().trim_end_matches('/').to_owned()
}

/// Baixa (com cache local por hash) a textura de um item da galeria.
async fn texture_bytes(client: &Client, paths: &LauncherPaths, server: &str, hash: &str) -> Result<Vec<u8>> {
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(LauncherError::Other("hash de textura inválido".into()));
    }
    let cache = root(paths).join("gallery-cache").join(format!("{hash}.png"));
    if let Ok(b) = tokio::fs::read(&cache).await {
        if hex::encode(Sha256::digest(&b)) == hash {
            return Ok(b);
        }
    }
    let url = format!("{}/csl/textures/{hash}", base_url(server));
    let bytes = check_status(client.get(&url).send().await?)?.bytes().await?.to_vec();
    if hex::encode(Sha256::digest(&bytes)) != hash {
        return Err(LauncherError::HashMismatch { path: url, expected: hash.into(), actual: "diferente".into() });
    }
    write_atomic(&cache, &bytes).await?;
    Ok(bytes)
}

pub async fn gallery_list(
    client: &Client,
    paths: &LauncherPaths,
    server: &str,
    query: &str,
    offset: u32,
) -> Result<GalleryPage> {
    let url = format!("{}/api/gallery?q={}&offset={offset}&limit=24", base_url(server), urlencoding::encode(query));
    let page: RemoteGalleryPage = check_status(client.get(&url).send().await?)?.json().await?;
    let futures: Vec<_> = page
        .items
        .into_iter()
        .map(|item| {
            let (client, paths, server) = (client.clone(), paths.clone(), server.to_owned());
            async move {
                let bytes = texture_bytes(&client, &paths, &server, &item.hash).await.ok()?;
                Some(GalleryItem {
                    id: item.id,
                    name: item.name,
                    author: item.author,
                    model: if item.slim { SkinModel::Slim } else { SkinModel::Classic },
                    downloads: item.downloads,
                    texture: format!(
                        "data:image/png;base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(bytes)
                    ),
                })
            }
        })
        .collect();
    let items = futures::future::join_all(futures).await.into_iter().flatten().collect();
    Ok(GalleryPage { items, total: page.total })
}

/// Copia uma skin da galeria para o guarda-roupa local.
pub async fn gallery_import(client: &Client, paths: &LauncherPaths, server: &str, id: &str) -> Result<Cosmetic> {
    let url = format!("{}/api/gallery/{}/use", base_url(server), urlencoding::encode(id));
    let item: RemoteGalleryItem = check_status(client.post(&url).send().await?)?.json().await?;
    let bytes = texture_bytes(client, paths, server, &item.hash).await?;
    let model = if item.slim { SkinModel::Slim } else { SkinModel::Classic };
    import_bytes(paths, &bytes, CosmeticKind::Skin, &format!("{} (de {})", item.name, item.author), Some(model)).await
}

/// Compartilha uma skin do guarda-roupa na galeria pública do servidor.
pub async fn gallery_publish(
    client: &Client,
    paths: &LauncherPaths,
    server: &str,
    account: &Account,
    cosmetic_id: &str,
    name: &str,
) -> Result<()> {
    let w = load(paths).await;
    let item = w
        .items
        .iter()
        .find(|c| c.id == cosmetic_id && c.kind == CosmeticKind::Skin)
        .ok_or_else(|| LauncherError::Other("skin não encontrada".into()))?;
    let bytes = tokio::fs::read(file_of(paths, item)).await?;
    let token = server_token(client, server, &account.username).await?;
    let body = json!({
        "username": account.username,
        "name": if name.trim().is_empty() { item.name.as_str() } else { name },
        "skin": base64::engine::general_purpose::STANDARD.encode(bytes),
        "model": item.model,
    });
    let r = client.post(format!("{}/api/gallery", base_url(server))).bearer_auth(token).json(&body).send().await?;
    if r.status().as_u16() == 429 {
        return Err(LauncherError::Other("você já compartilhou o máximo de skins permitido".into()));
    }
    check_status(r)?;
    Ok(())
}

/// Aplica tudo o que cabe para a conta antes de abrir o jogo. Falhas viram avisos.
pub async fn prepare_for_launch(
    client: &Client,
    paths: &LauncherPaths,
    account: &Account,
    game_dir: &Path,
    server: Option<&str>,
    loader_is_fabric: bool,
    mc_version: &str,
) -> Vec<String> {
    let mut warnings = Vec::new();
    match install_cosmetics_mod(paths, game_dir, loader_is_fabric, mc_version).await {
        Ok(true) => {
            let w = load(paths).await;
            let items = w.equipped.get(&account.id).map(|e| e.items3d.clone()).unwrap_or_default();
            if let Err(e) = write_mod_config(game_dir, server, &account.username, &items).await {
                warnings.push(format!("config dos cosméticos 3D: {e}"));
            }
        }
        Ok(false) => {}
        Err(e) => warnings.push(format!("mod de cosméticos 3D: {e}")),
    }
    if account.kind == AccountKind::Offline {
        if let Err(e) = apply_to_instance(paths, account, game_dir).await {
            warnings.push(format!("cosméticos locais: {e}"));
        }
    }
    if let Some(server) = server.filter(|s| !s.trim().is_empty()) {
        if let Err(e) = sync_to_server(client, paths, server, account).await {
            warnings.push(format!("servidor de cosméticos: {e}"));
        }
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&w.to_be_bytes());
        v.extend_from_slice(&h.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0]);
        v
    }

    #[test]
    fn validates_sizes() {
        assert!(validate_texture(CosmeticKind::Skin, &png(64, 64)).is_ok());
        assert!(validate_texture(CosmeticKind::Skin, &png(64, 32)).is_ok());
        assert!(validate_texture(CosmeticKind::Skin, &png(128, 128)).is_ok());
        assert!(validate_texture(CosmeticKind::Skin, &png(100, 100)).is_err());
        assert!(validate_texture(CosmeticKind::Cape, &png(64, 32)).is_ok());
        assert!(validate_texture(CosmeticKind::Cape, &png(22, 17)).is_ok());
        assert!(validate_texture(CosmeticKind::Cape, &png(64, 64)).is_err());
        assert!(validate_texture(CosmeticKind::Skin, b"nope").is_err());
    }

    #[tokio::test]
    async fn import_equip_apply() {
        let dir = std::env::temp_dir().join(format!("cos-{}", uuid::Uuid::new_v4().simple()));
        let paths = LauncherPaths::new(&dir);
        let skin = import_bytes(&paths, &png(64, 64), CosmeticKind::Skin, "Minha skin", None).await.unwrap();
        let again = import_bytes(&paths, &png(64, 64), CosmeticKind::Skin, "dup", None).await.unwrap();
        assert_eq!(skin.id, again.id, "mesmo arquivo não duplica");
        let cape = import_bytes(&paths, &png(64, 32), CosmeticKind::Cape, "Capa", None).await.unwrap();
        equip(&paths, "acc", CosmeticKind::Skin, Some(&skin.id)).await.unwrap();
        equip(&paths, "acc", CosmeticKind::Cape, Some(&cape.id)).await.unwrap();
        assert!(equip(&paths, "acc", CosmeticKind::Cape, Some(&skin.id)).await.is_err(), "skin não vira capa");

        let account = Account {
            id: "acc".into(),
            username: "Lucas".into(),
            kind: AccountKind::Offline,
            skin_url: None,
            added_at: 0,
        };
        let game = dir.join("game");
        apply_to_instance(&paths, &account, &game).await.unwrap();
        for sub in ["skins", "capes", "elytras"] {
            assert!(game.join("CustomSkinLoader/LocalSkin").join(sub).join("Lucas.png").is_file());
        }
        remove(&paths, &cape.id).await.unwrap();
        apply_to_instance(&paths, &account, &game).await.unwrap();
        assert!(!game.join("CustomSkinLoader/LocalSkin/capes/Lucas.png").exists());
        tokio::fs::remove_dir_all(dir).await.ok();
    }
}
