use std::future::Future;
use std::time::Duration;

use reqwest::{Client, Response};
use serde::de::DeserializeOwned;

use crate::error::{LauncherError, Result};

pub const USER_AGENT: &str = concat!("AuroraLauncher/", env!("CARGO_PKG_VERSION"));

pub fn build_client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        // read_timeout (e não timeout total) para não abortar downloads grandes em conexões lentas
        .read_timeout(Duration::from_secs(45))
        .pool_max_idle_per_host(32)
        .build()?)
}

/// Converte respostas não-2xx em `LauncherError::HttpStatus`.
pub fn check_status(resp: Response) -> Result<Response> {
    let status = resp.status();
    if status.is_success() {
        Ok(resp)
    } else {
        Err(LauncherError::HttpStatus { url: resp.url().to_string(), status: status.as_u16() })
    }
}

/// Executa `f` até `attempts` vezes com backoff exponencial (400ms, 800ms, 1.6s…)
/// somente para erros transitórios.
pub async fn with_retry<T, F, Fut>(attempts: u32, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let attempts = attempts.max(1);
    let mut attempt = 0;
    loop {
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if e.is_retryable() && attempt + 1 < attempts => {
                let delay = Duration::from_millis(400 * 2u64.pow(attempt));
                tracing::warn!("tentativa {} falhou ({e}); nova tentativa em {:?}", attempt + 1, delay);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

pub async fn get_json<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T> {
    with_retry(3, || async move {
        let resp = check_status(client.get(url).send().await?)?;
        Ok(resp.json::<T>().await?)
    })
    .await
}
