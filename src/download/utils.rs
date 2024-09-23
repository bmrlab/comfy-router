use super::manage::manage_cache;
use super::state::DownloadState;
use super::task::{DownloadStatus, DownloadTask};
use std::{path::Path, sync::Arc};
use tokio::sync::{watch, RwLock};
use url::Url;

pub enum CreateDownloadTaskResult {
    Existed,
    Created(watch::Receiver<DownloadStatus>),
}

pub async fn create_download_task(
    url: &Url,
    target_dir: impl AsRef<Path>,
    download_state: Arc<RwLock<DownloadState>>,
) -> (String, CreateDownloadTaskResult) {
    // when create download task, state should be locked until result is returned
    let state_clone = download_state.clone();
    let mut state = state_clone.write().await;
    let existed_task = state.get_by_url(&url).map(|v| v.clone());

    if let Some(task) = existed_task {
        match task.status() {
            DownloadStatus::Completed => {
                // make sure task exists (check if the file exists)
                let cache_path = state.cache_dir().join(task.file_id());
                if cache_path.exists() && tokio::fs::metadata(&cache_path).await.is_ok() {
                    // create symlink
                    let _ = state.add_target_dir(task.file_id(), &target_dir).await;

                    let dst = state.root_dir().join(target_dir);
                    let _ = tokio::fs::create_dir_all(&dst).await;
                    let dst = dst.join(task.file_id());

                    // if dst exists and is symlink, need to verify if target file exists
                    if dst.exists() && dst.is_symlink() {
                        match tokio::fs::read_link(&dst).await {
                            Ok(target) => {
                                if tokio::fs::metadata(target).await.is_err() {
                                    let _ = tokio::fs::remove_file(&dst).await;
                                }
                            }
                            _ => {
                                let _ = tokio::fs::remove_file(&dst).await;
                            }
                        }
                    }

                    if !dst.exists() {
                        if let Err(e) = tokio::fs::symlink(&cache_path, &dst).await {
                            tracing::warn!("failed to create symlink: {}", e);
                        }
                    }

                    return (
                        task.file_id().to_string(),
                        CreateDownloadTaskResult::Existed,
                    );
                } else {
                    // else the state need to be cleaned and symlinks should be removed
                    if let Some(symlinks) = state.target_dirs(task.file_id()) {
                        for symlink in symlinks {
                            let dst = state.root_dir().join(symlink).join(task.file_id());
                            let _ = tokio::fs::remove_file(dst).await;
                        }
                    }

                    if let Err(e) = state.remove(task.file_id()).await {
                        tracing::warn!("failed to remove download task: {}", e);
                    }
                }
            }
            DownloadStatus::Pending => {
                let _ = state.add_target_dir(task.file_id(), &target_dir).await;
                let rx = state
                    .get_notification(task.file_id())
                    .expect("notification must exists");

                return (
                    task.file_id().to_string(),
                    CreateDownloadTaskResult::Created(rx),
                );
            }
            DownloadStatus::Failed => {
                // remove failed task
                let _ = state.remove(task.file_id()).await;
            }
        }
    }

    let task = DownloadTask::new(&url);

    let file_id = task.file_id().to_string();

    if let Err(e) = state.add(task.clone()).await {
        tracing::warn!("failed to add download task: {}", e);
    }

    if let Err(e) = state.add_target_dir(file_id.as_str(), target_dir).await {
        tracing::warn!("failed to add target dir: {}", e);
    }

    let cache_dir = state.cache_dir().clone();
    let (tx, rx) = watch::channel(task.status().clone());

    state.set_notification(file_id.as_str(), rx.clone());

    tokio::spawn(async move {
        let result = task.run(&cache_dir).await;
        let download_success = result.is_ok();

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

        if download_success {
            // create symlinks
            let state = download_state.read().await;
            let symlinks = state.target_dirs(task.file_id());

            if let Some(symlinks) = symlinks {
                for symlink in symlinks {
                    let dst = state.root_dir().join(symlink);
                    let _ = tokio::fs::create_dir_all(&dst).await;

                    let dst = dst.join(task.file_id());
                    if let Err(e) = tokio::fs::symlink(&cache_dir.join(task.file_id()), &dst).await
                    {
                        tracing::warn!("failed to create symlink: {}", e);
                    }
                }
            }

            let _ = tx.send(DownloadStatus::Completed);
        } else {
            let _ = tx.send(DownloadStatus::Failed);
        }

        // remove notification
        {
            let mut state = download_state.write().await;
            state.remove_notification(task.file_id());
        }

        // manage cache
        // delete oldest file if cache folder size exceed max_cache_bytes
        if let Err(e) = manage_cache(download_state.clone()).await {
            tracing::warn!("failed to manage cache: {}", e);
        }
    });

    (file_id, CreateDownloadTaskResult::Created(rx))
}
