use crate::config::AppConfig;

#[derive(Clone, Debug)]
pub struct AppState {
    config: AppConfig,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }
}
