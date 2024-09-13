use super::{AppError, AppJson};
use crate::{
    cluster::NodeState,
    download::state::DownloadState,
    state::AppState,
    workflow::{
        payload::WorkflowPayload,
        task::{WorkflowRecord, WorkflowResult, WorkflowTask},
    },
};
use axum::{
    extract::{Path, Request, State},
    response::IntoResponse,
    routing::{get, post, Route},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::RwLock;
use tower::{Layer, Service, ServiceBuilder};
use tower_http::add_extension::AddExtensionLayer;

pub struct SharedState {
    workflow_record: Arc<RwLock<WorkflowRecord>>,
    node_state: Arc<RwLock<NodeState>>,
    download_state: Arc<RwLock<DownloadState>>,
}

impl SharedState {
    pub fn new(
        node_state: Arc<RwLock<NodeState>>,
        download_state: Arc<RwLock<DownloadState>>,
        workflow_record: Arc<RwLock<WorkflowRecord>>,
    ) -> Self {
        Self {
            workflow_record,
            node_state,
            download_state,
        }
    }

    pub fn workflow_record(&self) -> Arc<RwLock<WorkflowRecord>> {
        self.workflow_record.clone()
    }

    pub fn node_state(&self) -> Arc<RwLock<NodeState>> {
        self.node_state.clone()
    }

    pub fn download_state(&self) -> Arc<RwLock<DownloadState>> {
        self.download_state.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    id: String,
}

pub async fn run_workflow(
    State(app_state): State<Arc<AppState>>,
    Extension(state): Extension<Arc<SharedState>>,
    AppJson(data): AppJson<WorkflowPayload>,
) -> Result<AppJson<WorkflowResponse>, AppError> {
    let picked = {
        let cache_map = data.cache_map();
        let node_state = state.node_state();
        let mut node_state = node_state.write().await;
        node_state.pick(&cache_map)
    };

    match picked {
        Some(url) => {
            let task = WorkflowTask::new(&url, data);
            let task_id = task.id().to_string();

            let workflow_record = state.workflow_record();
            let mut workflow_record = workflow_record.write().await;
            workflow_record.add(task.clone());

            let node_state = state.node_state();
            let download_state = state.download_state();
            tokio::spawn(async move {
                task.run(download_state, app_state).await;
                // after task done, set node to idle
                let mut node_state = node_state.write().await;
                node_state.set_idle(task.node());
            });

            Ok(AppJson(WorkflowResponse { id: task_id }))
        }
        _ => Err(AppError::NoNodeAvailable),
    }
}

pub async fn check_workflow(
    Extension(state): Extension<Arc<SharedState>>,
    Path(id): Path<String>,
) -> Result<AppJson<WorkflowResult>, AppError> {
    let workflow_record = state.workflow_record();
    let workflow_record = workflow_record.read().await;
    let task = workflow_record.get(&id);
    if let Some(task) = task {
        Ok(AppJson(task.result().await))
    } else {
        Err(AppError::NotFoundError(anyhow::anyhow!("task not found")))
    }
}

pub async fn preview_workflow(
    Extension(state): Extension<Arc<SharedState>>,
    Path(id): Path<String>,
) -> Result<AppJson<WorkflowResult>, AppError> {
    let workflow_record = state.workflow_record();
    let workflow_record = workflow_record.read().await;
    let task = workflow_record.get(&id);
    if let Some(task) = task {
        let result = task.result().await;

        let result = match result {
            // ignore result
            WorkflowResult::Done(_) => WorkflowResult::Done(vec![]),
            _ => result,
        };

        Ok(AppJson(result))
    } else {
        Err(AppError::NotFoundError(anyhow::anyhow!("task not found")))
    }
}

pub fn workflow_routes<L>(
    node_state: Arc<RwLock<NodeState>>,
    download_state: Arc<RwLock<DownloadState>>,
    auth_layer: L,
) -> Router<Arc<AppState>>
where
    L: Layer<Route> + Clone + Send + 'static,
    L::Service: Service<Request> + Clone + Send + 'static,
    <L::Service as Service<Request>>::Response: IntoResponse + 'static,
    <L::Service as Service<Request>>::Error: Into<Infallible> + 'static,
    <L::Service as Service<Request>>::Future: Send + 'static,
{
    // TODO make record resizable according to node list size
    // for now, 50 is suitable for most of the cases
    let workflow_record = WorkflowRecord::new(50);
    let workflow_record = Arc::new(RwLock::new(workflow_record));
    let shared_state = Arc::new(SharedState::new(
        node_state,
        download_state,
        workflow_record,
    ));

    Router::new()
        .route("/", post(run_workflow))
        .route("/:id", get(check_workflow))
        .layer(auth_layer)
        .route("/:id/preview", get(preview_workflow))
        .layer(
            ServiceBuilder::new()
                .layer(AddExtensionLayer::new(shared_state))
                .into_inner(),
        )
}
