//! `cargo run -p cosmetics-server` → escuta em 0.0.0.0:8787 (ou `COSMETICS_ADDR`).
//! Coloque atrás de um proxy HTTPS (Caddy/Nginx/Cloudflare) e informe a URL no launcher.

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let dir = cosmetics_server::data_dir_from_env();
    let state = cosmetics_server::AppState::open(&dir).await?;
    let addr = std::env::var("COSMETICS_ADDR").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("servidor de cosméticos em http://{addr} (dados em {})", dir.display());

    axum::serve(listener, cosmetics_server::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
