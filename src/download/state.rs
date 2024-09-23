use super::task::{DownloadStatus, DownloadTask};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Debug)]
pub struct DownloadState {
    inner: InnerState,
    record_path: PathBuf,
    cache_dir: PathBuf,
    root_dir: PathBuf,
    max_cache_bytes: u64,
    notification: HashMap<String, watch::Receiver<DownloadStatus>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct InnerState {
    downloads: HashMap<String, DownloadTask>,
    // Url -> file_id (key of downloads)
    url_mapping: HashMap<Url, String>,
    // file_id (key of downloads) -> multiple target dir
    symlinks: HashMap<String, HashSet<PathBuf>>,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            downloads: HashMap::new(),
            url_mapping: HashMap::new(),
            symlinks: HashMap::new(),
        }
    }
}

impl DownloadState {
    pub async fn new(
        record_path: impl AsRef<Path>,
        root_dir: impl AsRef<Path>,
        cache_dir: impl AsRef<Path>,
        max_cache_bytes: u64,
    ) -> Self {
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
            cache_dir: cache_dir.as_ref().to_path_buf(),
            root_dir: root_dir.as_ref().to_path_buf(),
            max_cache_bytes,
            notification: HashMap::new(),
        }
    }

    async fn dump(&self) -> anyhow::Result<()> {
        let json_str = serde_json::to_string(&self.inner)?;
        tokio::fs::write(&self.record_path, json_str).await?;
        Ok(())
    }

    pub fn get_by_id(&self, id: &str) -> Option<&DownloadTask> {
        self.inner.downloads.get(id)
    }

    pub fn get_by_url(&self, url: &Url) -> Option<&DownloadTask> {
        let file_id = self.inner.url_mapping.get(url).map(|v| v.clone());

        match file_id {
            Some(file_id) => self.get_by_id(&file_id),
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

    pub async fn add_target_dir(
        &mut self,
        file_id: &str,
        target_dir: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        let target_dir = target_dir.as_ref().to_path_buf();

        match self.inner.symlinks.get_mut(file_id) {
            Some(target_dirs) => {
                target_dirs.insert(target_dir);
            }
            None => {
                let mut target_dirs = HashSet::new();
                target_dirs.insert(target_dir);
                self.inner.symlinks.insert(file_id.to_string(), target_dirs);
            }
        }

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

        self.inner.symlinks.remove(file_id);

        self.dump().await?;

        Ok(removed)
    }

    pub fn target_dirs(&self, file_id: &str) -> Option<&HashSet<PathBuf>> {
        self.inner.symlinks.get(file_id)
    }

    pub fn remove_target_dirs(&mut self, file_id: &str) -> Option<HashSet<PathBuf>> {
        self.inner.symlinks.remove(file_id)
    }

    pub fn root_dir(&self) -> &PathBuf {
        &self.root_dir
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn max_cache_bytes(&self) -> u64 {
        self.max_cache_bytes
    }

    pub fn get_notification(&self, file_id: &str) -> Option<watch::Receiver<DownloadStatus>> {
        self.notification.get(file_id).map(|v| v.clone())
    }

    pub fn set_notification(
        &mut self,
        file_id: &str,
        notification: watch::Receiver<DownloadStatus>,
    ) {
        self.notification.insert(file_id.to_string(), notification);
    }

    pub fn remove_notification(&mut self, file_id: &str) -> Option<watch::Receiver<DownloadStatus>> {
        self.notification.remove(file_id)
    }
}
