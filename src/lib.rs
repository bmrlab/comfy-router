mod cluster;
pub mod config;
mod download;
mod routes;
pub mod state;
mod workflow;

use axum::{extract::Request, routing::get, Router, ServiceExt};
use routes::{
    cluster::cluster_routes,
    download::download_routes,
    workflow::{preview_workflow, workflow_routes},
};

use state::AppState;
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tower::{Layer, ServiceBuilder};
use tower_http::{
    normalize_path::NormalizePathLayer, trace::TraceLayer,
    validate_request::ValidateRequestHeaderLayer,
};

#[cfg(debug_assertions)]
use tower_http::cors::{Any, CorsLayer};

#[cfg(not(debug_assertions))]
use axum_embed::ServeEmbed;
#[cfg(not(debug_assertions))]
use rust_embed::RustEmbed;

#[derive(RustEmbed, Clone)]
#[folder = "web/dist/"]
#[cfg(not(debug_assertions))]
struct AdminWebDist;

pub async fn run(app_state: AppState) -> anyhow::Result<()> {
    #[cfg(not(debug_assertions))]
    let serve_admin_web = ServeEmbed::<AdminWebDist>::new();

    let config = app_state.config().clone();

    let auth_layer = ValidateRequestHeaderLayer::basic(&config.username, &config.password);

    let app = Router::new()
        .nest("/download", download_routes())
        .nest("/cluster", cluster_routes(app_state.node_state()));

    #[cfg(not(debug_assertions))]
    let app = app.nest_service("/admin", serve_admin_web);

    let app = app
        .nest("/workflow", workflow_routes())
        .layer(auth_layer)
        .route("/preview/:id", get(preview_workflow))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(Arc::new(app_state))
        .route("/health_check", get(routes::health_check));

    #[cfg(debug_assertions)]
    let app = app.layer(
        CorsLayer::new()
            .allow_headers(Any)
            .allow_origin(Any)
            .allow_methods(Any),
    );

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
