//! Guarda segredos (refresh_token da Microsoft) no cofre nativo do SO:
//! Windows Credential Manager, macOS Keychain ou kernel keyutils no Linux.
//!
//! O Credential Manager limita cada credencial a ~2560 bytes, e refresh tokens da
//! Microsoft podem passar disso, então o valor é dividido em blocos.

use keyring::Entry;

use crate::error::{LauncherError, Result};

const SERVICE: &str = "AuroraLauncher";
const CHUNK: usize = 1000;

fn entry(key: &str) -> Result<Entry> {
    Ok(Entry::new(SERVICE, key)?)
}

pub fn store_secret(account_id: &str, secret: &str) -> Result<()> {
    delete_secret(account_id).ok();
    let chunks: Vec<&str> = secret
        .as_bytes()
        .chunks(CHUNK)
        // tokens OAuth são ASCII, então o corte por bytes é seguro
        .map(|c| std::str::from_utf8(c).map_err(|_| LauncherError::SecureStore("segredo não-ASCII".into())))
        .collect::<Result<_>>()?;
    entry(&format!("{account_id}#count"))?.set_password(&chunks.len().to_string())?;
    for (i, chunk) in chunks.iter().enumerate() {
        entry(&format!("{account_id}#{i}"))?.set_password(chunk)?;
    }
    Ok(())
}

pub fn load_secret(account_id: &str) -> Result<Option<String>> {
    let count = match entry(&format!("{account_id}#count"))?.get_password() {
        Ok(c) => c.parse::<usize>().map_err(|_| LauncherError::SecureStore("contador corrompido".into()))?,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut out = String::new();
    for i in 0..count {
        out.push_str(&entry(&format!("{account_id}#{i}"))?.get_password()?);
    }
    Ok(Some(out))
}

pub fn delete_secret(account_id: &str) -> Result<()> {
    let count_entry = entry(&format!("{account_id}#count"))?;
    let count = match count_entry.get_password() {
        Ok(c) => c.parse::<usize>().unwrap_or(0),
        Err(keyring::Error::NoEntry) => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    for i in 0..count {
        let _ = entry(&format!("{account_id}#{i}"))?.delete_credential();
    }
    let _ = count_entry.delete_credential();
    Ok(())
}
