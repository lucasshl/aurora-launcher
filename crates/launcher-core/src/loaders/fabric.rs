//! Instalação do Fabric via meta.fabricmc.net (gera um perfil `inheritsFrom` da vanilla).

use reqwest::Client;
use serde::Deserialize;

use crate::error::{LauncherError, Result};
use crate::http::{check_status, get_json, with_retry};
use crate::paths::{write_atomic, LauncherPaths};
use crate::version::model::VersionJson;

const META: &str = "https://meta.fabricmc.net/v2";

#[derive(Debug, Deserialize)]
struct LoaderEntry {
    loader: LoaderInfo,
}

#[derive(Debug, Deserialize)]
pub struct LoaderInfo {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

/// Versões do loader compatíveis com a versão do jogo (mais recente primeiro).
pub async fn loader_versions(client: &Client, mc_version: &str) -> Result<Vec<LoaderInfo>> {
    let url = format!("{META}/versions/loader/{}", urlencoding::encode(mc_version));
    let entries: Vec<LoaderEntry> = get_json(client, &url).await?;
    Ok(entries.into_iter().map(|e| e.loader).collect())
}

/// Instala o perfil e retorna `(version_id, loader_version)`.
pub async fn install(
    client: &Client,
    paths: &LauncherPaths,
    mc_version: &str,
    loader_version: Option<&str>,
) -> Result<(String, String)> {
    let loader_version = match loader_version {
        Some(v) => v.to_owned(),
        None => {
            let versions = loader_versions(client, mc_version).await?;
            versions
                .iter()
                .find(|l| l.stable)
                .or(versions.first())
                .map(|l| l.version.clone())
                .ok_or_else(|| LauncherError::Loader(format!("Fabric não suporta o Minecraft {mc_version}")))?
        }
    };

    let url = format!(
        "{META}/versions/loader/{}/{}/profile/json",
        urlencoding::encode(mc_version),
        urlencoding::encode(&loader_version)
    );
    let url = url.as_str();
    let lv = loader_version.as_str();
    let bytes = with_retry(3, || async move {
        let r = client.get(url).send().await?;
        if r.status().as_u16() == 400 || r.status().as_u16() == 404 {
            return Err(LauncherError::Loader(format!("Fabric {lv} não existe para o Minecraft {mc_version}")));
        }
        Ok(check_status(r)?.bytes().await?)
    })
    .await?;

    // Valida antes de gravar: um perfil corrompido quebraria a resolução depois
    let profile: VersionJson = serde_json::from_slice(&bytes)?;
    if profile.inherits_from.as_deref() != Some(mc_version) {
        return Err(LauncherError::Loader("perfil Fabric inesperado (inheritsFrom diferente)".into()));
    }
    write_atomic(&paths.version_json(&profile.id), &bytes).await?;
    Ok((profile.id, loader_version))
}
