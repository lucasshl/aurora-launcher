//! Gerenciador de conteúdo da instância: mods, resource packs e shaders.
//!
//! - Lista arquivos instalados lendo metadados de dentro do JAR
//!   (`fabric.mod.json`, `quilt.mod.json`, `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`).
//! - Ativa/desativa renomeando para `.disabled` (convenção de todos os launchers).
//! - Instala do Modrinth com resolução recursiva de dependências obrigatórias.
//! - Identifica arquivos por SHA-1 no Modrinth e verifica atualizações.
//!
//! `instances/<id>/content-index.json` guarda o que o launcher instalou (projeto → arquivo),
//! para atualizar sem deixar versões duplicadas e sem tocar em arquivos do usuário.

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use base64::Engine;
use futures::{stream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::download::hash::sha1_file;
use crate::download::{Checksum, DownloadTask, Downloader, ProgressSink, Stage};
use crate::error::{LauncherError, Result};
use crate::instance::InstanceConfig;
use crate::modrinth::{self, ContentKind, Version};
use crate::paths::{write_atomic, LauncherPaths};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModMeta {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    /// Ícone embutido no JAR como data URL (≤ 256 KB).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledContent {
    /// Nome do arquivo sem o sufixo `.disabled`.
    pub file_name: String,
    pub enabled: bool,
    pub kind: ContentKind,
    pub size: u64,
    pub meta: ModMeta,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub managed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub project_id: String,
    pub version_id: String,
    pub file_name: String,
    pub title: Option<String>,
    /// `true` se foi instalado só como dependência de outro.
    #[serde(default)]
    pub dependency: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ContentIndex {
    #[serde(default)]
    entries: HashMap<ContentKind, HashMap<String, IndexEntry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub file_name: String,
    pub project_id: String,
    pub current_version: String,
    pub latest: Version,
}

const DISABLED: &str = ".disabled";

fn index_path(paths: &LauncherPaths, instance_id: &str) -> PathBuf {
    paths.instance_dir(instance_id).join("content-index.json")
}

async fn load_index(paths: &LauncherPaths, instance_id: &str) -> ContentIndex {
    match tokio::fs::read(index_path(paths, instance_id)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => ContentIndex::default(),
    }
}

async fn save_index(paths: &LauncherPaths, instance_id: &str, index: &ContentIndex) -> Result<()> {
    write_atomic(&index_path(paths, instance_id), &serde_json::to_vec_pretty(index)?).await
}

pub fn content_dir(paths: &LauncherPaths, instance_id: &str, kind: ContentKind) -> PathBuf {
    paths.instance_game_dir(instance_id).join(kind.dir())
}

fn validate_file_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') || name.contains("..") {
        return Err(LauncherError::Instance(format!("nome de arquivo inválido: {name}")));
    }
    Ok(())
}

// ---------------------------------------------------------------- metadados

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, limit: u64) -> Option<Vec<u8>> {
    let entry = zip.by_name(name).ok()?;
    if entry.size() > limit {
        return None;
    }
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.take(limit).read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn icon_data_url(zip: &mut zip::ZipArchive<std::fs::File>, path: &str) -> Option<String> {
    let path = path.trim_start_matches('/');
    let bytes = read_entry(zip, path, 256 * 1024)?;
    if !bytes.starts_with(b"\x89PNG") {
        return None;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn authors_from_json(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_owned).or_else(|| x.get("name")?.as_str().map(str::to_owned)))
                .collect()
        })
        .unwrap_or_default()
}

/// Lê metadados de um JAR de mod (Fabric, Quilt, Forge, NeoForge).
pub fn read_mod_meta(path: &Path) -> ModMeta {
    let Ok(file) = std::fs::File::open(path) else { return ModMeta::default() };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return ModMeta::default() };

    if let Some(bytes) = read_entry(&mut zip, "fabric.mod.json", 1 << 20) {
        // Alguns mods trazem quebras de linha cruas dentro de strings; limpa antes de parsear
        let text = String::from_utf8_lossy(&bytes).replace(['\n', '\r', '\t'], " ");
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            let icon_path = match v.get("icon") {
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(serde_json::Value::Object(m)) => {
                    // Mapa tamanho → caminho: pega o maior
                    m.iter()
                        .max_by_key(|(k, _)| k.parse::<u32>().unwrap_or(0))
                        .and_then(|(_, p)| p.as_str().map(str::to_owned))
                }
                _ => None,
            };
            return ModMeta {
                id: v.get("id").and_then(|x| x.as_str()).map(str::to_owned),
                name: v.get("name").and_then(|x| x.as_str()).map(str::to_owned),
                version: v.get("version").and_then(|x| x.as_str()).map(str::to_owned),
                description: v.get("description").and_then(|x| x.as_str()).map(str::to_owned),
                authors: authors_from_json(v.get("authors")),
                icon: icon_path.and_then(|p| icon_data_url(&mut zip, &p)),
            };
        }
    }

    if let Some(bytes) = read_entry(&mut zip, "quilt.mod.json", 1 << 20) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            let ql = v.get("quilt_loader");
            let md = ql.and_then(|q| q.get("metadata"));
            let icon = md.and_then(|m| m.get("icon")).and_then(|i| i.as_str()).map(str::to_owned);
            return ModMeta {
                id: ql.and_then(|q| q.get("id")).and_then(|x| x.as_str()).map(str::to_owned),
                name: md.and_then(|m| m.get("name")).and_then(|x| x.as_str()).map(str::to_owned),
                version: ql.and_then(|q| q.get("version")).and_then(|x| x.as_str()).map(str::to_owned),
                description: md.and_then(|m| m.get("description")).and_then(|x| x.as_str()).map(str::to_owned),
                authors: Vec::new(),
                icon: icon.and_then(|p| icon_data_url(&mut zip, &p)),
            };
        }
    }

    for toml_name in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        let Some(bytes) = read_entry(&mut zip, toml_name, 1 << 20) else { continue };
        let Ok(doc) = toml::from_str::<toml::Value>(&String::from_utf8_lossy(&bytes)) else { continue };
        let Some(m) = doc.get("mods").and_then(|m| m.as_array()).and_then(|a| a.first()) else { continue };
        let s = |k: &str| m.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        let mut version = s("version");
        if version.as_deref().is_some_and(|v| v.contains("${file.jarVersion}")) {
            version = read_entry(&mut zip, "META-INF/MANIFEST.MF", 64 * 1024).and_then(|mf| {
                String::from_utf8_lossy(&mf)
                    .lines()
                    .find_map(|l| l.strip_prefix("Implementation-Version:").map(|v| v.trim().to_owned()))
            });
        }
        let logo = s("logoFile").or_else(|| doc.get("logoFile").and_then(|x| x.as_str()).map(str::to_owned));
        return ModMeta {
            id: s("modId"),
            name: s("displayName"),
            version,
            description: s("description").map(|d| d.trim().to_owned()),
            authors: s("authors").map(|a| vec![a]).unwrap_or_default(),
            icon: logo.and_then(|p| icon_data_url(&mut zip, &p)),
        };
    }
    ModMeta::default()
}

// ---------------------------------------------------------------- listagem

pub async fn list_installed(
    paths: &LauncherPaths,
    instance_id: &str,
    kind: ContentKind,
) -> Result<Vec<InstalledContent>> {
    let dir = content_dir(paths, instance_id, kind);
    tokio::fs::create_dir_all(&dir).await?;
    let index = load_index(paths, instance_id).await;
    let by_file: HashMap<String, IndexEntry> = index
        .entries
        .get(&kind)
        .map(|m| m.values().map(|e| (e.file_name.clone(), e.clone())).collect())
        .unwrap_or_default();

    let mut files = Vec::new();
    let mut rd = tokio::fs::read_dir(&dir).await?;
    while let Some(entry) = rd.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let raw = entry.file_name().to_string_lossy().into_owned();
        let (name, enabled) = match raw.strip_suffix(DISABLED) {
            Some(n) => (n.to_owned(), false),
            None => (raw.clone(), true),
        };
        let ext_ok = kind.extensions().iter().any(|e| name.to_ascii_lowercase().ends_with(&format!(".{e}")));
        if !ext_ok {
            continue;
        }
        let size = entry.metadata().await.map(|m| m.len()).unwrap_or(0);
        files.push((name, enabled, entry.path(), size));
    }

    let parsed: Vec<InstalledContent> = tokio::task::spawn_blocking(move || {
        files
            .into_iter()
            .map(|(name, enabled, path, size)| {
                let meta = if kind == ContentKind::Mod { read_mod_meta(&path) } else { ModMeta::default() };
                let idx = by_file.get(&name);
                InstalledContent {
                    meta: ModMeta { name: meta.name.clone().or_else(|| idx.and_then(|e| e.title.clone())), ..meta },
                    project_id: idx.map(|e| e.project_id.clone()),
                    version_id: idx.map(|e| e.version_id.clone()),
                    managed: idx.is_some(),
                    file_name: name,
                    enabled,
                    kind,
                    size,
                }
            })
            .collect()
    })
    .await?;

    let mut parsed = parsed;
    parsed.sort_by_key(|c| c.meta.name.clone().unwrap_or_else(|| c.file_name.clone()).to_lowercase());
    Ok(parsed)
}

pub async fn set_enabled(
    paths: &LauncherPaths,
    instance_id: &str,
    kind: ContentKind,
    file_name: &str,
    enabled: bool,
) -> Result<()> {
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind);
    let on = dir.join(file_name);
    let off = dir.join(format!("{file_name}{DISABLED}"));
    let (from, to) = if enabled { (off, on) } else { (on, off) };
    if tokio::fs::try_exists(&from).await? {
        tokio::fs::rename(from, to).await?;
    }
    Ok(())
}

pub async fn remove(paths: &LauncherPaths, instance_id: &str, kind: ContentKind, file_name: &str) -> Result<()> {
    validate_file_name(file_name)?;
    let dir = content_dir(paths, instance_id, kind);
    for candidate in [dir.join(file_name), dir.join(format!("{file_name}{DISABLED}"))] {
        if tokio::fs::try_exists(&candidate).await? {
            tokio::fs::remove_file(candidate).await?;
        }
    }
    let mut index = load_index(paths, instance_id).await;
    if let Some(map) = index.entries.get_mut(&kind) {
        map.retain(|_, e| e.file_name != file_name);
    }
    save_index(paths, instance_id, &index).await
}

// ---------------------------------------------------------------- instalação

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
}

/// Instala um ou mais projetos do Modrinth (com dependências obrigatórias, para mods).
/// `pinned` permite escolher uma versão específica de um projeto.
pub async fn install_projects(
    paths: &LauncherPaths,
    downloader: &Downloader,
    instance: &InstanceConfig,
    kind: ContentKind,
    projects: &[String],
    pinned: Option<(&str, &str)>,
    sink: &dyn ProgressSink,
) -> Result<InstallReport> {
    let client = downloader.client();
    let loaders = kind.loaders(instance.loader);
    let mc = instance.minecraft_version.as_str();
    let dir = content_dir(paths, &instance.id, kind);
    tokio::fs::create_dir_all(&dir).await?;

    let mut index = load_index(paths, &instance.id).await;
    let installed_projects: HashSet<String> =
        index.entries.get(&kind).map(|m| m.keys().cloned().collect()).unwrap_or_default();

    // Resolve versões (nível a nível, em paralelo) incluindo dependências
    let mut resolved: HashMap<String, (Version, bool)> = HashMap::new();
    let mut skipped = Vec::new();
    let mut frontier: Vec<(String, bool)> = projects.iter().map(|p| (p.clone(), false)).collect();
    let mut seen: HashSet<String> = HashSet::new();

    while !frontier.is_empty() {
        let batch: Vec<(String, bool)> = frontier.drain(..).filter(|(p, _)| seen.insert(p.clone())).collect();
        let pinned_owned = pinned.map(|(p, v)| (p.to_owned(), v.to_owned()));
        let futures = batch.into_iter().map(|(project, is_dep)| {
            let (client, loaders, mc, pinned) = (client.clone(), loaders.clone(), mc.to_owned(), pinned_owned.clone());
            async move {
                let v = match pinned {
                    Some((p, vid)) if p == project => modrinth::version(&client, &vid).await.map(Some),
                    _ => modrinth::best_version(&client, &project, &loaders, &mc).await,
                };
                (project, is_dep, v)
            }
        });
        let futures: Vec<_> = futures.collect();
        let results: Vec<(String, bool, Result<Option<Version>>)> =
            stream::iter(futures).buffer_unordered(8).collect().await;

        for (project, is_dep, result) in results {
            match result? {
                None => {
                    if !is_dep {
                        skipped.push(project);
                    }
                }
                Some(v) => {
                    if kind == ContentKind::Mod {
                        for dep in v.dependencies.iter().filter(|d| d.dependency_type == "required") {
                            if let Some(pid) = &dep.project_id {
                                // Dependência já instalada pelo launcher? Mantém a existente.
                                if !installed_projects.contains(pid) && !seen.contains(pid) {
                                    frontier.push((pid.clone(), true));
                                }
                            }
                        }
                    }
                    if seen.contains(&v.project_id) && v.project_id != project && resolved.contains_key(&v.project_id) {
                        continue;
                    }
                    seen.insert(v.project_id.clone());
                    resolved.insert(v.project_id.clone(), (v, is_dep));
                }
            }
        }
    }

    // Monta downloads e remove versões antigas dos mesmos projetos
    let kind_map = index.entries.entry(kind).or_default();
    let mut tasks = Vec::new();
    let mut installed = Vec::new();
    let titles: HashMap<String, String> = modrinth::projects(client, &resolved.keys().cloned().collect::<Vec<_>>())
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id, p.title))
        .collect();

    for (project_id, (version, is_dep)) in &resolved {
        let Some(file) = version.primary_file() else { continue };
        validate_file_name(&file.filename)?;
        if let Some(old) = kind_map.get(project_id) {
            if old.file_name != file.filename {
                let _ = tokio::fs::remove_file(dir.join(&old.file_name)).await;
                let _ = tokio::fs::remove_file(dir.join(format!("{}{DISABLED}", old.file_name))).await;
            }
        }
        tasks.push(DownloadTask::new(
            file.url.clone(),
            dir.join(&file.filename),
            Checksum::sha1(Some(&file.hashes.sha1)),
            Some(file.size),
        ));
        let was_explicit = kind_map.get(project_id).is_some_and(|e| !e.dependency);
        kind_map.insert(
            project_id.clone(),
            IndexEntry {
                project_id: project_id.clone(),
                version_id: version.id.clone(),
                file_name: file.filename.clone(),
                title: titles.get(project_id).cloned(),
                dependency: *is_dep && !was_explicit,
            },
        );
        installed.push(titles.get(project_id).cloned().unwrap_or_else(|| file.filename.clone()));
    }

    downloader.run(Stage::Mods, tasks, sink).await?;
    save_index(paths, &instance.id, &index).await?;
    installed.sort();
    Ok(InstallReport { installed, skipped })
}

/// Verifica atualizações de todos os arquivos (gerenciados ou não) pelo SHA-1.
pub async fn check_updates(
    client: &reqwest::Client,
    paths: &LauncherPaths,
    instance: &InstanceConfig,
    kind: ContentKind,
) -> Result<Vec<UpdateInfo>> {
    let dir = content_dir(paths, &instance.id, kind);
    let items = list_installed(paths, &instance.id, kind).await?;

    let mut by_hash: HashMap<String, InstalledContent> = HashMap::new();
    for item in items {
        let path =
            if item.enabled { dir.join(&item.file_name) } else { dir.join(format!("{}{DISABLED}", item.file_name)) };
        if let Ok(h) = sha1_file(&path).await {
            by_hash.insert(h, item);
        }
    }
    let hashes: Vec<String> = by_hash.keys().cloned().collect();
    let loaders = kind.loaders(instance.loader);
    let current = modrinth::versions_by_hashes(client, &hashes).await?;
    let latest = modrinth::latest_by_hashes(client, &hashes, &loaders, &instance.minecraft_version).await?;

    let mut out = Vec::new();
    for (hash, item) in by_hash {
        let (Some(cur), Some(new)) = (current.get(&hash), latest.get(&hash)) else { continue };
        if cur.id != new.id {
            out.push(UpdateInfo {
                file_name: item.file_name,
                project_id: new.project_id.clone(),
                current_version: cur.version_number.clone(),
                latest: new.clone(),
            });
        }
    }
    out.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(out)
}

/// Aplica uma atualização: baixa a nova versão e remove o arquivo antigo.
pub async fn apply_update(
    paths: &LauncherPaths,
    downloader: &Downloader,
    instance: &InstanceConfig,
    kind: ContentKind,
    update: &UpdateInfo,
    sink: &dyn ProgressSink,
) -> Result<()> {
    validate_file_name(&update.file_name)?;
    let dir = content_dir(paths, &instance.id, kind);
    let was_enabled = tokio::fs::try_exists(dir.join(&update.file_name)).await.unwrap_or(false);
    install_projects(
        paths,
        downloader,
        instance,
        kind,
        std::slice::from_ref(&update.project_id),
        Some((&update.project_id, &update.latest.id)),
        sink,
    )
    .await?;
    let new_name = update.latest.primary_file().map(|f| f.filename.clone()).unwrap_or_default();
    if new_name != update.file_name {
        let _ = tokio::fs::remove_file(dir.join(&update.file_name)).await;
        let _ = tokio::fs::remove_file(dir.join(format!("{}{DISABLED}", update.file_name))).await;
    }
    if !was_enabled && !new_name.is_empty() {
        set_enabled(paths, &instance.id, kind, &new_name, false).await?;
    }
    Ok(())
}

/// IDs de projeto instalados pelo launcher nesta instância.
pub async fn managed_project_ids(paths: &LauncherPaths, instance_id: &str, kind: ContentKind) -> HashSet<String> {
    load_index(paths, instance_id).await.entries.get(&kind).map(|m| m.keys().cloned().collect()).unwrap_or_default()
}
