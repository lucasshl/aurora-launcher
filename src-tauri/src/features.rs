//! Comandos de mods/conteúdo, configs, otimização e cosméticos.

use std::path::PathBuf;

use launcher_core::configs::{self, ConfigFile};
use launcher_core::content::{self, InstallReport, InstalledContent, UpdateInfo};
use launcher_core::cosmetics::{self, CosmeticKind, MojangCape, SkinModel, Wardrobe};
use launcher_core::download::Downloader;
use launcher_core::instance::{load_instance, InstanceConfig};
use launcher_core::modrinth::{self, ContentKind, SearchResult, SortIndex, Version};
use launcher_core::optimize::{self, CuratedStatus, GraphicsPreset};
use launcher_core::{LauncherError, Result};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, TauriObserver};

async fn instance(state: &AppState, id: &str) -> Result<InstanceConfig> {
    load_instance(&state.launcher.paths, id).await
}

async fn downloader(state: &AppState) -> Downloader {
    Downloader::new(state.launcher.client.clone(), state.launcher.settings.read().await.download_concurrency)
}

fn ensure_not_running(state: &AppState, id: &str) -> Result<()> {
    if state.running.lock().unwrap().contains_key(id) {
        return Err(LauncherError::Instance("feche o jogo antes de alterar os mods".into()));
    }
    Ok(())
}

// ------------------------------------------------------------------ conteúdo (mods, packs, shaders)

#[tauri::command]
pub async fn list_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
) -> Result<Vec<InstalledContent>> {
    content::list_installed(&state.launcher.paths, &instance_id, kind).await
}

#[tauri::command]
pub async fn toggle_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    file_name: String,
    enabled: bool,
) -> Result<()> {
    ensure_not_running(&state, &instance_id)?;
    content::set_enabled(&state.launcher.paths, &instance_id, kind, &file_name, enabled).await
}

#[tauri::command]
pub async fn remove_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    file_name: String,
) -> Result<()> {
    ensure_not_running(&state, &instance_id)?;
    content::remove(&state.launcher.paths, &instance_id, kind, &file_name).await
}

#[tauri::command]
pub async fn search_content(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    query: String,
    sort: Option<SortIndex>,
    offset: Option<u32>,
) -> Result<SearchResult> {
    let inst = instance(&state, &instance_id).await?;
    modrinth::search(
        &state.launcher.client,
        &query,
        kind,
        inst.loader,
        &inst.minecraft_version,
        sort.unwrap_or_default(),
        offset.unwrap_or(0),
        20,
    )
    .await
}

#[tauri::command]
pub async fn content_versions(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    project: String,
) -> Result<Vec<Version>> {
    let inst = instance(&state, &instance_id).await?;
    modrinth::compatible_versions(&state.launcher.client, &project, &kind.loaders(inst.loader), &inst.minecraft_version)
        .await
}

#[tauri::command]
pub async fn install_content(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    projects: Vec<String>,
    version_id: Option<String>,
) -> Result<InstallReport> {
    ensure_not_running(&state, &instance_id)?;
    let inst = instance(&state, &instance_id).await?;
    let dl = downloader(&state).await;
    let observer = TauriObserver::new(app, instance_id);
    let pinned = match (&version_id, projects.first()) {
        (Some(v), Some(p)) => Some((p.as_str(), v.as_str())),
        _ => None,
    };
    content::install_projects(&state.launcher.paths, &dl, &inst, kind, &projects, pinned, observer.as_ref()).await
}

#[tauri::command]
pub async fn check_content_updates(
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
) -> Result<Vec<UpdateInfo>> {
    let inst = instance(&state, &instance_id).await?;
    content::check_updates(&state.launcher.client, &state.launcher.paths, &inst, kind).await
}

/// Atualiza os arquivos indicados (ou todos, se `file_names` vier vazio).
#[tauri::command]
pub async fn update_content(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
    file_names: Vec<String>,
) -> Result<usize> {
    ensure_not_running(&state, &instance_id)?;
    let inst = instance(&state, &instance_id).await?;
    let dl = downloader(&state).await;
    let observer = TauriObserver::new(app, instance_id);
    let updates = content::check_updates(&state.launcher.client, &state.launcher.paths, &inst, kind).await?;
    let mut done = 0;
    for u in updates.iter().filter(|u| file_names.is_empty() || file_names.contains(&u.file_name)) {
        content::apply_update(&state.launcher.paths, &dl, &inst, kind, u, observer.as_ref()).await?;
        done += 1;
    }
    Ok(done)
}

#[tauri::command]
pub async fn open_content_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    kind: ContentKind,
) -> Result<()> {
    let dir = content::content_dir(&state.launcher.paths, &instance_id, kind);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| LauncherError::Other(e.to_string()))
}

// ------------------------------------------------------------------ configs

#[tauri::command]
pub async fn list_configs(state: State<'_, AppState>, instance_id: String) -> Result<Vec<ConfigFile>> {
    configs::list_configs(&state.launcher.paths, &instance_id).await
}

#[tauri::command]
pub async fn read_config(state: State<'_, AppState>, instance_id: String, path: String) -> Result<String> {
    configs::read_config(&state.launcher.paths, &instance_id, &path).await
}

#[tauri::command]
pub async fn write_config(
    state: State<'_, AppState>,
    instance_id: String,
    path: String,
    content: String,
) -> Result<()> {
    configs::write_config(&state.launcher.paths, &instance_id, &path, &content).await
}

// ------------------------------------------------------------------ otimização

#[tauri::command]
pub async fn curated_mods(state: State<'_, AppState>, instance_id: String, pack: String) -> Result<Vec<CuratedStatus>> {
    let inst = instance(&state, &instance_id).await?;
    let list = if pack == "pvp" { optimize::PVP } else { optimize::PERFORMANCE };
    optimize::curated_status(&state.launcher.client, &state.launcher.paths, &inst, list).await
}

#[tauri::command]
pub async fn install_curated(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    keys: Vec<String>,
) -> Result<InstallReport> {
    ensure_not_running(&state, &instance_id)?;
    let inst = instance(&state, &instance_id).await?;
    let dl = downloader(&state).await;
    let observer = TauriObserver::new(app, instance_id);
    optimize::install_curated(&state.launcher.paths, &dl, &inst, &keys, observer.as_ref()).await
}

#[tauri::command]
pub async fn apply_graphics_preset(
    state: State<'_, AppState>,
    instance_id: String,
    preset: GraphicsPreset,
) -> Result<()> {
    let inst = instance(&state, &instance_id).await?;
    optimize::apply_graphics_preset(&state.launcher.paths.instance_game_dir(&inst.id), preset, &inst.minecraft_version)
        .await
}

// ------------------------------------------------------------------ cosméticos

#[tauri::command]
pub async fn get_wardrobe(state: State<'_, AppState>) -> Result<Wardrobe> {
    Ok(cosmetics::load(&state.launcher.paths).await)
}

#[tauri::command]
pub async fn import_cosmetic(
    state: State<'_, AppState>,
    path: PathBuf,
    kind: CosmeticKind,
    name: String,
    model: Option<SkinModel>,
) -> Result<Wardrobe> {
    cosmetics::import(&state.launcher.paths, &path, kind, &name, model).await?;
    Ok(cosmetics::load(&state.launcher.paths).await)
}

#[tauri::command]
pub async fn remove_cosmetic(state: State<'_, AppState>, id: String) -> Result<Wardrobe> {
    cosmetics::remove(&state.launcher.paths, &id).await
}

#[tauri::command]
pub async fn set_cosmetic_model(state: State<'_, AppState>, id: String, model: SkinModel) -> Result<Wardrobe> {
    cosmetics::set_model(&state.launcher.paths, &id, model).await
}

#[tauri::command]
pub async fn equip_cosmetic(
    state: State<'_, AppState>,
    account_id: String,
    kind: CosmeticKind,
    id: Option<String>,
) -> Result<Wardrobe> {
    cosmetics::equip(&state.launcher.paths, &account_id, kind, id.as_deref()).await
}

#[tauri::command]
pub async fn cosmetic_data_url(state: State<'_, AppState>, id: String) -> Result<String> {
    cosmetics::data_url(&state.launcher.paths, &id).await
}

async fn ms_token(state: &AppState, account_id: &str) -> Result<String> {
    let client_id = state.launcher.settings.read().await.client_id();
    let auth = state.launcher.accounts.launch_auth(&state.launcher.client, client_id.as_deref(), account_id).await?;
    if auth.is_offline {
        return Err(LauncherError::Auth("esta ação é só para contas Microsoft".into()));
    }
    Ok(auth.access_token)
}

#[tauri::command]
pub async fn upload_skin_to_mojang(state: State<'_, AppState>, account_id: String, cosmetic_id: String) -> Result<()> {
    let token = ms_token(&state, &account_id).await?;
    let w = cosmetics::load(&state.launcher.paths).await;
    let item = w
        .items
        .iter()
        .find(|c| c.id == cosmetic_id && c.kind == CosmeticKind::Skin)
        .ok_or_else(|| LauncherError::Other("skin não encontrada".into()))?;
    let bytes = tokio::fs::read(state.launcher.paths.root().join("cosmetics").join(format!("{}.png", item.id))).await?;
    cosmetics::upload_skin_mojang(&state.launcher.client, &token, bytes, item.model).await
}

#[tauri::command]
pub async fn mojang_capes(state: State<'_, AppState>, account_id: String) -> Result<Vec<MojangCape>> {
    let token = ms_token(&state, &account_id).await?;
    cosmetics::mojang_capes(&state.launcher.client, &token).await
}

#[tauri::command]
pub async fn set_mojang_cape(state: State<'_, AppState>, account_id: String, cape_id: Option<String>) -> Result<()> {
    let token = ms_token(&state, &account_id).await?;
    cosmetics::set_mojang_cape(&state.launcher.client, &token, cape_id.as_deref()).await
}

#[tauri::command]
pub async fn sync_cosmetics(state: State<'_, AppState>, account_id: String) -> Result<()> {
    let server = state
        .launcher
        .settings
        .read()
        .await
        .cosmetics_server
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| LauncherError::Other("configure o servidor de cosméticos em Configurações".into()))?;
    let account =
        state.launcher.accounts.get(&account_id).ok_or_else(|| LauncherError::Auth("conta não encontrada".into()))?;
    cosmetics::sync_to_server(&state.launcher.client, &state.launcher.paths, &server, &account).await
}

// ------------------------------------------------------------------ editor de skin e galeria

#[tauri::command]
pub async fn import_cosmetic_data(
    state: State<'_, AppState>,
    png_base64: String,
    kind: CosmeticKind,
    name: String,
    model: Option<SkinModel>,
) -> Result<cosmetics::Cosmetic> {
    cosmetics::import_base64(&state.launcher.paths, &png_base64, kind, &name, model).await
}

async fn cosmetics_server(state: &AppState) -> Result<String> {
    state
        .launcher
        .settings
        .read()
        .await
        .cosmetics_server
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| LauncherError::Other("configure o servidor de cosméticos em Configurações".into()))
}

#[tauri::command]
pub async fn gallery_list(state: State<'_, AppState>, query: String, offset: u32) -> Result<cosmetics::GalleryPage> {
    let server = cosmetics_server(&state).await?;
    cosmetics::gallery_list(&state.launcher.client, &state.launcher.paths, &server, &query, offset).await
}

#[tauri::command]
pub async fn gallery_import(state: State<'_, AppState>, id: String) -> Result<Wardrobe> {
    let server = cosmetics_server(&state).await?;
    cosmetics::gallery_import(&state.launcher.client, &state.launcher.paths, &server, &id).await?;
    Ok(cosmetics::load(&state.launcher.paths).await)
}

#[tauri::command]
pub async fn gallery_publish(
    state: State<'_, AppState>,
    account_id: String,
    cosmetic_id: String,
    name: String,
) -> Result<()> {
    let server = cosmetics_server(&state).await?;
    let account =
        state.launcher.accounts.get(&account_id).ok_or_else(|| LauncherError::Auth("conta não encontrada".into()))?;
    cosmetics::gallery_publish(&state.launcher.client, &state.launcher.paths, &server, &account, &cosmetic_id, &name)
        .await
}

#[tauri::command]
pub async fn set_items3d(state: State<'_, AppState>, account_id: String, items: Vec<String>) -> Result<Wardrobe> {
    cosmetics::set_items3d(&state.launcher.paths, &account_id, items).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CosmeticsModStatus {
    built: bool,
    path: String,
    minecraft_version: &'static str,
}

#[tauri::command]
pub async fn cosmetics_mod_status(state: State<'_, AppState>) -> Result<CosmeticsModStatus> {
    let path = cosmetics::bundled_mod_path(&state.launcher.paths);
    Ok(CosmeticsModStatus {
        built: tokio::fs::try_exists(&path).await.unwrap_or(false),
        path: path.display().to_string(),
        minecraft_version: cosmetics::COSMETICS_MOD_MC,
    })
}
