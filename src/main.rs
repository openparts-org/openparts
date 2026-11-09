use openparts_server::{bundle, routes, store};
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
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let app = routes::build_router(loaded_store);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "openparts-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
