//! Autenticação: contas Microsoft (Device Code Flow → Xbox Live → XSTS → Minecraft Services)
//! e contas offline com UUID v3 determinístico.

pub mod accounts;
pub mod microsoft;
pub mod offline;
pub mod secure_store;

use serde::{Deserialize, Serialize};

/// Dados de autenticação injetados nos argumentos da JVM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAuth {
    pub username: String,
    /// UUID com hífens.
    pub uuid: String,
    pub access_token: String,
    /// `msa` para Microsoft, `legacy` para offline.
    pub user_type: String,
    pub xuid: Option<String>,
    pub is_offline: bool,
}

impl LaunchAuth {
    pub fn uuid_simple(&self) -> String {
        self.uuid.replace('-', "")
    }
}
