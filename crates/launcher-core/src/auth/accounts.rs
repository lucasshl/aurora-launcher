//! Persistência e troca rápida de contas.
//!
//! `accounts.json` guarda apenas metadados públicos (nick, UUID, tipo, skin).
//! O refresh_token Microsoft fica no cofre do SO (`secure_store`) e o
//! access_token da Mojang só existe em memória, sendo renovado quando expira.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use super::microsoft::{self, DeviceCode, MinecraftSession};
use super::{offline, secure_store, LaunchAuth};
use crate::error::{LauncherError, Result};
use crate::paths::{write_atomic, LauncherPaths};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Microsoft,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// UUID com hífens (perfil Mojang ou UUID offline).
    pub id: String,
    pub username: String,
    pub kind: AccountKind,
    pub skin_url: Option<String>,
    pub added_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsView {
    pub active: Option<String>,
    pub accounts: Vec<Account>,
}

#[derive(Clone)]
struct CachedSession {
    access_token: String,
    expires_at: u64,
    xuid: Option<String>,
}

pub struct AccountStore {
    file: PathBuf,
    data: Mutex<AccountsView>,
    sessions: Mutex<HashMap<String, CachedSession>>,
    pending_logins: Mutex<HashMap<String, CancellationToken>>,
}

fn secret_key(id: &str) -> String {
    format!("ms:{id}")
}

impl AccountStore {
    pub async fn load(paths: &LauncherPaths) -> Result<Self> {
        let file = paths.accounts_file();
        let data = match tokio::fs::read(&file).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                // Não perde o arquivo: guarda uma cópia para recuperação manual
                tracing::error!("accounts.json corrompido ({e}); criando backup e começando vazio");
                let _ = std::fs::copy(&file, file.with_extension("json.corrupt"));
                AccountsView::default()
            }),
            Err(_) => AccountsView::default(),
        };
        Ok(Self {
            file,
            data: Mutex::new(data),
            sessions: Mutex::new(HashMap::new()),
            pending_logins: Mutex::new(HashMap::new()),
        })
    }

    async fn persist(&self) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(&*self.data.lock().unwrap())?;
        write_atomic(&self.file, &bytes).await
    }

    pub fn list(&self) -> AccountsView {
        self.data.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<Account> {
        self.data.lock().unwrap().accounts.iter().find(|a| a.id == id).cloned()
    }

    pub fn active(&self) -> Option<Account> {
        let data = self.data.lock().unwrap();
        let id = data.active.as_ref()?;
        data.accounts.iter().find(|a| &a.id == id).cloned()
    }

    pub async fn set_active(&self, id: &str) -> Result<AccountsView> {
        {
            let mut data = self.data.lock().unwrap();
            if !data.accounts.iter().any(|a| a.id == id) {
                return Err(LauncherError::Auth("conta não encontrada".into()));
            }
            data.active = Some(id.to_owned());
        }
        self.persist().await?;
        Ok(self.list())
    }

    fn upsert(&self, account: Account) {
        let mut data = self.data.lock().unwrap();
        if let Some(existing) = data.accounts.iter_mut().find(|a| a.id == account.id) {
            existing.username = account.username;
            existing.skin_url = account.skin_url;
        } else {
            data.accounts.push(account.clone());
        }
        data.active = Some(account.id);
    }

    // ------------------------------------------------------------------ offline

    pub async fn add_offline(&self, username: &str) -> Result<Account> {
        let username = username.trim();
        offline::validate_username(username)?;
        let account = Account {
            id: offline::offline_uuid(username).hyphenated().to_string(),
            username: username.to_owned(),
            kind: AccountKind::Offline,
            skin_url: None,
            added_at: microsoft::now_secs(),
        };
        self.upsert(account.clone());
        self.persist().await?;
        Ok(account)
    }

    // ---------------------------------------------------------------- microsoft

    /// Passo 1: pede o código que o usuário digita em microsoft.com/link.
    pub async fn begin_microsoft(&self, client: &Client, client_id: &str) -> Result<DeviceCode> {
        let code = microsoft::request_device_code(client, client_id).await?;
        self.pending_logins.lock().unwrap().insert(code.device_code.clone(), CancellationToken::new());
        Ok(code)
    }

    /// Passo 2: aguarda o usuário concluir no navegador e finaliza a cadeia XBL/XSTS/Mojang.
    pub async fn finish_microsoft(&self, client: &Client, client_id: &str, code: &DeviceCode) -> Result<Account> {
        let cancel = self.pending_logins.lock().unwrap().get(&code.device_code).cloned().unwrap_or_default();

        let result = async {
            let ms = microsoft::poll_device_code(client, client_id, code, &cancel).await?;
            microsoft::complete_login(client, ms, None).await
        }
        .await;
        self.pending_logins.lock().unwrap().remove(&code.device_code);

        let session = result?;
        self.save_session(session).await
    }

    pub fn cancel_microsoft(&self, device_code: &str) {
        if let Some(token) = self.pending_logins.lock().unwrap().remove(device_code) {
            token.cancel();
        }
    }

    async fn save_session(&self, session: MinecraftSession) -> Result<Account> {
        let id = microsoft::hyphenate_uuid(&session.profile.id);
        let key = secret_key(&id);
        let refresh = session.refresh_token.clone();
        tokio::task::spawn_blocking(move || secure_store::store_secret(&key, &refresh)).await??;

        let account = Account {
            id: id.clone(),
            username: session.profile.name.clone(),
            kind: AccountKind::Microsoft,
            skin_url: session.profile.skins.iter().find(|s| s.state == "ACTIVE").map(|s| s.url.clone()),
            added_at: microsoft::now_secs(),
        };
        self.sessions.lock().unwrap().insert(
            id,
            CachedSession { access_token: session.access_token, expires_at: session.expires_at, xuid: session.xuid },
        );
        self.upsert(account.clone());
        self.persist().await?;
        Ok(account)
    }

    pub async fn remove(&self, id: &str) -> Result<AccountsView> {
        let removed = {
            let mut data = self.data.lock().unwrap();
            let before = data.accounts.len();
            let kind = data.accounts.iter().find(|a| a.id == id).map(|a| a.kind);
            data.accounts.retain(|a| a.id != id);
            if data.active.as_deref() == Some(id) {
                data.active = data.accounts.first().map(|a| a.id.clone());
            }
            (before != data.accounts.len()).then_some(kind).flatten()
        };
        if removed == Some(AccountKind::Microsoft) {
            let key = secret_key(id);
            tokio::task::spawn_blocking(move || secure_store::delete_secret(&key)).await??;
        }
        self.sessions.lock().unwrap().remove(id);
        self.persist().await?;
        Ok(self.list())
    }

    /// Credenciais prontas para a JVM. Para Microsoft, renova o token se faltar < 5 min.
    pub async fn launch_auth(&self, client: &Client, client_id: Option<&str>, id: &str) -> Result<LaunchAuth> {
        let account = self.get(id).ok_or_else(|| LauncherError::Auth("conta não encontrada".into()))?;

        if account.kind == AccountKind::Offline {
            return Ok(LaunchAuth {
                username: account.username,
                uuid: account.id,
                access_token: offline::offline_access_token(),
                user_type: "legacy".into(),
                xuid: None,
                is_offline: true,
            });
        }

        let cached = self.sessions.lock().unwrap().get(id).cloned();
        let session = match cached {
            Some(s) if s.expires_at > microsoft::now_secs() + 300 => s,
            _ => {
                let client_id = client_id
                    .ok_or_else(|| LauncherError::Auth("configure o client_id do Azure em Configurações".into()))?;
                let key = secret_key(id);
                let refresh =
                    tokio::task::spawn_blocking(move || secure_store::load_secret(&key)).await??.ok_or_else(|| {
                        LauncherError::Auth("sessão não encontrada no cofre; faça login novamente".into())
                    })?;
                let ms = microsoft::refresh_ms_token(client, client_id, &refresh).await?;
                let session = microsoft::complete_login(client, ms, Some(&refresh)).await?;
                self.save_session(session).await?;
                self.sessions
                    .lock()
                    .unwrap()
                    .get(id)
                    .cloned()
                    .ok_or_else(|| LauncherError::Auth("o UUID da conta mudou; faça login novamente".into()))?
            }
        };

        let account = self.get(id).unwrap_or(account);
        Ok(LaunchAuth {
            username: account.username,
            uuid: account.id,
            access_token: session.access_token,
            user_type: "msa".into(),
            xuid: session.xuid,
            is_offline: false,
        })
    }
}
