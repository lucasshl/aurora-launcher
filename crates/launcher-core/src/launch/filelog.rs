//! Espelha tudo o que vai para o console da UI em `instances/<id>/logs/aurora-launcher.log`:
//! linhas do launcher, comando da JVM (com token ocultado) e stdout/stderr do jogo.
//! Assim dá para diagnosticar mesmo quando o jogo fecha antes de criar o `latest.log`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{ExitInfo, LaunchObserver, LogLine, LogStream};
use crate::download::{Progress, ProgressSink};

pub struct FileTee {
    inner: Arc<dyn LaunchObserver>,
    file: Mutex<Option<BufWriter<File>>>,
    pub path: PathBuf,
}

impl FileTee {
    pub fn new(inner: Arc<dyn LaunchObserver>, instance_dir: &Path) -> Arc<Self> {
        let dir = instance_dir.join("logs");
        let path = dir.join("aurora-launcher.log");
        let file = std::fs::create_dir_all(&dir)
            .and_then(|_| {
                // Mantém a execução anterior como .1 para comparação
                let _ = std::fs::rename(&path, dir.join("aurora-launcher.1.log"));
                File::create(&path)
            })
            .map(BufWriter::new)
            .map_err(|e| tracing::warn!("não foi possível criar {}: {e}", path.display()))
            .ok();
        Arc::new(Self { inner, file: Mutex::new(file), path })
    }

    pub fn write(&self, text: &str) {
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = writeln!(f, "{text}");
        }
    }

    /// Fecha o arquivo quando a preparação falha antes de existir um processo.
    pub fn exited_without_process(&self) {
        self.flush();
    }

    fn flush(&self) {
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.flush();
        }
    }
}

impl ProgressSink for FileTee {
    fn progress(&self, p: Progress) {
        self.inner.progress(p);
    }
}

impl LaunchObserver for FileTee {
    fn log(&self, line: LogLine) {
        let tag = match line.stream {
            LogStream::Launcher => "[launcher]",
            LogStream::Stdout => "[stdout]",
            LogStream::Stderr => "[stderr]",
        };
        self.write(&format!("{tag} {}", line.line));
        // Grava no disco a cada linha: se o launcher ou o jogo caírem, nada se perde
        self.flush();
        self.inner.log(line);
    }

    fn exited(&self, info: ExitInfo) {
        self.write(&format!(
            "[launcher] processo encerrado: código {:?}, crash={}, report={:?}",
            info.code, info.crashed, info.crash_report
        ));
        self.flush();
        self.inner.exited(info);
    }
}
