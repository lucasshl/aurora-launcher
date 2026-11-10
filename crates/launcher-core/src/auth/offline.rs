use md5::{Digest, Md5};
use uuid::Uuid;

use crate::error::{LauncherError, Result};

/// Equivalente exato de `java.util.UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(UTF_8))`,
/// que é o que servidores em `online-mode=false` usam para identificar o jogador.
///
/// Diferente do UUID v3 da RFC 4122, o Java **não** usa namespace: é MD5 direto dos bytes
/// com os bits de versão (3) e variante (IETF) ajustados.
pub fn offline_uuid(username: &str) -> Uuid {
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{username}").as_bytes()).into();
    hash[6] = (hash[6] & 0x0f) | 0x30; // versão 3
    hash[8] = (hash[8] & 0x3f) | 0x80; // variante IETF
    Uuid::from_bytes(hash)
}

/// Mesmas regras do jogo: 3–16 caracteres, apenas `[A-Za-z0-9_]`.
pub fn validate_username(username: &str) -> Result<()> {
    let len = username.chars().count();
    if !(3..=16).contains(&len) {
        return Err(LauncherError::Auth("o nick precisa ter entre 3 e 16 caracteres".into()));
    }
    if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(LauncherError::Auth("o nick só pode conter letras, números e _".into()));
    }
    Ok(())
}

/// Token "falso" para contas offline. Algumas versões recusam token vazio.
pub fn offline_access_token() -> String {
    Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_name_uuid_from_bytes() {
        // Valores conferidos com UUID.nameUUIDFromBytes("OfflinePlayer:Notch".getBytes(UTF_8))
        assert_eq!(offline_uuid("Notch").to_string(), "b50ad385-829d-3141-a216-7e7d7539ba7f");
        assert_eq!(offline_uuid("Notch").get_version_num(), 3);
    }

    #[test]
    fn deterministic() {
        assert_eq!(offline_uuid("Steve"), offline_uuid("Steve"));
        assert_ne!(offline_uuid("Steve"), offline_uuid("steve"));
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("Lucas_01").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("nome com espaço").is_err());
        assert!(validate_username("a_very_long_name_x").is_err());
    }
}
