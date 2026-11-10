//! Asset index + objetos (sons, idiomas, texturas extras).

use std::path::{Path, PathBuf};

use super::{Checksum, DownloadTask, Downloader, ProgressSink, Stage};
use crate::error::{LauncherError, Result};
use crate::paths::LauncherPaths;
use crate::version::model::{AssetIndex, AssetIndexRef};

const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

pub struct AssetPlan {
    pub index_id: String,
    pub index: AssetIndex,
    pub tasks: Vec<DownloadTask>,
}

pub async fn prepare_assets(
    paths: &LauncherPaths,
    downloader: &Downloader,
    index_ref: &AssetIndexRef,
    sink: &dyn ProgressSink,
) -> Result<AssetPlan> {
    let index_path = paths.asset_indexes().join(format!("{}.json", index_ref.id));
    let task = DownloadTask::new(
        index_ref.url.clone(),
        index_path.clone(),
        Checksum::sha1(Some(&index_ref.sha1)),
        Some(index_ref.size),
    );
    downloader.download_one(Stage::AssetIndex, task, sink).await?;

    let bytes = tokio::fs::read(&index_path).await?;
    let index: AssetIndex = serde_json::from_slice(&bytes)
        .map_err(|e| LauncherError::Other(format!("asset index {} ilegível: {e}", index_ref.id)))?;

    let objects = paths.asset_objects();
    let tasks = index
        .objects
        .values()
        .map(|obj| {
            let prefix = &obj.hash[..2];
            let mut t = DownloadTask::new(
                format!("{RESOURCES_URL}/{prefix}/{}", obj.hash),
                objects.join(prefix).join(&obj.hash),
                Checksum::sha1(Some(&obj.hash)),
                Some(obj.size),
            );
            t.fast_verify = true; // nome do arquivo = hash
            t
        })
        .collect();

    Ok(AssetPlan { index_id: index_ref.id.clone(), index, tasks })
}

/// Versões antigas (≤ 1.7.2 `virtual`, pré-1.6 `map_to_resources`) esperam os assets
/// com nomes legíveis. Retorna o diretório para `${game_assets}` quando aplicável.
pub async fn materialize_legacy_assets(
    paths: &LauncherPaths,
    plan: &AssetPlan,
    game_dir: &Path,
) -> Result<Option<PathBuf>> {
    let target = if plan.index.map_to_resources {
        game_dir.join("resources")
    } else if plan.index.is_virtual {
        paths.assets().join("virtual").join(&plan.index_id)
    } else {
        return Ok(None);
    };

    let objects = paths.asset_objects();
    let entries: Vec<(PathBuf, PathBuf)> = plan
        .index
        .objects
        .iter()
        .map(|(name, obj)| (objects.join(&obj.hash[..2]).join(&obj.hash), target.join(name)))
        .collect();

    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        for (src, dst) in entries {
            if dst.exists() {
                continue;
            }
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dst)?;
        }
        Ok(())
    })
    .await??;

    Ok(Some(target))
}
