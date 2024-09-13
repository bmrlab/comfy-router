mod cluster;
pub mod config;
mod download;
mod routes;
pub mod state;
mod workflow;

use axum::{extract::Request, routing::get, Router, ServiceExt};
use axum_embed::ServeEmbed;
use cluster::NodeState;
use download::state::DownloadState;
use routes::{cluster::cluster_routes, download::download_routes, workflow::workflow_routes};
use rust_embed::RustEmbed;
use state::AppState;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use tower::{Layer, ServiceBuilder};
use tower_http::{
    normalize_path::NormalizePathLayer, trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};

#[cfg(debug_assertions)]
use tower_http::cors::{Any, CorsLayer};

#[derive(RustEmbed, Clone)]
#[folder = "web/dist/"]
struct AdminWebDist;

pub async fn run(app_state: AppState) -> anyhow::Result<()> {
    let serve_admin_web = ServeEmbed::<AdminWebDist>::new();

    let config = app_state.config().clone();

    let download_state = DownloadState::new(&config.record_path).await;
    let download_state = Arc::new(RwLock::new(download_state));
    let node_state = NodeState::new();
    let node_state = Arc::new(RwLock::new(node_state));

    // TODO here is a little stupid to pass auth layer to routes
    // but workflow_route has different auth requirement for each api
    let auth_layer = ValidateRequestHeaderLayer::basic(&config.username, &config.password);

    let app = Router::new()
        .nest("/download", download_routes(download_state.clone()))
        .nest("/cluster", cluster_routes(node_state.clone()))
        .nest_service("/admin", serve_admin_web)
        .layer(auth_layer.clone())
        .nest(
            "/workflow",
            workflow_routes(node_state.clone(), download_state, auth_layer),
        )
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(Arc::new(app_state))
        .route("/health_check", get(routes::health_check));

    #[cfg(debug_assertions)]
    let app = app.layer(CorsLayer::new().allow_headers(Any).allow_origin(Any));

    let app = NormalizePathLayer::trim_trailing_slash().layer(app);

    let addr = SocketAddr::from_str(format!("{}:{}", &config.host, &config.port).as_str())?;

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!("Listening on http://{}", listener.local_addr()?);

    axum::serve(listener, ServiceExt::<Request>::into_make_service(app))
        .with_graceful_shutdown(signal_shutdown())
        .await?;

    Ok(())
}

async fn signal_shutdown() {
    tokio::signal::ctrl_c()
        .await
        .expect("expect tokio signal ctrl-c");
    tracing::info!("signal shutdown");
}
