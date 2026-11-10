use std::path::{Path, PathBuf};

use crate::error::{LauncherError, Result};

pub const APP_DIR_NAME: &str = "AuroraLauncher";

/// Layout em disco:
/// ```text
/// <root>/
///   libraries/          compartilhado entre instâncias
///   assets/             compartilhado (indexes/, objects/, virtual/)
///   versions/<id>/      JSON + client JAR por versão (vanilla e perfis de loader)
///   runtimes/java-<N>/  JREs portáteis baixados da Adoptium
///   instances/<id>/     instance.json + .minecraft isolado (mods, saves, config…)
///   natives/<sessão>/   nativos extraídos, apagados ao fechar o jogo
///   accounts.json       metadados de contas (sem segredos)
///   settings.json
/// ```
#[derive(Debug, Clone)]
pub struct LauncherPaths {
    root: PathBuf,
}

impl LauncherPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn default_location() -> Result<Self> {
        let base = dirs::data_dir()
            .ok_or_else(|| LauncherError::Other("não foi possível localizar a pasta de dados do usuário".into()))?;
        Ok(Self::new(base.join(APP_DIR_NAME)))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn libraries(&self) -> PathBuf {
        self.root.join("libraries")
    }
    pub fn assets(&self) -> PathBuf {
        self.root.join("assets")
    }
    pub fn asset_indexes(&self) -> PathBuf {
        self.assets().join("indexes")
    }
    pub fn asset_objects(&self) -> PathBuf {
        self.assets().join("objects")
    }
    pub fn versions(&self) -> PathBuf {
        self.root.join("versions")
    }
    pub fn runtimes(&self) -> PathBuf {
        self.root.join("runtimes")
    }
    pub fn instances(&self) -> PathBuf {
        self.root.join("instances")
    }
    pub fn natives_root(&self) -> PathBuf {
        self.root.join("natives")
    }
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }
    pub fn manifest_cache(&self) -> PathBuf {
        self.versions().join("version_manifest_v2.json")
    }

    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions().join(id)
    }
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances().join(id)
    }
    /// O `game_directory` da instância (equivalente ao `.minecraft`).
    pub fn instance_game_dir(&self, id: &str) -> PathBuf {
        self.instance_dir(id).join(".minecraft")
    }

    pub async fn ensure_layout(&self) -> Result<()> {
        for dir in [
            self.libraries(),
            self.asset_indexes(),
            self.asset_objects(),
            self.versions(),
            self.runtimes(),
            self.instances(),
            self.natives_root(),
        ] {
            tokio::fs::create_dir_all(&dir).await?;
        }
        Ok(())
    }
}

/// Grava um arquivo de forma atômica (tmp + rename) para nunca deixar JSON pela metade.
pub async fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await?;
    Ok(())
}
