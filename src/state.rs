use crate::{
    cluster::NodeState, config::AppConfig, download::state::DownloadState,
    workflow::record::WorkflowRecord,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct AppState {
    config: AppConfig,
    download_state: Arc<RwLock<DownloadState>>,
    node_state: Arc<RwLock<NodeState>>,
    workflow_record: Arc<RwLock<WorkflowRecord>>,
}

impl AppState {
    pub async fn new(config: AppConfig) -> Self {
        let download_state = DownloadState::new(&config.record_path).await;
        let node_state = NodeState::new();

        // TODO make record resizable according to node list size
        // for now, 50 is suitable for most of the cases
        let workflow_record =
            WorkflowRecord::new(config.workflow_history_limit, config.workflow_pending_limit);

        Self {
            config,
            download_state: Arc::new(RwLock::new(download_state)),
            node_state: Arc::new(RwLock::new(node_state)),
            workflow_record: Arc::new(RwLock::new(workflow_record)),
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn download_state(&self) -> Arc<RwLock<DownloadState>> {
        self.download_state.clone()
    }

    pub fn node_state(&self) -> Arc<RwLock<NodeState>> {
        self.node_state.clone()
    }

    pub fn workflow_record(&self) -> Arc<RwLock<WorkflowRecord>> {
        self.workflow_record.clone()
    }
}
