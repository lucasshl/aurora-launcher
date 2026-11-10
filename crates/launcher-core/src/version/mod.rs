//! Manifest da Mojang, download do JSON da versão e resolução de herança (`inheritsFrom`).

pub mod model;
pub mod rules;

use std::collections::HashSet;

use reqwest::Client;

use crate::download::hash::sha1_file;
use crate::error::{LauncherError, Result};
use crate::http::{check_status, get_json, with_retry};
use crate::paths::{write_atomic, LauncherPaths};
use model::{Arguments, VersionJson, VersionManifest};

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// Baixa o manifest; se a rede falhar, usa o cache local (permite jogar offline).
pub async fn fetch_manifest(client: &Client, paths: &LauncherPaths) -> Result<VersionManifest> {
    match get_json::<VersionManifest>(client, MANIFEST_URL).await {
        Ok(manifest) => {
            if let Ok(bytes) = serde_json::to_vec(&manifest) {
                let _ = write_atomic(&paths.manifest_cache(), &bytes).await;
            }
            Ok(manifest)
        }
        Err(net_err) => {
            tracing::warn!("manifest indisponível ({net_err}); tentando cache local");
            let bytes = tokio::fs::read(paths.manifest_cache()).await.map_err(|_| net_err)?;
            Ok(serde_json::from_slice(&bytes)?)
        }
    }
}

async fn read_version_json(paths: &LauncherPaths, id: &str) -> Result<Option<VersionJson>> {
    let path = paths.version_json(id);
    match tokio::fs::read(&path).await {
        Ok(bytes) => match serde_json::from_slice::<VersionJson>(&bytes) {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                tracing::warn!("{} corrompido ({e}); será baixado de novo", path.display());
                let _ = tokio::fs::remove_file(&path).await;
                Ok(None)
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Garante `versions/<id>/<id>.json`. Versões vanilla são baixadas do manifest com
/// verificação SHA-1; perfis de loader (ex.: `fabric-loader-…`) precisam já existir no disco.
pub async fn ensure_version_json(
    client: &Client,
    paths: &LauncherPaths,
    manifest: Option<&VersionManifest>,
    id: &str,
) -> Result<VersionJson> {
    let path = paths.version_json(id);
    let entry = manifest.and_then(|m| m.versions.iter().find(|v| v.id == id));

    if let Some(local) = read_version_json(paths, id).await? {
        match entry {
            // Confere integridade do JSON vanilla contra o manifest
            Some(e) if sha1_file(&path).await.ok().as_deref() != Some(e.sha1.as_str()) => {
                tracing::warn!("hash do JSON de {id} não confere; baixando de novo");
            }
            _ => return Ok(local),
        }
    }

    let entry = entry.ok_or_else(|| LauncherError::VersionNotFound(id.to_owned()))?;
    let url = entry.url.as_str();
    let expected = entry.sha1.as_str();
    let bytes = with_retry(3, || async move {
        let bytes = check_status(client.get(url).send().await?)?.bytes().await?;
        let actual = crate::download::hash::sha1_bytes(&bytes);
        if actual != expected {
            return Err(LauncherError::HashMismatch {
                path: format!("{id}.json"),
                expected: expected.to_owned(),
                actual,
            });
        }
        Ok(bytes)
    })
    .await?;
    write_atomic(&path, &bytes).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Versão "achatada", pronta para planejar downloads e montar argumentos.
#[derive(Debug, Clone)]
pub struct ResolvedVersion {
    pub json: VersionJson,
    /// ID da versão cujo `.jar` é o client (a vanilla na base da herança).
    pub client_jar_id: String,
    /// Cadeia do perfil pedido até a raiz, ex.: `["fabric-loader-0.16.9-1.21.1", "1.21.1"]`.
    pub chain: Vec<String>,
}

pub async fn resolve_version(
    client: &Client,
    paths: &LauncherPaths,
    manifest: Option<&VersionManifest>,
    id: &str,
) -> Result<ResolvedVersion> {
    let mut stack = vec![ensure_version_json(client, paths, manifest, id).await?];
    let mut chain = vec![id.to_owned()];
    let mut seen: HashSet<String> = chain.iter().cloned().collect();

    while let Some(parent) = stack.last().and_then(|v| v.inherits_from.clone()) {
        if !seen.insert(parent.clone()) {
            return Err(LauncherError::Other(format!("herança circular de versões em {parent}")));
        }
        stack.push(ensure_version_json(client, paths, manifest, &parent).await?);
        chain.push(parent);
    }

    let mut merged = stack.pop().expect("stack nunca vazia");
    let mut client_jar_id = merged.id.clone();
    while let Some(child) = stack.pop() {
        if child.downloads.as_ref().and_then(|d| d.client.as_ref()).is_some() {
            client_jar_id = child.id.clone();
        }
        merged = merge(merged, child);
    }

    Ok(ResolvedVersion { json: merged, client_jar_id, chain })
}

/// Mescla filho sobre pai seguindo a semântica do launcher oficial:
/// libraries do filho primeiro (e substituem as do pai com mesmo grupo:artefato),
/// argumentos concatenados, campos escalares do filho prevalecem.
fn merge(parent: VersionJson, child: VersionJson) -> VersionJson {
    let child_keys: HashSet<String> =
        child.libraries.iter().filter_map(|l| model::MavenCoord::parse(&l.name)).map(|c| c.dedupe_key()).collect();

    let mut libraries = child.libraries;
    libraries.extend(
        parent.libraries.into_iter().filter(|l| {
            model::MavenCoord::parse(&l.name).map(|c| !child_keys.contains(&c.dedupe_key())).unwrap_or(true)
        }),
    );

    let arguments = match (parent.arguments, child.arguments) {
        (Some(p), Some(c)) => Some(Arguments {
            game: p.game.into_iter().chain(c.game).collect(),
            jvm: p.jvm.into_iter().chain(c.jvm).collect(),
        }),
        (p, c) => c.or(p),
    };

    VersionJson {
        id: child.id,
        inherits_from: None,
        kind: child.kind.or(parent.kind),
        main_class: child.main_class.or(parent.main_class),
        minecraft_arguments: child.minecraft_arguments.or(parent.minecraft_arguments),
        arguments,
        asset_index: child.asset_index.or(parent.asset_index),
        assets: child.assets.or(parent.assets),
        downloads: child.downloads.or(parent.downloads),
        libraries,
        java_version: child.java_version.or(parent.java_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::Library;

    fn lib(name: &str) -> Library {
        serde_json::from_value(serde_json::json!({ "name": name })).unwrap()
    }

    #[test]
    fn child_overrides_parent_library() {
        let parent = VersionJson {
            id: "1.20.1".into(),
            main_class: Some("net.minecraft.client.main.Main".into()),
            libraries: vec![lib("org.ow2.asm:asm:9.3"), lib("com.mojang:brigadier:1.1.8")],
            ..Default::default()
        };
        let child = VersionJson {
            id: "fabric".into(),
            inherits_from: Some("1.20.1".into()),
            main_class: Some("net.fabricmc.loader.impl.launch.knot.KnotClient".into()),
            libraries: vec![lib("org.ow2.asm:asm:9.7")],
            ..Default::default()
        };
        let m = merge(parent, child);
        assert_eq!(m.main_class.unwrap(), "net.fabricmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(m.libraries.len(), 2);
        assert_eq!(m.libraries[0].name, "org.ow2.asm:asm:9.7");
    }
}
