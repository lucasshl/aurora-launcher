//! Cliente da API do Modrinth (busca, versões compatíveis, identificação por hash e atualizações).

use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{LauncherError, Result};
use crate::http::{check_status, with_retry};
use crate::instance::ModLoader;

pub const API: &str = "https://api.modrinth.com/v2";

/// Tipo de conteúdo e a pasta da instância onde ele mora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentKind {
    Mod,
    ResourcePack,
    Shader,
}

impl ContentKind {
    pub fn dir(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::Shader => "shaderpacks",
        }
    }
    pub fn project_type(self) -> &'static str {
        match self {
            ContentKind::Mod => "mod",
            ContentKind::ResourcePack => "resourcepack",
            ContentKind::Shader => "shader",
        }
    }
    /// "Loaders" que o Modrinth usa para filtrar cada tipo.
    pub fn loaders(self, loader: ModLoader) -> Vec<&'static str> {
        match self {
            ContentKind::Mod => vec![loader.modrinth_name()],
            ContentKind::ResourcePack => vec!["minecraft"],
            ContentKind::Shader => vec!["iris", "optifine", "canvas", "vanilla"],
        }
    }
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            ContentKind::Mod => &["jar"],
            ContentKind::ResourcePack | ContentKind::Shader => &["zip"],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    #[serde(alias = "project_id")]
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default, alias = "icon_url")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default, alias = "date_modified")]
    pub date_modified: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    #[serde(alias = "total_hits")]
    pub total_hits: u64,
    pub offset: u64,
    pub limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    #[serde(alias = "project_id")]
    pub project_id: String,
    pub name: String,
    #[serde(alias = "version_number")]
    pub version_number: String,
    #[serde(default, alias = "game_versions")]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default, alias = "version_type")]
    pub version_type: String,
    #[serde(default, alias = "date_published")]
    pub date_published: String,
}

impl Version {
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or(self.files.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub size: u64,
    pub hashes: FileHashes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileHashes {
    pub sha1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    #[serde(default, alias = "project_id")]
    pub project_id: Option<String>,
    #[serde(default, alias = "version_id")]
    pub version_id: Option<String>,
    #[serde(alias = "dependency_type")]
    pub dependency_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, alias = "icon_url")]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl SortIndex {
    fn as_str(self) -> &'static str {
        match self {
            SortIndex::Relevance => "relevance",
            SortIndex::Downloads => "downloads",
            SortIndex::Follows => "follows",
            SortIndex::Newest => "newest",
            SortIndex::Updated => "updated",
        }
    }
}

fn json_list(items: &[&str]) -> String {
    serde_json::to_string(items).unwrap_or_else(|_| "[]".into())
}

/// GET que trata 404 como "não existe" em vez de erro.
async fn get_optional<T: serde::de::DeserializeOwned>(client: &Client, url: &str) -> Result<Option<T>> {
    with_retry(3, || async move {
        let r = client.get(url).send().await?;
        if r.status().as_u16() == 404 {
            return Ok(None);
        }
        Ok(Some(check_status(r)?.json::<T>().await?))
    })
    .await
}

pub async fn search(
    client: &Client,
    query: &str,
    kind: ContentKind,
    loader: ModLoader,
    mc_version: &str,
    sort: SortIndex,
    offset: u32,
    limit: u32,
) -> Result<SearchResult> {
    let mut facets: Vec<Vec<String>> =
        vec![vec![format!("project_type:{}", kind.project_type())], vec![format!("versions:{mc_version}")]];
    if kind == ContentKind::Mod {
        facets.push(vec![format!("categories:{}", loader.modrinth_name())]);
        // Mods só de servidor não servem para o client
        facets.push(vec!["client_side:required".into(), "client_side:optional".into()]);
    }
    let url = format!(
        "{API}/search?query={}&facets={}&index={}&offset={offset}&limit={}",
        urlencoding::encode(query),
        urlencoding::encode(&serde_json::to_string(&facets)?),
        sort.as_str(),
        limit.clamp(1, 100)
    );
    let url = url.as_str();
    with_retry(3, || async move { Ok(check_status(client.get(url).send().await?)?.json::<SearchResult>().await?) })
        .await
}

/// Versões compatíveis com loader/MC, mais recentes primeiro. Projeto inexistente → lista vazia.
pub async fn compatible_versions(
    client: &Client,
    project: &str,
    loaders: &[&str],
    mc_version: &str,
) -> Result<Vec<Version>> {
    let url = format!(
        "{API}/project/{}/version?loaders={}&game_versions={}",
        urlencoding::encode(project),
        urlencoding::encode(&json_list(loaders)),
        urlencoding::encode(&json_list(&[mc_version]))
    );
    Ok(get_optional::<Vec<Version>>(client, &url).await?.unwrap_or_default())
}

/// Melhor versão: release estável se houver, senão a mais nova (beta/alpha).
pub async fn best_version(
    client: &Client,
    project: &str,
    loaders: &[&str],
    mc_version: &str,
) -> Result<Option<Version>> {
    let versions = compatible_versions(client, project, loaders, mc_version).await?;
    let release = versions.iter().position(|v| v.version_type == "release");
    Ok(match release {
        Some(i) => versions.into_iter().nth(i),
        None => versions.into_iter().next(),
    })
}

pub async fn version(client: &Client, id: &str) -> Result<Version> {
    let url = format!("{API}/version/{}", urlencoding::encode(id));
    get_optional(client, &url).await?.ok_or_else(|| LauncherError::Other(format!("versão {id} não existe no Modrinth")))
}

pub async fn projects(client: &Client, ids: &[String]) -> Result<Vec<Project>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let url = format!("{API}/projects?ids={}", urlencoding::encode(&json_list(&refs)));
    let url = url.as_str();
    with_retry(3, || async move { Ok(check_status(client.get(url).send().await?)?.json::<Vec<Project>>().await?) })
        .await
}

/// Identifica arquivos locais pelo SHA-1 (retorna hash → versão).
pub async fn versions_by_hashes(client: &Client, hashes: &[String]) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let body = json!({ "hashes": hashes, "algorithm": "sha1" });
    let body = &body;
    with_retry(3, || async move {
        let r = client.post(format!("{API}/version_files")).json(body).send().await?;
        Ok(check_status(r)?.json::<HashMap<String, Version>>().await?)
    })
    .await
}

/// Última versão compatível para cada hash (usado para "verificar atualizações").
pub async fn latest_by_hashes(
    client: &Client,
    hashes: &[String],
    loaders: &[&str],
    mc_version: &str,
) -> Result<HashMap<String, Version>> {
    if hashes.is_empty() {
        return Ok(HashMap::new());
    }
    let body = json!({ "hashes": hashes, "algorithm": "sha1", "loaders": loaders, "game_versions": [mc_version] });
    let body = &body;
    with_retry(3, || async move {
        let r = client.post(format!("{API}/version_files/update")).json(body).send().await?;
        Ok(check_status(r)?.json::<HashMap<String, Version>>().await?)
    })
    .await
}
