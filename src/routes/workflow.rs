use super::{AppError, AppJson};
use crate::{
    state::AppState,
    workflow::{payload::WorkflowPayload, record::run_task, task::WorkflowResult},
};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResponse {
    id: String,
}

pub async fn run_workflow(
    State(app_state): State<Arc<AppState>>,
    AppJson(data): AppJson<WorkflowPayload>,
) -> Result<AppJson<WorkflowResponse>, AppError> {
    let workflow_record = app_state.workflow_record();
    let mut workflow_record = workflow_record.write().await;
    let workflow_task = workflow_record.add(data)?;
    let task_id = workflow_task.id().to_string();

    tokio::spawn(async move {
        run_task(app_state).await;
    });

    Ok(AppJson(WorkflowResponse { id: task_id }))
}

pub async fn check_workflow(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<AppJson<WorkflowResult>, AppError> {
    let workflow_record = app_state.workflow_record();
    let workflow_record = workflow_record.read().await;
    let task = workflow_record.get(&id);
    if let Some(task) = task {
        Ok(AppJson(task.result().await))
    } else {
        Err(AppError::NotFoundError(anyhow::anyhow!("task not found")))
    }
}

pub async fn preview_workflow(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<AppJson<WorkflowResult>, AppError> {
    let workflow_record = app_state.workflow_record();
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

pub fn workflow_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(run_workflow))
        .route("/:id", get(check_workflow))
}
