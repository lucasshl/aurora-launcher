//! Execução da JVM: subprocesso com stdout/stderr redirecionados para o console da UI,
//! ou desacoplado. Detecta crash reports ao final.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Serialize;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::oneshot;

use super::LaunchObserver;
use crate::error::{LauncherError, Result};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
    Launcher,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub instance_id: String,
    pub stream: LogStream,
    pub level: LogLevel,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitInfo {
    pub instance_id: String,
    pub code: Option<i32>,
    pub crashed: bool,
    pub crash_report: Option<PathBuf>,
}

pub fn detect_level(line: &str, stream: LogStream) -> LogLevel {
    let upper = line.to_ascii_uppercase();
    if upper.contains("/ERROR]")
        || upper.contains("/FATAL]")
        || upper.contains("EXCEPTION")
        || upper.starts_with("\tAT ")
    {
        LogLevel::Error
    } else if upper.contains("/WARN]") {
        LogLevel::Warn
    } else if upper.contains("/DEBUG]") {
        LogLevel::Debug
    } else if stream == LogStream::Stderr {
        LogLevel::Warn
    } else {
        LogLevel::Info
    }
}

/// Handle para encerrar o jogo pela UI.
pub struct GameProcess {
    pub pid: Option<u32>,
    kill: Option<oneshot::Sender<()>>,
    alive: Arc<std::sync::atomic::AtomicBool>,
}

impl GameProcess {
    /// `false` assim que o processo termina (mesmo que a UI ainda não tenha sido avisada).
    pub fn is_alive(&self) -> bool {
        self.alive.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn kill(&mut self) -> bool {
        self.kill.take().map(|tx| tx.send(()).is_ok()).unwrap_or(false)
    }
}

pub struct SpawnSpec {
    pub instance_id: String,
    pub java: PathBuf,
    pub java_major: u32,
    pub args: Vec<String>,
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub attach_console: bool,
}

/// No Windows a linha de comando é limitada a 32.767 caracteres; modpacks grandes estouram isso
/// pelo classpath. Java 9+ aceita `@arquivo` com os argumentos, então usamos quando necessário.
async fn maybe_argfile(spec: &SpawnSpec) -> Result<Vec<String>> {
    let total: usize = spec.args.iter().map(|a| a.len() + 3).sum();
    if !cfg!(windows) || total < 30_000 {
        return Ok(spec.args.clone());
    }
    if spec.java_major < 9 {
        return Err(LauncherError::Other(
            "linha de comando longa demais para Java 8 no Windows; reduza a quantidade de libraries".into(),
        ));
    }
    let file = spec.natives_dir.join("launch-args.txt");
    let body = spec
        .args
        .iter()
        .map(|a| format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<Vec<_>>()
        .join("\n");
    tokio::fs::write(&file, body).await?;
    Ok(vec![format!("@{}", file.display())])
}

pub async fn spawn_game(spec: SpawnSpec, observer: Arc<dyn LaunchObserver>) -> Result<GameProcess> {
    let args = maybe_argfile(&spec).await?;
    let mut cmd = Command::new(&spec.java);
    cmd.args(&args).current_dir(&spec.game_dir).stdin(Stdio::null()).kill_on_drop(false);

    if spec.attach_console {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
    }

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let flags = if spec.attach_console { CREATE_NO_WINDOW } else { DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP };
        cmd.creation_flags(flags);
    }

    let started = SystemTime::now();
    let mut child =
        cmd.spawn().map_err(|e| LauncherError::Java(format!("falha ao iniciar {}: {e}", spec.java.display())))?;
    let pid = child.id();

    let mut pumps = Vec::new();
    if let Some(out) = child.stdout.take() {
        pumps.push(tokio::spawn(pump(out, LogStream::Stdout, spec.instance_id.clone(), observer.clone())));
    }
    if let Some(err) = child.stderr.take() {
        pumps.push(tokio::spawn(pump(err, LogStream::Stderr, spec.instance_id.clone(), observer.clone())));
    }

    let (kill_tx, kill_rx) = oneshot::channel::<()>();
    let alive = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let alive_flag = alive.clone();
    let SpawnSpec { instance_id, game_dir, natives_dir, .. } = spec;

    tokio::spawn(async move {
        let status = tokio::select! {
            status = child.wait() => status.ok(),
            Ok(()) = kill_rx => {
                let _ = child.kill().await;
                child.wait().await.ok()
            }
        };
        alive_flag.store(false, std::sync::atomic::Ordering::SeqCst);
        // Espera terminar de ler stdout/stderr: a mensagem de erro costuma ser a última coisa impressa
        for p in pumps {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), p).await;
        }
        let code = status.and_then(|s| s.code());
        let crash_report = find_crash_report(&game_dir, started).await;
        let crashed = crash_report.is_some() || code.is_some_and(|c| c != 0);
        let secs = started.elapsed().map(|d| d.as_secs()).unwrap_or(0);
        let level = if crashed { LogLevel::Error } else { LogLevel::Info };
        let mut msg = format!(
            "Jogo encerrado após {secs}s (código {})",
            code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        );
        if crashed && secs < 20 {
            msg.push_str(" — fechou logo ao abrir: veja as linhas [stderr] acima ou o aurora-launcher.log");
        }
        observer.log(LogLine { instance_id: instance_id.clone(), stream: LogStream::Launcher, level, line: msg });

        crate::download::natives::cleanup(&natives_dir).await;
        observer.exited(ExitInfo { instance_id, code, crashed, crash_report });
    });

    Ok(GameProcess { pid, kill: Some(kill_tx), alive })
}

/// Lê linhas como bytes (a saída do jogo nem sempre é UTF-8 no Windows) e repassa para a UI.
async fn pump<R: AsyncRead + Unpin>(
    reader: R,
    stream: LogStream,
    instance_id: String,
    observer: Arc<dyn LaunchObserver>,
) {
    let mut reader = BufReader::new(reader);
    let mut buf = Vec::with_capacity(512);
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_owned();
                let level = detect_level(&line, stream);
                observer.log(LogLine { instance_id: instance_id.clone(), stream, level, line });
            }
        }
    }
}

/// Crash report criado depois do início (crash-reports/*.txt) ou log fatal da JVM (hs_err_pid*.log).
async fn find_crash_report(game_dir: &Path, since: SystemTime) -> Option<PathBuf> {
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for (dir, matcher) in [
        (game_dir.join("crash-reports"), (|n: &str| n.ends_with(".txt")) as fn(&str) -> bool),
        (game_dir.to_path_buf(), |n: &str| n.starts_with("hs_err_pid") && n.ends_with(".log")),
    ] {
        let Ok(mut rd) = tokio::fs::read_dir(&dir).await else { continue };
        while let Ok(Some(entry)) = rd.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !matcher(&name) {
                continue;
            }
            let Ok(modified) = entry.metadata().await.and_then(|m| m.modified()) else { continue };
            if modified >= since && newest.as_ref().is_none_or(|(t, _)| modified > *t) {
                newest = Some((modified, entry.path()));
            }
        }
    }
    newest.map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(detect_level("[12:00:00] [Render thread/WARN]: x", LogStream::Stdout), LogLevel::Warn);
        assert_eq!(detect_level("[12:00:00] [main/ERROR]: x", LogStream::Stdout), LogLevel::Error);
        assert_eq!(detect_level("[12:00:00] [main/INFO]: x", LogStream::Stdout), LogLevel::Info);
    }
}
