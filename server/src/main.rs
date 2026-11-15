use openparts_server::{app, bundle, store};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let data_dir = std::env::var("OPENPARTS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../openparts-data"));
    tracing::info!(data_dir = %data_dir.display(), "loading openparts-data");
    let mut loaded_store = store::discover_and_load(&data_dir)?;
    // A bundle from `openparts bundle`: artifacts are served from it, so
    // this server needs no SolidCraft. Without one (or for anything it
    // lacks) 3D models are built live, which needs SolidCraft.
    if let Some(dir) = std::env::var_os("OPENPARTS_ARTIFACT_DIR") {
        let dir = PathBuf::from(dir);
        loaded_store.bundle = Some(bundle::Bundle::load(&dir)?);
    }
    loaded_store.review = openparts_server::review::ReviewService::from_env()?;
    loaded_store.source_cache = openparts_server::sources::SourceCache::from_env()?;
    let solidcraft = tokio::task::spawn_blocking(openparts_solidcraft::available).await?;
    tracing::info!(
        bundle = loaded_store.bundle.is_some(),
        solidcraft,
        "3D models: served from the bundle when present, built live only with SolidCraft"
    );
    let loaded_store = Arc::new(loaded_store);

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    // HOST=127.0.0.1 keeps the server private behind a tunnel/proxy.
    let host: std::net::IpAddr = std::env::var("HOST")
        .ok()
        .and_then(|h| h.parse().ok())
        .unwrap_or(std::net::IpAddr::from([0, 0, 0, 0]));
    let addr = SocketAddr::from((host, port));

    // OPENPARTS_WEB_DIR: an openparts-web build to serve alongside the API
    // (same origin; the site calls /api/...).
    let web_dir = std::env::var_os("OPENPARTS_WEB_DIR").map(PathBuf::from);
    if let Some(dir) = &web_dir {
        tracing::info!(web_dir = %dir.display(), "serving the web site");
    }
    let app = app::build_app(loaded_store, web_dir.as_deref());
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "openparts-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
