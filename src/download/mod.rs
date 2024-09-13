use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use manage::manage_cache;
use state::DownloadState;
use task::{run_download_task, DownloadStatus, DownloadTask};
use tokio::sync::{Notify, RwLock};
use url::Url;

use crate::state::AppState;

pub mod manage;
pub mod state;
pub mod task;

pub enum CreateDownloadTaskResult {
    Existed(DownloadStatus),
    Created(DownloadStatus, Arc<Notify>),
}

pub async fn create_download_task(
    url: &Url,
    target_folder: impl AsRef<Path>,
    download_state: Arc<RwLock<DownloadState>>,
    app_state: Arc<AppState>,
) -> (String, CreateDownloadTaskResult) {
    let existed_task = {
        let state = download_state.write().await;
        state.get_by_url(&url).await.map(|v| v.clone())
    };

    match existed_task {
        Some(task) => (
            task.file_id().to_string(),
            CreateDownloadTaskResult::Existed(task.status().clone()),
        ),
        _ => {
            let target_folder = PathBuf::from(&app_state.config().root_dir).join(&target_folder);
            let task = DownloadTask::new(&url, &target_folder.to_string_lossy().to_string());

            let file_id = task.file_id().to_string();
            let task_status = task.status().clone();

            {
                let mut state = download_state.write().await;
                if let Err(e) = state.add(task.clone()).await {
                    tracing::warn!("failed to add download task: {}", e);
                }
            }

            let cache_dir = app_state.config().cache_dir.clone();
            let max_cache_bytes = app_state.config().max_cache_bytes;

            let notify = Notify::new();
            let notify = Arc::new(notify);
            let notify_clone = notify.clone();

            tokio::spawn(async move {
                tracing::info!("start download task: {}", task.url());
                let result = run_download_task(&task, &cache_dir).await;

                {
                    let mut state = download_state.write().await;
                    if let Err(e) = state
                        .update_status(
                            task.file_id(),
                            match result {
                                Err(e) => {
                                    tracing::warn!(
                                        "failed to download {}({}): {}",
                                        task.file_id(),
                                        task.url(),
                                        e
                                    );
                                    DownloadStatus::Failed
                                }
                                _ => {
                                    tracing::info!(
                                        "download completed: {}({})",
                                        task.file_id(),
                                        task.url()
                                    );
                                    DownloadStatus::Completed
                                }
                            },
                        )
                        .await
                    {
                        tracing::warn!("failed to update downloaded status: {}", e);
                    }
                }

                // manage cache
                // delete oldest file if cache folder size exceed max_cache_bytes
                if let Err(e) =
                    manage_cache(&cache_dir, download_state.clone(), max_cache_bytes).await
                {
                    tracing::warn!("failed to manage cache: {}", e);
                }

                notify.notify_waiters();
            });

            (
                file_id,
                CreateDownloadTaskResult::Created(task_status, notify_clone),
            )
        }
    }
}
