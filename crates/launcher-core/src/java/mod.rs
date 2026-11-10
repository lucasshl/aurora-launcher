//! Gerenciamento de Java: escolhe a versão certa, procura em `runtimes/` e, se faltar,
//! baixa o JRE portátil da Adoptium (Eclipse Temurin) com verificação SHA-256.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;

use crate::download::{Checksum, DownloadTask, Downloader, ProgressSink, Stage};
use crate::error::{LauncherError, Result};
use crate::http::get_json;
use crate::paths::LauncherPaths;
use crate::version::model::VersionJson;

/// Regra pedida para versões release:
/// ≤ 1.16.5 → 8 · 1.17 a 1.20.4 → 17 · ≥ 1.20.5 → 21.
pub fn java_for_minecraft(mc_version: &str) -> u32 {
    let mut parts = mc_version.split(['.', '-', ' ']).map(|p| p.parse::<u32>().ok());
    let (Some(Some(1)), Some(Some(minor))) = (parts.next(), parts.next()) else {
        // Snapshots (ex.: 24w14a) e formatos novos: usa o mais recente
        return 21;
    };
    let patch = parts.next().flatten().unwrap_or(0);
    match (minor, patch) {
        (m, _) if m <= 16 => 8,
        (m, _) if m < 20 => 17,
        (20, p) if p <= 4 => 17,
        _ => 21,
    }
}

/// Prioriza `javaVersion.majorVersion` do próprio JSON (fonte oficial; cobre snapshots e
/// versões futuras como Java 25) e cai na tabela fixa se o campo não existir.
pub fn required_java(version: &VersionJson, mc_version: &str) -> u32 {
    version.java_version.as_ref().map(|j| j.major_version).unwrap_or_else(|| java_for_minecraft(mc_version))
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn runtime_dir(paths: &LauncherPaths, major: u32) -> PathBuf {
    paths.runtimes().join(format!("java-{major}"))
}

/// Procura `bin/java` até 4 níveis abaixo (cobre `jdk8u…-jre/bin` e `Contents/Home/bin` do macOS).
fn find_java_in(dir: &Path, depth: u32) -> Option<PathBuf> {
    let candidate = dir.join("bin").join(java_binary_name());
    if candidate.is_file() {
        return Some(candidate);
    }
    if depth == 0 {
        return None;
    }
    let mut entries: Vec<_> = std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    entries.sort();
    entries.into_iter().find_map(|p| find_java_in(&p, depth - 1))
}

/// Executa `java -version` e extrai a versão principal.
pub async fn probe_java_major(java: &Path) -> Result<u32> {
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-version").stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let out = cmd
        .output()
        .await
        .map_err(|e| LauncherError::Java(format!("não foi possível executar {}: {e}", java.display())))?;
    let text = String::from_utf8_lossy(&out.stderr);
    parse_java_version(&text).ok_or_else(|| {
        LauncherError::Java(format!("saída de -version não reconhecida: {}", text.lines().next().unwrap_or("")))
    })
}

fn parse_java_version(output: &str) -> Option<u32> {
    let start = output.find('"')? + 1;
    let rest = &output[start..];
    let version = &rest[..rest.find('"')?];
    let mut it = version.split(['.', '_', '-', '+']);
    let first: u32 = it.next()?.parse().ok()?;
    if first == 1 {
        it.next()?.parse().ok()
    } else {
        Some(first)
    }
}

/// Retorna um Java utilizável para `major`, baixando-o se necessário.
pub async fn ensure_java(
    paths: &LauncherPaths,
    downloader: &Downloader,
    major: u32,
    sink: &dyn ProgressSink,
) -> Result<PathBuf> {
    let dir = runtime_dir(paths, major);
    let scan_dir = dir.clone();
    if let Some(java) = tokio::task::spawn_blocking(move || find_java_in(&scan_dir, 4)).await? {
        match probe_java_major(&java).await {
            Ok(found) if found == major => return Ok(java),
            Ok(found) => tracing::warn!("{} é Java {found}, esperado {major}; reinstalando", java.display()),
            Err(e) => tracing::warn!("runtime em {} quebrado ({e}); reinstalando", dir.display()),
        }
        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
    install_temurin(paths, downloader, major, sink).await
}

#[derive(Debug, Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
}

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
    checksum: Option<String>,
    size: Option<u64>,
}

fn adoptium_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn adoptium_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "x86" => Ok("x32"),
        "aarch64" => Ok("aarch64"),
        "arm" => Ok("arm"),
        other => Err(LauncherError::Java(format!("arquitetura não suportada pela Adoptium: {other}"))),
    }
}

async fn find_package(downloader: &Downloader, major: u32) -> Result<AdoptiumPackage> {
    let arch = adoptium_arch()?;
    // JRE primeiro; se não existir para a plataforma (ex.: Java 8 em Apple Silicon), tenta JDK.
    for image in ["jre", "jdk"] {
        let url = format!(
            "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={arch}&image_type={image}&os={}&vendor=eclipse",
            adoptium_os()
        );
        let assets: Vec<AdoptiumAsset> = get_json(downloader.client(), &url).await?;
        if let Some(asset) = assets.into_iter().next() {
            return Ok(asset.binary.package);
        }
    }
    Err(LauncherError::Java(format!(
        "a Adoptium não publica Java {major} para {}/{arch}; defina um Java manualmente na instância",
        adoptium_os()
    )))
}

async fn install_temurin(
    paths: &LauncherPaths,
    downloader: &Downloader,
    major: u32,
    sink: &dyn ProgressSink,
) -> Result<PathBuf> {
    let package = find_package(downloader, major).await?;
    let archive = paths.runtimes().join(&package.name);
    let checksum = package.checksum.map(|c| Checksum::Sha256(c.to_ascii_lowercase())).unwrap_or(Checksum::None);
    downloader
        .download_one(Stage::Java, DownloadTask::new(package.link, archive.clone(), checksum, package.size), sink)
        .await?;

    let dir = runtime_dir(paths, major);
    let staging = paths.runtimes().join(format!(".java-{major}.extracting"));
    let _ = tokio::fs::remove_dir_all(&staging).await;
    let (archive_c, staging_c) = (archive.clone(), staging.clone());
    tokio::task::spawn_blocking(move || extract_archive(&archive_c, &staging_c)).await??;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::rename(&staging, &dir).await?;
    let _ = tokio::fs::remove_file(&archive).await;

    let scan = dir.clone();
    let java = tokio::task::spawn_blocking(move || find_java_in(&scan, 4))
        .await?
        .ok_or_else(|| LauncherError::Java("o pacote baixado não contém bin/java".into()))?;
    let found = probe_java_major(&java).await?;
    if found != major {
        return Err(LauncherError::Java(format!("o pacote instalado é Java {found}, esperado {major}")));
    }
    Ok(java)
}

fn extract_archive(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let name = archive.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".zip") {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(archive)?)?;
        // extract() valida caminhos contra zip-slip e preserva permissões unix
        zip.extract(dest)?;
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(archive)?);
        let mut tar = tar::Archive::new(gz);
        tar.set_preserve_permissions(true);
        // unpack() recusa entradas fora de `dest`
        tar.unpack(dest)?;
    } else {
        return Err(LauncherError::Java(format!("formato de pacote desconhecido: {}", archive.display())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_table() {
        assert_eq!(java_for_minecraft("1.8.9"), 8);
        assert_eq!(java_for_minecraft("1.12.2"), 8);
        assert_eq!(java_for_minecraft("1.16.5"), 8);
        assert_eq!(java_for_minecraft("1.17"), 17);
        assert_eq!(java_for_minecraft("1.18.2"), 17);
        assert_eq!(java_for_minecraft("1.20.4"), 17);
        assert_eq!(java_for_minecraft("1.20.5"), 21);
        assert_eq!(java_for_minecraft("1.21.4"), 21);
        assert_eq!(java_for_minecraft("24w14a"), 21);
    }

    #[test]
    fn parses_version_output() {
        assert_eq!(parse_java_version(r#"openjdk version "1.8.0_432""#), Some(8));
        assert_eq!(parse_java_version(r#"openjdk version "17.0.13" 2024-10-15"#), Some(17));
        assert_eq!(parse_java_version(r#"openjdk version "21" 2023-09-19"#), Some(21));
    }
}
