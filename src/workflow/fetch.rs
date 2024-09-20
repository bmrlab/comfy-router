use std::sync::Arc;

use tokio::{sync::Notify, task::JoinSet};

use crate::state::AppState;

pub trait Fetch {
    fn fetch(
        &self,
        target_folder: &str,
        app_state: Arc<AppState>,
    ) -> impl std::future::Future<Output = (String, Option<Arc<Notify>>)> + Send;
}

pub struct FetchHelper {
    join_set: JoinSet<()>,
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
        let (name, notify) = artifact.fetch(target_folder, self.app_state.clone()).await;

        if let Some(notify) = notify {
            self.join_set.spawn(async move {
                notify.notified().await;
            });
        }

        name
    }

    /// Wait for all download task added by `add` to finish.
    pub async fn wait_all(self) {
        self.join_set.join_all().await;
    }
}
