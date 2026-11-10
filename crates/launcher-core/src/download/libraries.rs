//! Planeja libraries: o que baixar, o que entra no classpath e quais JARs contêm nativos.

use std::collections::HashSet;
use std::path::PathBuf;

use super::{Checksum, DownloadTask};
use crate::paths::LauncherPaths;
use crate::version::model::{MavenCoord, VersionJson};
use crate::version::rules::{rules_allow, Environment};
use crate::version::ResolvedVersion;

const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

#[derive(Debug, Clone)]
pub struct NativeJar {
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

#[derive(Debug, Default)]
pub struct LibraryPlan {
    pub downloads: Vec<DownloadTask>,
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<NativeJar>,
    /// Libraries sem URL (geradas por instaladores, ex.: Forge) que precisam existir localmente.
    pub required_local: Vec<PathBuf>,
}

fn join_url(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

pub fn plan_libraries(paths: &LauncherPaths, version: &VersionJson, env: &Environment) -> LibraryPlan {
    let mut plan = LibraryPlan::default();
    let lib_root = paths.libraries();
    let mut cp_seen = HashSet::new();

    for lib in &version.libraries {
        if let Some(rules) = &lib.rules {
            if !rules_allow(rules, env) {
                continue;
            }
        }
        let coord = MavenCoord::parse(&lib.name);
        let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());

        // 1) Artefato principal (formato Mojang/Forge com `downloads.artifact`)
        if let Some(art) = artifact {
            let rel = art.path.clone().or_else(|| coord.as_ref().map(MavenCoord::path));
            if let Some(rel) = rel {
                let path = lib_root.join(&rel);
                if art.url.is_empty() {
                    plan.required_local.push(path.clone());
                } else {
                    plan.downloads.push(DownloadTask::new(
                        art.url.clone(),
                        path.clone(),
                        Checksum::sha1(art.sha1.as_deref()),
                        art.size,
                    ));
                }
                // LWJGL ≥ 3.3 publica nativos como artefatos com classificador `natives-*`
                let is_native =
                    coord.as_ref().and_then(|c| c.classifier.as_deref()).is_some_and(|c| c.starts_with("natives-"));
                if is_native {
                    plan.natives.push(NativeJar {
                        path: path.clone(),
                        exclude: lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default(),
                    });
                }
                if cp_seen.insert(path.clone()) {
                    plan.classpath.push(path);
                }
            }
        } else if lib.natives.is_none() {
            // 2) Formato Maven puro (Fabric/Quilt): `name` + `url` do repositório
            if let Some(coord) = &coord {
                let rel = coord.path();
                let path = lib_root.join(&rel);
                let base = lib.url.as_deref().unwrap_or(MOJANG_LIBRARIES);
                plan.downloads.push(DownloadTask::new(
                    join_url(base, &rel),
                    path.clone(),
                    Checksum::sha1(lib.sha1.as_deref()),
                    lib.size,
                ));
                if cp_seen.insert(path.clone()) {
                    plan.classpath.push(path);
                }
            }
        }

        // 3) Nativos no formato antigo (`natives` + `downloads.classifiers`, ≤ 1.18)
        if let (Some(natives), Some(classifiers)) =
            (&lib.natives, lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()))
        {
            if let Some(classifier) = natives.get(env.os_name) {
                let classifier = classifier.replace("${arch}", if env.is_64bit() { "64" } else { "32" });
                if let Some(art) = classifiers.get(&classifier) {
                    let rel = art.path.clone().or_else(|| {
                        coord.as_ref().map(|c| MavenCoord { classifier: Some(classifier.clone()), ..c.clone() }.path())
                    });
                    if let Some(rel) = rel {
                        let path = lib_root.join(rel);
                        plan.downloads.push(DownloadTask::new(
                            art.url.clone(),
                            path.clone(),
                            Checksum::sha1(art.sha1.as_deref()),
                            art.size,
                        ));
                        plan.natives.push(NativeJar {
                            path,
                            exclude: lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default(),
                        });
                    }
                }
            }
        }
    }
    plan
}

/// Tarefa do client JAR (`versions/<id>/<id>.jar`).
pub fn client_jar_task(paths: &LauncherPaths, resolved: &ResolvedVersion) -> Option<DownloadTask> {
    let client = resolved.json.downloads.as_ref()?.client.as_ref()?;
    Some(DownloadTask::new(
        client.url.clone(),
        paths.version_jar(&resolved.client_jar_id),
        Checksum::sha1(client.sha1.as_deref()),
        client.size,
    ))
}
