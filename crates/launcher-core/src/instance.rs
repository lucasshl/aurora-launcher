//! Instâncias isoladas: `instances/<id>/instance.json` + `instances/<id>/.minecraft/`.
//! `libraries/` e `assets/` ficam na raiz e são compartilhados por todas.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::auth::microsoft::now_secs;
use crate::error::{LauncherError, Result};
use crate::paths::{write_atomic, LauncherPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Fabric,
    Forge,
    NeoForge,
}

impl ModLoader {
    pub fn supports_mods(self) -> bool {
        self != ModLoader::Vanilla
    }
    pub fn modrinth_name(self) -> &'static str {
        match self {
            ModLoader::Vanilla => "minecraft",
            ModLoader::Fabric => "fabric",
            ModLoader::Forge => "forge",
            ModLoader::NeoForge => "neoforge",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfig {
    /// Gravado no arquivo para referência; a fonte de verdade é o nome da pasta.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub minecraft_version: String,
    pub loader: ModLoader,
    /// Versão do loader (ex.: `0.16.9`). `None` = mais recente estável na criação.
    #[serde(default)]
    pub loader_version: Option<String>,
    /// ID do perfil em `versions/` (ex.: `fabric-loader-0.16.9-1.21.1`). Definido após instalar o loader.
    #[serde(default)]
    pub version_id: Option<String>,
    /// Java manual; `None` = gerenciado automaticamente.
    #[serde(default)]
    pub java_path: Option<PathBuf>,
    pub memory_min_mb: u32,
    pub memory_max_mb: u32,
    /// Argumentos extras da JVM definidos pelo usuário.
    #[serde(default)]
    pub jvm_args: Vec<String>,
    /// Aplica o preset G1GC (Aikar's Flags).
    #[serde(default = "default_true")]
    pub optimized_gc: bool,
    /// Instala/configura o CustomSkinLoader quando a conta é offline.
    #[serde(default = "default_true")]
    pub skin_loader: bool,
    /// AppCDS experimental (Java 19+). Desligado por padrão: em alguns PCs com Windows/Java 25
    /// a JVM fecha com código 1 sem mensagem. O antigo campo `fastStartup` é ignorado.
    #[serde(default)]
    pub experimental_cds: bool,
    #[serde(default)]
    pub resolution: Option<Resolution>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub last_played: Option<u64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewInstance {
    pub name: String,
    pub minecraft_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub memory_max_mb: Option<u32>,
}

const SUBDIRS: &[&str] = &["mods", "saves", "config", "resourcepacks", "shaderpacks", "screenshots", "logs"];

fn slugify(name: &str) -> String {
    let slug: String = name
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .trim_matches('.')
        .to_owned();
    if slug.is_empty() {
        "instancia".into()
    } else {
        slug.chars().take(48).collect()
    }
}

fn config_path(paths: &LauncherPaths, id: &str) -> PathBuf {
    paths.instance_dir(id).join("instance.json")
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return Err(LauncherError::Instance(format!("id de instância inválido: {id}")));
    }
    Ok(())
}

pub async fn create_instance(
    paths: &LauncherPaths,
    req: NewInstance,
    default_min: u32,
    default_max: u32,
) -> Result<InstanceConfig> {
    if req.name.trim().is_empty() {
        return Err(LauncherError::Instance("dê um nome à instância".into()));
    }
    let base = slugify(&req.name);
    let mut id = base.clone();
    let mut n = 2;
    while tokio::fs::try_exists(paths.instance_dir(&id)).await? {
        id = format!("{base}-{n}");
        n += 1;
    }

    let game_dir = paths.instance_game_dir(&id);
    for sub in SUBDIRS {
        tokio::fs::create_dir_all(game_dir.join(sub)).await?;
    }

    let config = InstanceConfig {
        id: id.clone(),
        name: req.name.trim().to_owned(),
        minecraft_version: req.minecraft_version,
        loader: req.loader,
        loader_version: req.loader_version.filter(|v| !v.is_empty()),
        version_id: None,
        java_path: None,
        memory_min_mb: default_min,
        memory_max_mb: req.memory_max_mb.unwrap_or(default_max).max(512),
        jvm_args: Vec::new(),
        optimized_gc: true,
        skin_loader: true,
        experimental_cds: false,
        resolution: None,
        created_at: now_secs(),
        last_played: None,
    };
    save_instance(paths, &config).await?;
    Ok(config)
}

pub async fn load_instance(paths: &LauncherPaths, id: &str) -> Result<InstanceConfig> {
    validate_id(id)?;
    let bytes = tokio::fs::read(config_path(paths, id))
        .await
        .map_err(|_| LauncherError::Instance(format!("instância '{id}' não encontrada")))?;
    let mut cfg: InstanceConfig = serde_json::from_slice(&bytes)
        .map_err(|e| LauncherError::Instance(format!("instance.json de '{id}' corrompido: {e}")))?;
    cfg.id = id.to_owned();
    Ok(cfg)
}

pub async fn save_instance(paths: &LauncherPaths, cfg: &InstanceConfig) -> Result<()> {
    validate_id(&cfg.id)?;
    write_atomic(&config_path(paths, &cfg.id), &serde_json::to_vec_pretty(cfg)?).await
}

pub async fn list_instances(paths: &LauncherPaths) -> Result<Vec<InstanceConfig>> {
    let mut out = Vec::new();
    let mut rd = tokio::fs::read_dir(paths.instances()).await?;
    while let Some(entry) = rd.next_entry().await? {
        if !entry.file_type().await?.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        match load_instance(paths, &id).await {
            Ok(cfg) => out.push(cfg),
            Err(e) => tracing::warn!("ignorando instância {id}: {e}"),
        }
    }
    out.sort_by(|a, b| b.last_played.unwrap_or(b.created_at).cmp(&a.last_played.unwrap_or(a.created_at)));
    Ok(out)
}

pub async fn delete_instance(paths: &LauncherPaths, id: &str) -> Result<()> {
    validate_id(id)?;
    let dir = paths.instance_dir(id);
    if !dir.starts_with(paths.instances()) {
        return Err(LauncherError::Instance("caminho fora da pasta de instâncias".into()));
    }
    tokio::fs::remove_dir_all(dir).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Meu Modpack 1.20!"), "Meu-Modpack-1.20");
        assert_eq!(slugify("../../etc"), "-..-etc");
        assert_eq!(slugify(".."), "instancia");
        assert_eq!(slugify("   "), "instancia");
    }
}
