//! Comandos invocados pelo frontend via `invoke()`.

use std::path::PathBuf;

use launcher_core::auth::accounts::{Account, AccountsView};
use launcher_core::auth::microsoft::DeviceCode;
use launcher_core::instance::{self, InstanceConfig, NewInstance};
use launcher_core::launch::{self, LaunchStarted};
use launcher_core::loaders::fabric;
use launcher_core::settings::Settings;
use launcher_core::skins::csl;
use launcher_core::version::fetch_manifest;
use launcher_core::{LauncherError, Result};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::state::{AppState, TauriObserver};

// ------------------------------------------------------------------ settings

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings> {
    Ok(state.launcher.settings.read().await.clone())
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings> {
    settings.save(&state.launcher.paths).await?;
    *state.launcher.settings.write().await = settings.clone();
    Ok(settings)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    total_memory_mb: u64,
    data_dir: String,
    has_ms_client_id: bool,
    built_in_ms_client_id: bool,
}

#[tauri::command]
pub async fn system_info(state: State<'_, AppState>) -> Result<SystemInfo> {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    Ok(SystemInfo {
        total_memory_mb: sys.total_memory() / 1024 / 1024,
        data_dir: state.launcher.paths.root().display().to_string(),
        has_ms_client_id: state.launcher.settings.read().await.client_id().is_some(),
        built_in_ms_client_id: launcher_core::settings::BUILD_MS_CLIENT_ID.is_some_and(|s| !s.trim().is_empty()),
    })
}

// ------------------------------------------------------------------ contas

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<AccountsView> {
    Ok(state.launcher.accounts.list())
}

#[tauri::command]
pub async fn add_offline_account(state: State<'_, AppState>, username: String) -> Result<Account> {
    state.launcher.accounts.add_offline(&username).await
}

async fn client_id(state: &AppState) -> Result<String> {
    state.launcher.settings.read().await.client_id().ok_or_else(|| {
        LauncherError::Auth("defina o Client ID do Azure em Configurações para entrar com Microsoft".into())
    })
}

#[tauri::command]
pub async fn begin_microsoft_login(state: State<'_, AppState>) -> Result<DeviceCode> {
    let id = client_id(&state).await?;
    state.launcher.accounts.begin_microsoft(&state.launcher.client, &id).await
}

#[tauri::command]
pub async fn complete_microsoft_login(state: State<'_, AppState>, code: DeviceCode) -> Result<Account> {
    let id = client_id(&state).await?;
    state.launcher.accounts.finish_microsoft(&state.launcher.client, &id, &code).await
}

#[tauri::command]
pub async fn cancel_microsoft_login(state: State<'_, AppState>, device_code: String) -> Result<()> {
    state.launcher.accounts.cancel_microsoft(&device_code);
    Ok(())
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: String) -> Result<AccountsView> {
    state.launcher.accounts.remove(&id).await
}

#[tauri::command]
pub async fn set_active_account(state: State<'_, AppState>, id: String) -> Result<AccountsView> {
    state.launcher.accounts.set_active(&id).await
}

// ------------------------------------------------------------------ versões

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    id: String,
    kind: String,
    release_time: String,
}

#[tauri::command]
/// Todas as versões do manifest oficial, filtradas por tipo
/// (`release`, `snapshot`, `old_beta`, `old_alpha`). Lista vazia = todas.
pub async fn list_minecraft_versions(state: State<'_, AppState>, kinds: Vec<String>) -> Result<Vec<GameVersion>> {
    let manifest = fetch_manifest(&state.launcher.client, &state.launcher.paths).await?;
    Ok(manifest
        .versions
        .into_iter()
        .filter(|v| kinds.is_empty() || kinds.contains(&v.kind))
        .map(|v| GameVersion { id: v.id, kind: v.kind, release_time: v.release_time })
        .collect())
}

#[tauri::command]
pub async fn list_fabric_versions(state: State<'_, AppState>, minecraft_version: String) -> Result<Vec<String>> {
    Ok(fabric::loader_versions(&state.launcher.client, &minecraft_version)
        .await?
        .into_iter()
        .map(|l| l.version)
        .collect())
}

// ------------------------------------------------------------------ instâncias

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> Result<Vec<InstanceConfig>> {
    instance::list_instances(&state.launcher.paths).await
}

#[tauri::command]
pub async fn create_instance(state: State<'_, AppState>, request: NewInstance) -> Result<InstanceConfig> {
    let s = state.launcher.settings.read().await.clone();
    instance::create_instance(&state.launcher.paths, request, s.default_memory_min_mb, s.default_memory_max_mb).await
}

#[tauri::command]
pub async fn update_instance(state: State<'_, AppState>, config: InstanceConfig) -> Result<InstanceConfig> {
    // Preserva campos controlados pelo launcher
    let current = instance::load_instance(&state.launcher.paths, &config.id).await?;
    let mut merged = config;
    if merged.minecraft_version != current.minecraft_version
        || merged.loader != current.loader
        || merged.loader_version != current.loader_version
    {
        merged.version_id = None; // força reinstalar o perfil do loader
    }
    merged.created_at = current.created_at;
    merged.last_played = current.last_played;
    instance::save_instance(&state.launcher.paths, &merged).await?;
    Ok(merged)
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<()> {
    if state.running.lock().unwrap().contains_key(&id) {
        return Err(LauncherError::Instance("feche o jogo antes de excluir a instância".into()));
    }
    instance::delete_instance(&state.launcher.paths, &id).await
}

#[tauri::command]
pub async fn open_instance_folder(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    let dir = state.launcher.paths.instance_game_dir(&id);
    tokio::fs::create_dir_all(&dir).await?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| LauncherError::Other(e.to_string()))
}

#[tauri::command]
pub async fn set_offline_skin(
    state: State<'_, AppState>,
    instance_id: String,
    username: String,
    png_path: PathBuf,
) -> Result<()> {
    csl::set_local_skin(&state.launcher.paths, &instance_id, &username, &png_path).await
}

// ------------------------------------------------------------------ execução

/// Remove a instância do conjunto "preparando" mesmo se a preparação falhar.
struct PreparingGuard<'a> {
    state: &'a AppState,
    id: String,
}

impl Drop for PreparingGuard<'_> {
    fn drop(&mut self) {
        self.state.preparing.lock().unwrap().remove(&self.id);
    }
}

#[tauri::command]
pub async fn launch_instance(
    app: AppHandle,
    state: State<'_, AppState>,
    instance_id: String,
    account_id: String,
) -> Result<LaunchStarted> {
    {
        if state.running.lock().unwrap().contains_key(&instance_id) {
            return Err(LauncherError::Instance("esta instância já está aberta".into()));
        }
        if !state.preparing.lock().unwrap().insert(instance_id.clone()) {
            return Err(LauncherError::Instance("esta instância já está sendo preparada".into()));
        }
    }
    let _guard = PreparingGuard { state: &state, id: instance_id.clone() };

    let observer = TauriObserver::new(app, instance_id.clone());
    let (process, started) = launch::launch_instance(&state.launcher, &instance_id, &account_id, observer).await?;
    // O jogo pode ter fechado antes desta linha (crash imediato): não registra como aberto
    if process.is_alive() {
        state.running.lock().unwrap().insert(instance_id, process);
    }
    Ok(started)
}

#[tauri::command]
pub async fn kill_instance(state: State<'_, AppState>, instance_id: String) -> Result<bool> {
    Ok(state.running.lock().unwrap().get_mut(&instance_id).map(|p| p.kill()).unwrap_or(false))
}

#[tauri::command]
pub async fn running_instances(state: State<'_, AppState>) -> Result<Vec<String>> {
    Ok(state.running.lock().unwrap().keys().cloned().collect())
}
