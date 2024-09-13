use super::task::{DownloadStatus, DownloadTask};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use url::Url;

#[derive(Clone, Debug)]
pub struct DownloadState {
    inner: InnerState,
    record_path: PathBuf,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InnerState {
    // TODO target_folder should also be considered when caching
    downloads: HashMap<String, DownloadTask>,
    url_mapping: HashMap<Url, String>,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            downloads: HashMap::new(),
            url_mapping: HashMap::new(),
        }
    }
}

impl DownloadState {
    pub async fn new(record_path: impl AsRef<Path>) -> Self {
        let inner_state = {
            let record_path = record_path.as_ref();
            if record_path.exists() {
                let json_str = tokio::fs::read_to_string(record_path)
                    .await
                    .unwrap_or_default();
                serde_json::from_str(&json_str).unwrap_or_default()
            } else {
                InnerState::default()
            }
        };

        Self {
            inner: inner_state,
            record_path: record_path.as_ref().to_path_buf(),
        }
    }

    async fn dump(&self) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&self.inner)?;
        tokio::fs::write(&self.record_path, json_str).await?;
        Ok(())
    }

    pub async fn get_by_id(&self, id: &str) -> Option<&DownloadTask> {
        self.inner.downloads.get(id)
    }

    pub async fn get_by_url(&self, url: &Url) -> Option<&DownloadTask> {
        match self.inner.url_mapping.get(url) {
            Some(file_id) => self.inner.downloads.get(file_id),
            _ => None,
        }
    }

    pub async fn add(&mut self, task: DownloadTask) -> anyhow::Result<()> {
        let file_id = task.file_id().to_string();
        let url = task.url().clone();
        self.inner.downloads.insert(file_id.clone(), task);
        self.inner.url_mapping.insert(url, file_id);

        self.dump().await?;

        Ok(())
    }

    pub async fn update_status(
        &mut self,
        file_id: &str,
        status: DownloadStatus,
    ) -> anyhow::Result<()> {
        match self.inner.downloads.get_mut(file_id) {
            Some(task) => task.with_status(status),
            _ => (),
        }

        self.dump().await?;

        Ok(())
    }

    pub async fn remove(&mut self, file_id: &str) -> anyhow::Result<Option<DownloadTask>> {
        let removed = self.inner.downloads.remove(file_id);

        if let Some(task) = &removed {
            self.inner.url_mapping.remove(task.url());
        }

        self.dump().await?;

        Ok(removed)
    }
}
