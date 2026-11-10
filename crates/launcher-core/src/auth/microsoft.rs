//! Fluxo Microsoft completo:
//!
//! ```text
//! Device Code (login.microsoftonline.com/consumers)
//!   → access_token MSA (escopo XboxLive.signin offline_access)
//!   → Xbox Live user.authenticate  (RPS ticket "d=<token>")  → XBL token + uhs
//!   → XSTS authorize (rp://api.minecraftservices.com/)        → XSTS token
//!   → api.minecraftservices.com/authentication/login_with_xbox → Bearer da Mojang
//!   → entitlements/mcstore + minecraft/profile                → nick, UUID, skins
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::error::{LauncherError, Result};
use crate::http::{check_status, with_retry};

const DEVICE_CODE_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_ENTITLEMENTS_URL: &str = "https://api.minecraftservices.com/entitlements/mcstore";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    #[serde(alias = "user_code")]
    pub user_code: String,
    #[serde(alias = "device_code")]
    pub device_code: String,
    #[serde(alias = "verification_uri")]
    pub verification_uri: String,
    #[serde(alias = "expires_in")]
    pub expires_in: u64,
    pub interval: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MsToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    error_description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<ProfileSkin>,
    #[serde(default)]
    pub capes: Vec<crate::cosmetics::MojangCape>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSkin {
    pub url: String,
    pub state: String,
    #[serde(default)]
    pub variant: Option<String>,
}

/// Resultado final do login: tudo que o launcher precisa para iniciar o jogo.
#[derive(Debug, Clone)]
pub struct MinecraftSession {
    pub profile: MinecraftProfile,
    pub access_token: String,
    /// Epoch em segundos.
    pub expires_at: u64,
    pub refresh_token: String,
    pub xuid: Option<String>,
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Etapa 1: Device Code Flow
// ---------------------------------------------------------------------------

pub async fn request_device_code(client: &Client, client_id: &str) -> Result<DeviceCode> {
    with_retry(3, || async move {
        let resp = client.post(DEVICE_CODE_URL).form(&[("client_id", client_id), ("scope", SCOPE)]).send().await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            if let Ok(err) = serde_json::from_str::<OAuthError>(&body) {
                return Err(LauncherError::Auth(format!(
                    "Azure recusou o client_id ({}): {}",
                    err.error,
                    err.error_description.unwrap_or_default()
                )));
            }
            return Err(LauncherError::HttpStatus { url: DEVICE_CODE_URL.into(), status });
        }
        Ok(resp.json::<DeviceCode>().await?)
    })
    .await
}

/// Faz polling no endpoint de token até o usuário concluir o login no navegador.
pub async fn poll_device_code(
    client: &Client,
    client_id: &str,
    code: &DeviceCode,
    cancel: &CancellationToken,
) -> Result<MsToken> {
    let deadline = now_secs() + code.expires_in;
    let mut interval = code.interval.max(1);

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Err(LauncherError::Cancelled),
            _ = tokio::time::sleep(Duration::from_secs(interval)) => {}
        }
        if now_secs() >= deadline {
            return Err(LauncherError::Auth("o código expirou; inicie o login novamente".into()));
        }

        let resp = match client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", DEVICE_CODE_GRANT),
                ("client_id", client_id),
                ("device_code", code.device_code.as_str()),
            ])
            .send()
            .await
        {
            Ok(r) => r,
            // Queda de rede durante o polling: tenta de novo no próximo ciclo
            Err(e) => {
                tracing::warn!("falha de rede no polling do device code: {e}");
                continue;
            }
        };

        if resp.status().is_success() {
            return Ok(resp.json::<MsToken>().await?);
        }

        let body = resp.text().await.unwrap_or_default();
        let err: OAuthError = serde_json::from_str(&body)
            .map_err(|_| LauncherError::Auth(format!("resposta inesperada do Azure: {body}")))?;
        match err.error.as_str() {
            "authorization_pending" => continue,
            "slow_down" => interval += 5,
            "authorization_declined" | "access_denied" => {
                return Err(LauncherError::Auth("o login foi recusado no navegador".into()))
            }
            "expired_token" | "bad_verification_code" => {
                return Err(LauncherError::Auth("o código expirou; inicie o login novamente".into()))
            }
            other => {
                return Err(LauncherError::Auth(format!("{other}: {}", err.error_description.unwrap_or_default())))
            }
        }
    }
}

pub async fn refresh_ms_token(client: &Client, client_id: &str, refresh_token: &str) -> Result<MsToken> {
    with_retry(3, || async move {
        let resp = client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ])
            .send()
            .await?;
        if resp.status().as_u16() == 400 || resp.status().as_u16() == 401 {
            let body = resp.text().await.unwrap_or_default();
            return Err(LauncherError::Auth(format!("sessão Microsoft expirada, faça login novamente ({body})")));
        }
        Ok(check_status(resp)?.json::<MsToken>().await?)
    })
    .await
}

// ---------------------------------------------------------------------------
// Etapa 2: Xbox Live + XSTS
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    token: String,
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<XuiClaim>,
}

#[derive(Debug, Deserialize)]
struct XuiClaim {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XstsError {
    x_err: u64,
}

pub struct XboxToken {
    pub token: String,
    pub uhs: String,
    pub xuid: Option<String>,
}

async fn xbox_live_authenticate(client: &Client, ms_access_token: &str) -> Result<XboxToken> {
    let body = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT",
    });
    let body = &body;
    let resp: XboxResponse = with_retry(3, || async move {
        let r = client.post(XBL_URL).header("Accept", "application/json").json(body).send().await?;
        Ok(check_status(r)?.json::<XboxResponse>().await?)
    })
    .await?;
    let claim = resp
        .display_claims
        .xui
        .into_iter()
        .next()
        .ok_or_else(|| LauncherError::Auth("Xbox Live não retornou o user hash (uhs)".into()))?;
    Ok(XboxToken { token: resp.token, uhs: claim.uhs, xuid: claim.xid })
}

async fn xsts_authorize(client: &Client, xbl_token: &str) -> Result<XboxToken> {
    let body = json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT",
    });
    let body = &body;
    with_retry(3, || async move {
        let r = client.post(XSTS_URL).header("Accept", "application/json").json(body).send().await?;
        if r.status().as_u16() == 401 {
            let err: XstsError = r.json().await.map_err(|_| LauncherError::Auth("XSTS recusou o token".into()))?;
            return Err(LauncherError::Auth(xsts_error_message(err.x_err)));
        }
        let resp: XboxResponse = check_status(r)?.json().await?;
        let claim = resp
            .display_claims
            .xui
            .into_iter()
            .next()
            .ok_or_else(|| LauncherError::Auth("XSTS não retornou o user hash (uhs)".into()))?;
        Ok(XboxToken { token: resp.token, uhs: claim.uhs, xuid: claim.xid })
    })
    .await
}

fn xsts_error_message(code: u64) -> String {
    match code {
        2148916233 => "Esta conta Microsoft não tem perfil Xbox. Entre em xbox.com uma vez para criar.".into(),
        2148916235 => "Xbox Live não está disponível no país desta conta.".into(),
        2148916236 | 2148916237 => "A conta precisa de verificação de idade adulta (Coreia do Sul).".into(),
        2148916238 => "Conta de menor de idade: um adulto precisa adicioná-la a uma Família Microsoft.".into(),
        other => format!("XSTS recusou a conta (XErr {other})"),
    }
}

// ---------------------------------------------------------------------------
// Etapa 3: Minecraft Services
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct McLoginResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct Entitlements {
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

async fn minecraft_login(client: &Client, xsts: &XboxToken) -> Result<McLoginResponse> {
    let body = json!({ "identityToken": format!("XBL3.0 x={};{}", xsts.uhs, xsts.token) });
    let body = &body;
    with_retry(3, || async move {
        let r = client.post(MC_LOGIN_URL).json(body).send().await?;
        if r.status().as_u16() == 403 {
            return Err(LauncherError::Auth(
                "A Mojang recusou o app Azure (403). Novos client IDs precisam ser aprovados pela Mojang \
                 para acessar a API do Minecraft — veja o README."
                    .into(),
            ));
        }
        Ok(check_status(r)?.json::<McLoginResponse>().await?)
    })
    .await
}

async fn has_entitlement(client: &Client, mc_token: &str) -> Result<bool> {
    let e: Entitlements = with_retry(3, || async move {
        let r = client.get(MC_ENTITLEMENTS_URL).bearer_auth(mc_token).send().await?;
        Ok(check_status(r)?.json::<Entitlements>().await?)
    })
    .await?;
    Ok(!e.items.is_empty())
}

pub async fn fetch_profile(client: &Client, mc_token: &str) -> Result<MinecraftProfile> {
    with_retry(3, || async move {
        let r = client.get(MC_PROFILE_URL).bearer_auth(mc_token).send().await?;
        if r.status().as_u16() == 404 {
            return Err(LauncherError::Auth(
                "A conta tem o jogo, mas ainda não criou um perfil (nick). Abra minecraft.net para criar.".into(),
            ));
        }
        Ok(check_status(r)?.json::<MinecraftProfile>().await?)
    })
    .await
}

/// Executa XBL → XSTS → Minecraft a partir de um token MSA válido.
pub async fn complete_login(client: &Client, ms: MsToken, previous_refresh: Option<&str>) -> Result<MinecraftSession> {
    let xbl = xbox_live_authenticate(client, &ms.access_token).await?;
    let xsts = xsts_authorize(client, &xbl.token).await?;
    let mc = minecraft_login(client, &xsts).await?;

    // Game Pass e contas migradas às vezes retornam entitlements vazio mas têm perfil;
    // por isso o perfil é a verificação definitiva.
    let owns = has_entitlement(client, &mc.access_token).await.unwrap_or(true);
    let profile = match fetch_profile(client, &mc.access_token).await {
        Ok(p) => p,
        Err(e) if !owns => {
            tracing::debug!("perfil indisponível: {e}");
            return Err(LauncherError::NoEntitlement);
        }
        Err(e) => return Err(e),
    };

    let refresh_token = ms
        .refresh_token
        .or_else(|| previous_refresh.map(str::to_owned))
        .ok_or_else(|| LauncherError::Auth("a Microsoft não retornou refresh_token".into()))?;

    Ok(MinecraftSession {
        profile,
        access_token: mc.access_token,
        expires_at: now_secs() + mc.expires_in,
        refresh_token,
        xuid: xsts.xuid.or(xbl.xuid),
    })
}

/// UUID da Mojang vem sem hífens; o launcher trabalha com o formato canônico.
pub fn hyphenate_uuid(raw: &str) -> String {
    uuid::Uuid::parse_str(raw).map(|u| u.hyphenated().to_string()).unwrap_or_else(|_| raw.to_owned())
}
