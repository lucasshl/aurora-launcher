//! Extração de bibliotecas nativas (.dll/.so/.dylib) para uma pasta temporária por sessão.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::libraries::NativeJar;
use crate::error::Result;
use crate::paths::LauncherPaths;

const NATIVE_EXTENSIONS: &[&str] = &["dll", "so", "dylib", "jnilib"];

/// Cria `natives/<uuid>/` e extrai todos os nativos para ela (estrutura achatada).
pub async fn extract_natives(paths: &LauncherPaths, jars: Vec<NativeJar>) -> Result<PathBuf> {
    let dir = paths.natives_root().join(Uuid::new_v4().simple().to_string());
    let target = dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&target)?;
        for jar in &jars {
            extract_one(&jar.path, &target, &jar.exclude)?;
        }
        Ok(())
    })
    .await??;
    Ok(dir)
}

fn extract_one(jar: &Path, target: &Path, exclude: &[String]) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(jar)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_owned();
        if name.starts_with("META-INF/") || exclude.iter().any(|e| name.starts_with(e.as_str())) {
            continue;
        }
        // enclosed_name() protege contra zip-slip (../../)
        let Some(rel) = entry.enclosed_name() else { continue };
        let ext = rel.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if !NATIVE_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let Some(file_name) = rel.file_name() else { continue };
        let out_path = target.join(file_name);
        let mut out = File::create(&out_path)?;
        io::copy(&mut entry, &mut out)?;
    }
    Ok(())
}

/// Remove a pasta da sessão (chamado quando o jogo fecha).
pub async fn cleanup(dir: &Path) {
    if let Err(e) = tokio::fs::remove_dir_all(dir).await {
        tracing::debug!("não foi possível remover {}: {e}", dir.display());
    }
}

/// Remove sessões órfãs (ex.: launcher fechado à força). Chamado na inicialização.
pub async fn cleanup_stale(paths: &LauncherPaths) {
    if let Ok(mut rd) = tokio::fs::read_dir(paths.natives_root()).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            let _ = tokio::fs::remove_dir_all(entry.path()).await;
        }
    }
}
