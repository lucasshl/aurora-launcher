mod commands;
mod features;
mod state;

use launcher_core::download::natives;
use launcher_core::Launcher;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,launcher_core=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let launcher = tauri::async_runtime::block_on(async {
                let launcher = Launcher::new().await?;
                natives::cleanup_stale(&launcher.paths).await;
                Ok::<_, launcher_core::LauncherError>(launcher)
            })?;
            tracing::info!("dados do launcher em {}", launcher.paths.root().display());
            app.manage(AppState::new(launcher));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::system_info,
            commands::list_accounts,
            commands::add_offline_account,
            commands::begin_microsoft_login,
            commands::complete_microsoft_login,
            commands::cancel_microsoft_login,
            commands::remove_account,
            commands::set_active_account,
            commands::list_minecraft_versions,
            commands::list_fabric_versions,
            commands::list_instances,
            commands::create_instance,
            commands::update_instance,
            commands::delete_instance,
            commands::open_instance_folder,
            commands::set_offline_skin,
            commands::launch_instance,
            commands::kill_instance,
            commands::running_instances,
            features::list_content,
            features::toggle_content,
            features::remove_content,
            features::search_content,
            features::content_versions,
            features::install_content,
            features::check_content_updates,
            features::update_content,
            features::open_content_folder,
            features::list_configs,
            features::read_config,
            features::write_config,
            features::curated_mods,
            features::install_curated,
            features::apply_graphics_preset,
            features::get_wardrobe,
            features::import_cosmetic,
            features::remove_cosmetic,
            features::set_cosmetic_model,
            features::equip_cosmetic,
            features::cosmetic_data_url,
            features::upload_skin_to_mojang,
            features::mojang_capes,
            features::set_mojang_cape,
            features::sync_cosmetics,
            features::import_cosmetic_data,
            features::gallery_list,
            features::gallery_import,
            features::gallery_publish,
            features::set_items3d,
            features::cosmetics_mod_status,
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o Aurora Launcher");
}
