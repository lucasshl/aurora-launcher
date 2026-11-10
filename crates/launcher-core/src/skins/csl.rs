//! Automação do CustomSkinLoader (CSL) para contas offline.
//!
//! 1. Busca no Modrinth a versão do CSL compatível com loader + versão do jogo.
//! 2. Baixa o `.jar` (SHA-1 do Modrinth) para `mods/` e remove versões antigas.
//! 3. Instala dependências marcadas como `required` (resolvidas pelo próprio Modrinth).
//! 4. Escreve `CustomSkinLoader/CustomSkinLoader.json` com a cascata:
//!    LocalSkin → API customizada → Ely.by → Mojang.

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::content;
use crate::download::{Downloader, ProgressSink};
use crate::error::{LauncherError, Result};
use crate::instance::InstanceConfig;
use crate::modrinth::ContentKind;
use crate::paths::{write_atomic, LauncherPaths};

pub const CSL_PROJECT: &str = "customskinloader";
/// Mod que renderiza orelhas, rabo, chifres e asas codificados na própria skin.
pub const EARS_PROJECT: &str = "ears";

/// Instala o CSL (e dependências obrigatórias) e escreve a configuração.
/// Retorna `Ok(false)` se não houver build do CSL para a combinação loader/versão.
pub async fn ensure_custom_skin_loader(
    paths: &LauncherPaths,
    downloader: &Downloader,
    instance: &InstanceConfig,
    custom_api_root: Option<&str>,
    cosmetics_server: Option<&str>,
    sink: &dyn ProgressSink,
) -> Result<bool> {
    if !instance.loader.supports_mods() {
        return Ok(false);
    }
    let report =
        content::install_projects(paths, downloader, instance, ContentKind::Mod, &[CSL_PROJECT.to_owned()], None, sink)
            .await?;
    if !report.skipped.is_empty() {
        return Ok(false);
    }
    // Ears: orelhas, rabo, chifres, focinho e asas desenhados dentro da skin (opcional)
    if let Err(e) =
        content::install_projects(paths, downloader, instance, ContentKind::Mod, &[EARS_PROJECT.to_owned()], None, sink)
            .await
    {
        tracing::warn!("Ears não instalado: {e}");
    }
    write_config(&paths.instance_game_dir(&instance.id), custom_api_root, cosmetics_server).await?;
    Ok(true)
}

/// Lista de resolvers em cascata. Campos fora de `loadlist` definidos pelo usuário são preservados.
pub fn build_loadlist(custom_api_root: Option<&str>, cosmetics_server: Option<&str>) -> Value {
    let mut list = vec![json!({
        "name": "LocalSkin",
        "type": "Legacy",
        "checkPNG": false,
        "skin": "LocalSkin/skins/{USERNAME}.png",
        "model": "auto",
        "cape": "LocalSkin/capes/{USERNAME}.png",
        "elytra": "LocalSkin/elytras/{USERNAME}.png"
    })];
    if let Some(server) = cosmetics_server.map(str::trim).filter(|r| !r.is_empty()) {
        list.push(json!({ "name": "AuroraCosmetics", "type": "CustomSkinAPI", "root": format!("{}/csl/", server.trim_end_matches('/')) }));
    }
    if let Some(root) = custom_api_root.map(str::trim).filter(|r| !r.is_empty()) {
        let root = if root.ends_with('/') { root.to_owned() } else { format!("{root}/") };
        list.push(json!({ "name": "CustomAPI", "type": "CustomSkinAPI", "root": root }));
    }
    list.push(json!({ "name": "ElyBy", "type": "ElyByAPI", "root": "http://skinsystem.ely.by/textures/" }));
    list.push(json!({
        "name": "Mojang",
        "type": "MojangAPI",
        "apiRoot": "https://api.mojang.com/",
        "sessionRoot": "https://sessionserver.mojang.com/"
    }));
    Value::Array(list)
}

pub async fn write_config(
    game_dir: &Path,
    custom_api_root: Option<&str>,
    cosmetics_server: Option<&str>,
) -> Result<PathBuf> {
    let path = game_dir.join("CustomSkinLoader").join("CustomSkinLoader.json");
    let mut root: Map<String, Value> = match tokio::fs::read(&path).await {
        Ok(bytes) => {
            serde_json::from_slice::<Value>(&bytes).ok().and_then(|v| v.as_object().cloned()).unwrap_or_default()
        }
        Err(_) => Map::new(),
    };
    root.insert("enable".into(), Value::Bool(true));
    root.insert("loadlist".into(), build_loadlist(custom_api_root, cosmetics_server));
    write_atomic(&path, &serde_json::to_vec_pretty(&Value::Object(root))?).await?;
    Ok(path)
}

/// Copia um PNG para `LocalSkin/skins/<nick>.png` (primeira opção da cascata).
pub async fn set_local_skin(paths: &LauncherPaths, instance_id: &str, username: &str, png: &Path) -> Result<()> {
    let bytes = tokio::fs::read(png).await?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(LauncherError::Other("o arquivo de skin precisa ser um PNG".into()));
    }
    let target =
        paths.instance_game_dir(instance_id).join("CustomSkinLoader/LocalSkin/skins").join(format!("{username}.png"));
    write_atomic(&target, &bytes).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cascade_order() {
        let list = build_loadlist(Some("https://skins.example.com/csl"), Some("https://cos.example.com/"));
        let names: Vec<&str> = list.as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["LocalSkin", "AuroraCosmetics", "CustomAPI", "ElyBy", "Mojang"]);
        assert_eq!(list[1]["root"], "https://cos.example.com/csl/");
        assert_eq!(list[2]["root"], "https://skins.example.com/csl/");
        let list = build_loadlist(None, None);
        assert_eq!(list.as_array().unwrap().len(), 3);
    }
}
