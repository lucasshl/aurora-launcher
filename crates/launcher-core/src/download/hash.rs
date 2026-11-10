use std::io::Read;
use std::path::{Path, PathBuf};

use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    Sha1(String),
    Sha256(String),
    None,
}

impl Checksum {
    pub fn sha1(opt: Option<&str>) -> Self {
        match opt {
            Some(s) if !s.is_empty() => Checksum::Sha1(s.to_ascii_lowercase()),
            _ => Checksum::None,
        }
    }

    pub fn expected(&self) -> Option<&str> {
        match self {
            Checksum::Sha1(s) | Checksum::Sha256(s) => Some(s),
            Checksum::None => None,
        }
    }
}

/// Hasher incremental usado durante o streaming do download.
pub enum StreamHasher {
    Sha1(Sha1),
    Sha256(Sha256),
    None,
}

impl StreamHasher {
    pub fn new(c: &Checksum) -> Self {
        match c {
            Checksum::Sha1(_) => StreamHasher::Sha1(Sha1::new()),
            Checksum::Sha256(_) => StreamHasher::Sha256(Sha256::new()),
            Checksum::None => StreamHasher::None,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        match self {
            StreamHasher::Sha1(h) => h.update(data),
            StreamHasher::Sha256(h) => h.update(data),
            StreamHasher::None => {}
        }
    }

    pub fn finish(self) -> Option<String> {
        match self {
            StreamHasher::Sha1(h) => Some(hex::encode(h.finalize())),
            StreamHasher::Sha256(h) => Some(hex::encode(h.finalize())),
            StreamHasher::None => None,
        }
    }
}

pub fn sha1_bytes(data: &[u8]) -> String {
    hex::encode(Sha1::digest(data))
}

fn hash_file_blocking(path: &Path, checksum: &Checksum) -> std::io::Result<Option<String>> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = StreamHasher::new(checksum);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finish())
}

pub async fn sha1_file(path: &Path) -> Result<String> {
    let path: PathBuf = path.to_owned();
    let out = tokio::task::spawn_blocking(move || hash_file_blocking(&path, &Checksum::Sha1(String::new()))).await??;
    Ok(out.unwrap_or_default())
}

/// Verifica um arquivo já existente. `fast` confia no tamanho (usado em assets,
/// cujo nome do arquivo já é o próprio hash).
pub async fn is_file_valid(path: &Path, checksum: &Checksum, size: Option<u64>, fast: bool) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else { return false };
    if !meta.is_file() {
        return false;
    }
    if let Some(size) = size {
        if meta.len() != size {
            return false;
        }
    }
    let Some(expected) = checksum.expected() else { return true };
    if fast && size.is_some() {
        return true;
    }
    let path = path.to_owned();
    let checksum = checksum.clone();
    match tokio::task::spawn_blocking(move || hash_file_blocking(&path, &checksum)).await {
        Ok(Ok(Some(actual))) => actual.eq_ignore_ascii_case(expected),
        _ => false,
    }
}
