//! Modloaders. O launcher trabalha com perfis no formato `inheritsFrom`, então
//! qualquer loader cujo JSON esteja em `versions/<id>/` é executável pelo resolvedor.

pub mod fabric;

use reqwest::Client;

use crate::error::{LauncherError, Result};
use crate::instance::{InstanceConfig, ModLoader};
use crate::paths::LauncherPaths;

/// Garante o perfil de versão da instância e devolve o ID a ser resolvido.
pub async fn ensure_loader_profile(
    client: &Client,
    paths: &LauncherPaths,
    instance: &mut InstanceConfig,
) -> Result<String> {
    if let Some(id) = &instance.version_id {
        if tokio::fs::try_exists(paths.version_json(id)).await.unwrap_or(false) {
            return Ok(id.clone());
        }
    }

    let id = match instance.loader {
        ModLoader::Vanilla => instance.minecraft_version.clone(),
        ModLoader::Fabric => {
            let (id, loader_version) =
                fabric::install(client, paths, &instance.minecraft_version, instance.loader_version.as_deref()).await?;
            instance.loader_version = Some(loader_version);
            id
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            // Forge/NeoForge exigem rodar os "processors" do instalador oficial (patch do client).
            // Se o perfil já foi gerado pelo instalador nesta pasta `versions/`, ele é usado.
            let id = instance.version_id.clone().unwrap_or_default();
            if id.is_empty() || !tokio::fs::try_exists(paths.version_json(&id)).await.unwrap_or(false) {
                return Err(LauncherError::Loader(format!(
                    "perfil {:?} não encontrado. Rode o instalador oficial apontando para {} e informe o ID gerado em versionId do instance.json",
                    instance.loader,
                    paths.root().display()
                )));
            }
            id
        }
    };
    instance.version_id = Some(id.clone());
    Ok(id)
}
