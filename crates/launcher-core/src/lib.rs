//! # launcher-core
//!
//! Núcleo independente de UI do Aurora Launcher. Nada aqui depende do Tauri:
//! a camada `src-tauri` apenas expõe estas funções como comandos e repassa
//! os eventos (progresso, logs, saída do processo) para o frontend.
//!
//! Fluxo de uma execução (`launch::launch_instance`):
//! 1. Carrega `instance.json` e garante o perfil do modloader (Fabric etc.).
//! 2. Resolve a versão (`version_manifest_v2.json` → JSON da versão → herança `inheritsFrom`).
//! 3. Garante o Java correto (local ou baixado da Adoptium para `runtimes/`).
//! 4. Baixa client JAR, libraries e assets em paralelo com verificação SHA-1.
//! 5. Instala o CustomSkinLoader em instâncias offline com modloader.
//! 6. Extrai nativos para uma pasta temporária da sessão.
//! 7. Monta classpath/argumentos e dispara a JVM com stdout/stderr redirecionados.

pub mod auth;
pub mod configs;
pub mod content;
pub mod cosmetics;
pub mod download;
pub mod error;
pub mod http;
pub mod instance;
pub mod java;
pub mod launch;
pub mod loaders;
pub mod modrinth;
pub mod optimize;
pub mod paths;
pub mod settings;
pub mod skins;
pub mod version;

pub use error::{LauncherError, Result};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Contexto compartilhado por todos os módulos.
pub struct Launcher {
    pub paths: paths::LauncherPaths,
    pub client: reqwest::Client,
    pub settings: RwLock<settings::Settings>,
    pub accounts: auth::accounts::AccountStore,
}

impl Launcher {
    /// Inicializa o launcher no diretório de dados padrão do SO
    /// (`%APPDATA%\AuroraLauncher`, `~/.local/share/AuroraLauncher`, `~/Library/Application Support/AuroraLauncher`).
    pub async fn new() -> Result<Arc<Self>> {
        let paths = paths::LauncherPaths::default_location()?;
        Self::with_paths(paths).await
    }

    pub async fn with_paths(paths: paths::LauncherPaths) -> Result<Arc<Self>> {
        paths.ensure_layout().await?;
        let settings = settings::Settings::load(&paths).await;
        let accounts = auth::accounts::AccountStore::load(&paths).await?;
        Ok(Arc::new(Self { client: http::build_client()?, settings: RwLock::new(settings), accounts, paths }))
    }
}
