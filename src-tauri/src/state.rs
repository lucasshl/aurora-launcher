use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use launcher_core::download::{Progress, ProgressSink};
use launcher_core::launch::{ExitInfo, GameProcess, LaunchObserver, LogLine};
use launcher_core::Launcher;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;

pub const EV_PROGRESS: &str = "launch://progress";
pub const EV_LOG: &str = "launch://log";
pub const EV_EXIT: &str = "launch://exit";

pub struct AppState {
    pub launcher: Arc<Launcher>,
    /// Instâncias com o jogo aberto.
    pub running: Mutex<HashMap<String, GameProcess>>,
    /// Instâncias em preparação (downloads), para impedir clique duplo em "Jogar".
    pub preparing: Mutex<HashSet<String>>,
}

impl AppState {
    pub fn new(launcher: Arc<Launcher>) -> Self {
        Self { launcher, running: Mutex::new(HashMap::new()), preparing: Mutex::new(HashSet::new()) }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent<'a> {
    instance_id: &'a str,
    #[serde(flatten)]
    progress: Progress,
}

/// Ponte core → frontend. Logs são agrupados a cada 50 ms para não inundar o IPC
/// quando um modpack imprime milhares de linhas por segundo.
pub struct TauriObserver {
    app: AppHandle,
    instance_id: String,
    log_tx: mpsc::UnboundedSender<LogLine>,
}

impl TauriObserver {
    pub fn new(app: AppHandle, instance_id: String) -> Arc<Self> {
        let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogLine>();
        let emitter = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut batch = Vec::with_capacity(256);
            loop {
                let Some(first) = log_rx.recv().await else { break };
                batch.push(first);
                tokio::time::sleep(Duration::from_millis(50)).await;
                while let Ok(line) = log_rx.try_recv() {
                    batch.push(line);
                    if batch.len() >= 2000 {
                        break;
                    }
                }
                let _ = emitter.emit(EV_LOG, std::mem::take(&mut batch));
            }
        });
        Arc::new(Self { app, instance_id, log_tx })
    }
}

impl ProgressSink for TauriObserver {
    fn progress(&self, progress: Progress) {
        let _ = self.app.emit(EV_PROGRESS, ProgressEvent { instance_id: &self.instance_id, progress });
    }
}

impl LaunchObserver for TauriObserver {
    fn log(&self, line: LogLine) {
        let _ = self.log_tx.send(line);
    }

    fn exited(&self, info: ExitInfo) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.running.lock().unwrap().remove(&info.instance_id);
        }
        // Pequeno atraso para o último lote de logs chegar antes do evento de saída
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(120)).await;
            let _ = app.emit(EV_EXIT, info);
        });
    }
}
