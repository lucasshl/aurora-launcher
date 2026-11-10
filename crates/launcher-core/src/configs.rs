//! Editor de configurações de mods (`config/`) e do jogo (`options.txt`).
//! Valida JSON/TOML antes de salvar e guarda um `.bak` da versão anterior.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::error::{LauncherError, Result};
use crate::paths::{write_atomic, LauncherPaths};

const TEXT_EXTENSIONS: &[&str] =
    &["json", "json5", "jsonc", "toml", "properties", "cfg", "conf", "txt", "yml", "yaml", "ini", "snbt", "mcmeta"];
const MAX_SIZE: u64 = 2 * 1024 * 1024;
const ROOT_FILES: &[&str] = &["options.txt", "optionsof.txt", "optionsshaders.txt"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigFile {
    /// Caminho relativo ao `.minecraft` da instância, com `/`.
    pub path: String,
    pub size: u64,
    pub modified: u64,
}

fn is_text(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| TEXT_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Resolve um caminho relativo garantindo que ele não escapa do `.minecraft`.
fn safe_join(game_dir: &Path, rel: &str) -> Result<PathBuf> {
    let rel_path = Path::new(rel);
    let ok = rel_path.components().all(|c| matches!(c, Component::Normal(_)));
    let allowed_root = rel.starts_with("config/") || ROOT_FILES.contains(&rel);
    if !ok || !allowed_root || !is_text(rel_path) {
        return Err(LauncherError::Instance(format!("caminho de configuração não permitido: {rel}")));
    }
    Ok(game_dir.join(rel_path))
}

pub async fn list_configs(paths: &LauncherPaths, instance_id: &str) -> Result<Vec<ConfigFile>> {
    let game_dir = paths.instance_game_dir(instance_id);
    tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for f in ROOT_FILES {
            push_file(&game_dir, &game_dir.join(f), &mut out);
        }
        walk(&game_dir, &game_dir.join("config"), 0, &mut out);
        out.sort_by(|a, b| a.path.to_lowercase().cmp(&b.path.to_lowercase()));
        out
    })
    .await
    .map_err(Into::into)
}

fn push_file(game_dir: &Path, path: &Path, out: &mut Vec<ConfigFile>) {
    let Ok(meta) = std::fs::metadata(path) else { return };
    if !meta.is_file() || meta.len() > MAX_SIZE || !is_text(path) {
        return;
    }
    let Ok(rel) = path.strip_prefix(game_dir) else { return };
    out.push(ConfigFile {
        path: rel.to_string_lossy().replace('\\', "/"),
        size: meta.len(),
        modified: meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0),
    });
}

fn walk(game_dir: &Path, dir: &Path, depth: u32, out: &mut Vec<ConfigFile>) {
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(game_dir, &path, depth + 1, out),
            Ok(t) if t.is_file() => push_file(game_dir, &path, out),
            _ => {}
        }
    }
}

pub async fn read_config(paths: &LauncherPaths, instance_id: &str, rel: &str) -> Result<String> {
    let path = safe_join(&paths.instance_game_dir(instance_id), rel)?;
    let bytes = tokio::fs::read(&path).await?;
    if bytes.len() as u64 > MAX_SIZE {
        return Err(LauncherError::Instance("arquivo grande demais para o editor".into()));
    }
    String::from_utf8(bytes).map_err(|_| LauncherError::Instance("o arquivo não é texto UTF-8".into()))
}

/// Valida a sintaxe conforme a extensão. Retorna mensagem com linha/coluna se inválido.
pub fn validate_syntax(rel: &str, content: &str) -> std::result::Result<(), String> {
    let ext = rel.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "json" | "mcmeta" => serde_json::from_str::<serde_json::Value>(content)
            .map(|_| ())
            .map_err(|e| format!("JSON inválido na linha {}, coluna {}: {e}", e.line(), e.column())),
        "toml" => toml::from_str::<toml::Value>(content).map(|_| ()).map_err(|e| format!("TOML inválido: {e}")),
        _ => Ok(()),
    }
}

pub async fn write_config(paths: &LauncherPaths, instance_id: &str, rel: &str, content: &str) -> Result<()> {
    let path = safe_join(&paths.instance_game_dir(instance_id), rel)?;
    validate_syntax(rel, content).map_err(LauncherError::Instance)?;
    if tokio::fs::try_exists(&path).await? {
        let mut bak = path.as_os_str().to_owned();
        bak.push(".bak");
        tokio::fs::copy(&path, PathBuf::from(bak)).await?;
    }
    write_atomic(&path, content.as_bytes()).await
}

/// Mescla chaves em `options.txt` (formato `chave:valor`), preservando as demais.
pub async fn merge_options(game_dir: &Path, values: &[(&str, String)]) -> Result<()> {
    let path = game_dir.join("options.txt");
    let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let mut lines: Vec<String> = existing.lines().map(str::to_owned).collect();
    for (key, value) in values {
        let prefix = format!("{key}:");
        match lines.iter_mut().find(|l| l.starts_with(&prefix)) {
            Some(line) => *line = format!("{key}:{value}"),
            None => lines.push(format!("{key}:{value}")),
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    write_atomic(&path, out.as_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_traversal() {
        let g = Path::new("/game");
        assert!(safe_join(g, "config/sodium-options.json").is_ok());
        assert!(safe_join(g, "options.txt").is_ok());
        assert!(safe_join(g, "config/../../etc/passwd.txt").is_err());
        assert!(safe_join(g, "mods/x.jar").is_err());
        assert!(safe_join(g, "/etc/hosts.txt").is_err());
    }

    #[test]
    fn validates_json_and_toml() {
        assert!(validate_syntax("config/a.json", r#"{"a":1}"#).is_ok());
        assert!(validate_syntax("config/a.json", r#"{"a":}"#).unwrap_err().contains("linha 1"));
        assert!(validate_syntax("config/a.toml", "a = 1").is_ok());
        assert!(validate_syntax("config/a.toml", "a = ").is_err());
        assert!(validate_syntax("options.txt", "qualquer:coisa").is_ok());
    }

    #[tokio::test]
    async fn merges_options() {
        let dir = std::env::temp_dir().join(format!("opt-{}", uuid::Uuid::new_v4().simple()));
        tokio::fs::create_dir_all(&dir).await.unwrap();
        tokio::fs::write(dir.join("options.txt"), "renderDistance:12\nlang:pt_br\n").await.unwrap();
        merge_options(&dir, &[("renderDistance", "6".into()), ("particles", "2".into())]).await.unwrap();
        let s = tokio::fs::read_to_string(dir.join("options.txt")).await.unwrap();
        assert_eq!(s, "renderDistance:6\nlang:pt_br\nparticles:2\n");
        tokio::fs::remove_dir_all(dir).await.ok();
    }
}
