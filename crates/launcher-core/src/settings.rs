use serde::{Deserialize, Serialize};

use crate::paths::{write_atomic, LauncherPaths};
use crate::Result;

/// Client ID do app Azure (embutido em build via `AURORA_MS_CLIENT_ID`, ou configurado em settings.json).
pub const BUILD_MS_CLIENT_ID: Option<&str> = option_env!("AURORA_MS_CLIENT_ID");

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Client ID do Azure AD usado no Device Code Flow.
    pub ms_client_id: Option<String>,
    /// Raiz de uma API compatível com CustomSkinAPI (ex.: `https://skins.seudominio.com/csl/`).
    pub custom_skin_api: Option<String>,
    /// Servidor de cosméticos (crate `cosmetics-server`), ex.: `https://cosmeticos.seudominio.com`.
    /// Sincroniza skin/capa e entra na cascata do CustomSkinLoader para todos os jogadores.
    pub cosmetics_server: Option<String>,
    pub download_concurrency: usize,
    pub default_memory_max_mb: u32,
    pub default_memory_min_mb: u32,
    /// Mantém o launcher aberto mostrando logs; se `false`, a JVM roda desacoplada.
    pub keep_console_attached: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ms_client_id: None,
            custom_skin_api: None,
            cosmetics_server: None,
            download_concurrency: 32,
            default_memory_max_mb: 4096,
            default_memory_min_mb: 1024,
            keep_console_attached: true,
        }
    }
}

impl Settings {
    pub async fn load(paths: &LauncherPaths) -> Self {
        match tokio::fs::read(paths.settings_file()).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                tracing::warn!("settings.json inválido, usando padrões: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }

    pub async fn save(&self, paths: &LauncherPaths) -> Result<()> {
        write_atomic(&paths.settings_file(), &serde_json::to_vec_pretty(self)?).await
    }

    pub fn client_id(&self) -> Option<String> {
        // O ID embutido no build tem prioridade; o campo das configurações só vale em builds sem ele
        BUILD_MS_CLIENT_ID
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
            .or_else(|| self.ms_client_id.clone().filter(|s| !s.trim().is_empty()))
    }
}
