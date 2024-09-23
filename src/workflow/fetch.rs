use crate::{download::task::DownloadStatus, state::AppState};
use std::sync::Arc;
use tokio::{sync::watch, task::JoinSet};

pub trait Fetch {
    fn fetch(
        &self,
        app_state: Arc<AppState>,
        target_folder: &str,
    ) -> impl std::future::Future<Output = (String, Option<watch::Receiver<DownloadStatus>>)> + Send;
}

pub struct FetchHelper {
    join_set: JoinSet<DownloadStatus>,
    app_state: Arc<AppState>,
}

impl FetchHelper {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            join_set: JoinSet::new(),
            app_state,
        }
    }

    /// Get the filename of the artifact.
    /// If the file doesn't exist, a download task will be triggered
    /// in the background, which will not block this function.
    /// Use `wait_all` to wait for all triggered download task.
    pub async fn add(&mut self, artifact: impl Fetch, target_folder: &str) -> String {
        let (name, rx) = artifact.fetch(self.app_state.clone(), target_folder).await;

        if let Some(mut rx) = rx {
            self.join_set.spawn(async move {
                loop {
                    if rx.changed().await.is_err() {
                        break;
                    }

                    let status = rx.borrow();
                    match &*status {
                        DownloadStatus::Completed => break,
                        DownloadStatus::Failed => break,
                        _ => {}
                    }
                }

                rx.borrow().clone()
            });
        }

        name
    }

    /// Wait for all download task added by `add` to finish.
    /// If any task failed, this function will return an error.
    pub async fn wait_all(self) -> anyhow::Result<()> {
        let results = self.join_set.join_all().await;

        if results.iter().any(|v| *v != DownloadStatus::Completed) {
            anyhow::bail!("download failed");
        } else {
            Ok(())
        }
    }
}
