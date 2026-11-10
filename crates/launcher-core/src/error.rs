use serde::{Serialize, Serializer};

/// Erro único do núcleo. Serializa como string legível para o frontend.
#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("Falha de rede: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Servidor respondeu HTTP {status} em {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("Erro de arquivo: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON inválido: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Arquivo corrompido: {path} (esperado {expected}, obtido {actual})")]
    HashMismatch { path: String, expected: String, actual: String },

    #[error("{count} download(s) falharam. Primeiro erro: {first}")]
    DownloadFailed { count: usize, first: String },

    #[error("Arquivo compactado inválido: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Autenticação: {0}")]
    Auth(String),

    #[error("Esta conta Microsoft não possui Minecraft: Java Edition")]
    NoEntitlement,

    #[error("Login cancelado")]
    Cancelled,

    #[error("Armazenamento seguro do sistema: {0}")]
    SecureStore(String),

    #[error("Versão não encontrada: {0}")]
    VersionNotFound(String),

    #[error("Java: {0}")]
    Java(String),

    #[error("Instância: {0}")]
    Instance(String),

    #[error("Modloader: {0}")]
    Loader(String),

    #[error("{0}")]
    Other(String),
}

impl LauncherError {
    /// Erros transitórios que valem nova tentativa (rede, 5xx, 429, hash corrompido em trânsito).
    pub fn is_retryable(&self) -> bool {
        match self {
            LauncherError::Network(e) => !e.is_builder() && !e.is_redirect(),
            LauncherError::HttpStatus { status, .. } => *status == 429 || *status >= 500,
            LauncherError::HashMismatch { .. } => true,
            LauncherError::Io(_) => true,
            _ => false,
        }
    }
}

impl From<keyring::Error> for LauncherError {
    fn from(e: keyring::Error) -> Self {
        LauncherError::SecureStore(e.to_string())
    }
}

impl From<tokio::task::JoinError> for LauncherError {
    fn from(e: tokio::task::JoinError) -> Self {
        LauncherError::Other(format!("tarefa interna falhou: {e}"))
    }
}

impl Serialize for LauncherError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LauncherError>;
