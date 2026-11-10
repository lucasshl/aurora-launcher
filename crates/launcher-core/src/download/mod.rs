//! Download concorrente com verificação de integridade.
//!
//! - Pool limitado por `buffer_unordered(concurrency)` sobre streams HTTP.
//! - Cada arquivo é gravado como `<arquivo>.part`, com hash calculado durante o
//!   streaming; só é renomeado para o nome final se o SHA-1/SHA-256 conferir.
//! - Falhas transitórias (rede, 5xx, 429, hash corrompido) têm retry com backoff.
//! - Arquivos já válidos no disco são pulados (re-download apenas do que está corrompido).

pub mod assets;
pub mod hash;
pub mod libraries;
pub mod natives;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::{stream, StreamExt};
use reqwest::Client;
use serde::Serialize;
use tokio::io::AsyncWriteExt;

use crate::error::{LauncherError, Result};
use crate::http::{check_status, with_retry};
pub use hash::Checksum;
use hash::{is_file_valid, StreamHasher};

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub path: PathBuf,
    pub checksum: Checksum,
    pub size: Option<u64>,
    /// Se `true`, um arquivo existente com o tamanho certo é aceito sem re-hash.
    pub fast_verify: bool,
}

impl DownloadTask {
    pub fn new(url: impl Into<String>, path: PathBuf, checksum: Checksum, size: Option<u64>) -> Self {
        Self { url: url.into(), path, checksum, size, fast_verify: false }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Manifest,
    Loader,
    Java,
    Client,
    Libraries,
    AssetIndex,
    Assets,
    Mods,
    Natives,
    Launching,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: Stage,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub current: Option<String>,
}

pub trait ProgressSink: Send + Sync {
    fn progress(&self, p: Progress);
}

/// Sink que descarta eventos (testes, CLI).
pub struct NoProgress;
impl ProgressSink for NoProgress {
    fn progress(&self, _: Progress) {}
}

#[derive(Clone)]
pub struct Downloader {
    client: Client,
    concurrency: usize,
    retries: u32,
}

impl Downloader {
    pub fn new(client: Client, concurrency: usize) -> Self {
        Self { client, concurrency: concurrency.clamp(1, 128), retries: 4 }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn download_one(&self, stage: Stage, task: DownloadTask, sink: &dyn ProgressSink) -> Result<()> {
        self.run(stage, vec![task], sink).await
    }

    /// Baixa todas as tarefas; retorna erro agregado se alguma falhar após os retries.
    pub async fn run(&self, stage: Stage, mut tasks: Vec<DownloadTask>, sink: &dyn ProgressSink) -> Result<()> {
        let mut seen = HashSet::new();
        tasks.retain(|t| seen.insert(t.path.clone()));

        let files_total = tasks.len() as u64;
        let bytes_total: u64 = tasks.iter().filter_map(|t| t.size).sum();
        let files_done = AtomicU64::new(0);
        let bytes_done = AtomicU64::new(0);
        let last_emit = Mutex::new(Instant::now() - Duration::from_secs(1));

        let emit = |current: Option<String>, force: bool| {
            let mut last = last_emit.lock().unwrap();
            if force || last.elapsed() >= Duration::from_millis(80) {
                *last = Instant::now();
                sink.progress(Progress {
                    stage,
                    files_done: files_done.load(Ordering::Relaxed),
                    files_total,
                    bytes_done: bytes_done.load(Ordering::Relaxed).min(bytes_total.max(1)),
                    bytes_total,
                    current,
                });
            }
        };
        emit(None, true);

        let errors: Vec<LauncherError> = stream::iter(tasks)
            .map(|task| {
                let bytes_done = &bytes_done;
                let files_done = &files_done;
                let emit = &emit;
                async move {
                    let name = task.path.file_name().map(|n| n.to_string_lossy().into_owned());
                    let result = self.fetch(&task, bytes_done).await;
                    files_done.fetch_add(1, Ordering::Relaxed);
                    emit(name, false);
                    result.map_err(|e| {
                        tracing::error!("falha ao baixar {}: {e}", task.url);
                        e
                    })
                }
            })
            .buffer_unordered(self.concurrency)
            .filter_map(|r| async move { r.err() })
            .collect()
            .await;

        emit(None, true);

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.into_iter().next().unwrap()),
            n => Err(LauncherError::DownloadFailed { count: n, first: errors[0].to_string() }),
        }
    }

    async fn fetch(&self, task: &DownloadTask, bytes_done: &AtomicU64) -> Result<()> {
        if is_file_valid(&task.path, &task.checksum, task.size, task.fast_verify).await {
            bytes_done.fetch_add(task.size.unwrap_or(0), Ordering::Relaxed);
            return Ok(());
        }
        with_retry(self.retries, || self.fetch_once(task, bytes_done)).await
    }

    async fn fetch_once(&self, task: &DownloadTask, bytes_done: &AtomicU64) -> Result<()> {
        if let Some(parent) = task.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let mut part = task.path.as_os_str().to_owned();
        part.push(".part");
        let part = PathBuf::from(part);

        let mut written: u64 = 0;
        let mut hasher = StreamHasher::new(&task.checksum);

        let result: Result<()> = async {
            let resp = check_status(self.client.get(&task.url).send().await?)?;
            let mut file = tokio::fs::File::create(&part).await?;
            let mut body = resp.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk?;
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
                written += chunk.len() as u64;
                bytes_done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
            }
            file.flush().await?;
            file.sync_all().await?;
            Ok(())
        }
        .await;

        let verdict = result.and_then(|_| {
            if let (Some(expected), Some(actual)) = (task.checksum.expected(), hasher.finish()) {
                if !actual.eq_ignore_ascii_case(expected) {
                    return Err(LauncherError::HashMismatch {
                        path: task.path.display().to_string(),
                        expected: expected.to_owned(),
                        actual,
                    });
                }
            } else if let Some(size) = task.size {
                if size != written {
                    return Err(LauncherError::HashMismatch {
                        path: task.path.display().to_string(),
                        expected: format!("{size} bytes"),
                        actual: format!("{written} bytes"),
                    });
                }
            }
            Ok(())
        });

        if let Err(e) = verdict {
            // Desfaz a contagem de bytes desta tentativa e apaga o parcial corrompido
            bytes_done.fetch_sub(written, Ordering::Relaxed);
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }

        tokio::fs::rename(&part, &task.path).await?;
        Ok(())
    }
}
