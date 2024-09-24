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
use utoipa::ToSchema;

const OPENAPI_TAG: &str = "Workflow";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WorkflowResponse {
    id: String,
}

/// Run workflow
/// 
/// Run SD15, SDXL or Flux workflow using predefined params.
#[utoipa::path(
    post, 
    path = "/workflow",
    request_body(content=WorkflowPayload, content_type="application/json"),
    responses((
        status = OK, 
        body = WorkflowResponse
    )),
    security(("basic_auth" = [])),
    tag = OPENAPI_TAG
)]
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

/// Check workflow
/// 
/// Get the full results of a workflow with given id.
#[utoipa::path(
    get, 
    path = "/workflow/{id}", 
    responses((
        status = OK, 
        body = WorkflowResult
    ), (
        status = NOT_FOUND,
        description = "Workflow not found.",
        body = String
    )),
    security(("basic_auth" = [])),
    tag = OPENAPI_TAG
)]
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

/// Get preview
/// 
/// Get the preview result of a workflow with given id.
/// If the workflow has finished, the preview will no longer be available.
#[utoipa::path(
    get, 
    path = "/preview/{id}", 
    responses((
        status = OK, 
        body = WorkflowResult
    ), (
        status = NOT_FOUND,
        description = "Workflow not found.",
        body = String
    )),
    tag = OPENAPI_TAG
)]
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
