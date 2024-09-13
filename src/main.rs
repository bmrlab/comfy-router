use comfy_router::config::AppConfig;
use comfy_router::run;
use comfy_router::state::AppState;
use tracing::{debug, error};

#[cfg(debug_assertions)]
use dotenv::dotenv;

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    dotenv().ok();

    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env();
    debug!("config: {:?}", config);
    let state = AppState::new(config).await;

    if let Err(e) = run(state).await {
        error!("failed to start app: {}", e);
    }
}
