use super::{AppError, AppJson};
use crate::{
    download::{
        create_download_task, state::DownloadState, task::DownloadStatus, CreateDownloadTaskResult,
    },
    state::AppState,
};
use axum::{
    extract::{Path, State},
    routing::{get, post},
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::add_extension::AddExtensionLayer;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateDownloadTask {
    url: Url,
    target_folder: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResponseDownloadTask {
    pub file_id: String,
    pub status: DownloadStatus,
}

type SharedAppState = Arc<RwLock<DownloadState>>;

pub async fn create(
    State(app_state): State<Arc<AppState>>,
    Extension(state): Extension<SharedAppState>,
    AppJson(data): AppJson<CreateDownloadTask>,
) -> Result<AppJson<ResponseDownloadTask>, AppError> {
    let (file_id, result) =
        create_download_task(&data.url, &data.target_folder, state, app_state).await;

    match result {
        CreateDownloadTaskResult::Existed(status) => {
            Ok(AppJson(ResponseDownloadTask { file_id, status }))
        }
        CreateDownloadTaskResult::Created(status, _) => {
            Ok(AppJson(ResponseDownloadTask { file_id, status }))
        }
    }
}

pub async fn get_download_task(
    Extension(state): Extension<SharedAppState>,
    Path(file_id): Path<String>,
) -> Result<AppJson<ResponseDownloadTask>, AppError> {
    let state = state.read().await;
    let task = state.get_by_id(&file_id).await;

    match task {
        Some(task) => Ok(AppJson(ResponseDownloadTask {
            file_id: task.file_id().to_string(),
            status: task.status().clone(),
        })),
        _ => Err(AppError::NotFoundError(anyhow::anyhow!("task not found"))),
    }
}

pub fn download_routes(shared_state: Arc<RwLock<DownloadState>>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create))
        .route("/:file_id", get(get_download_task))
        .layer(
            ServiceBuilder::new()
                .layer(AddExtensionLayer::new(shared_state))
                .into_inner(),
        )
}
