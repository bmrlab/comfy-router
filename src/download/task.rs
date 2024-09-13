use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadTask {
    // the url used to identify the file, without any search parameters
    url: Url,
    downloadable_url: Url,
    target_folder: String,
    status: DownloadStatus,
    file_id: String,
}

pub async fn run_download_task(
    task: &DownloadTask,
    cache_dir: impl AsRef<Path>,
) -> anyhow::Result<()> {
    tracing::debug!("task started {}", &task.file_id);

    let cache_path = PathBuf::from(cache_dir.as_ref()).join(task.file_id());
    let target_path = PathBuf::from(task.target_folder()).join(task.file_id());

    tokio::fs::create_dir_all(&cache_path.parent().expect("cache path should have parent")).await?;
    tokio::fs::create_dir_all(
        &target_path
            .parent()
            .expect("target path should have parent"),
    )
    .await?;

    let client = Client::new();
    let res = client.get(task.downloadable_url().clone()).send().await?;
    let total_size = res.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(&cache_path).await?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk).await?;
        let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
    }

    file.flush().await?;

    tokio::fs::symlink(&cache_path, &target_path).await?;

    tracing::debug!("task completed {}", &task.file_id);

    Ok(())
}

impl DownloadTask {
    pub fn new(url: &Url, target_folder: &str) -> Self {
        let mut file_id = uuid::Uuid::new_v4().to_string();

        // preserve the file extension if any
        if let Some(segments) = url.path_segments() {
            if let Some(file_name) = segments.last() {
                let file_path = PathBuf::from(file_name);
                if let Some(extension) = file_path.extension() {
                    file_id = format!("{}.{}", file_id, extension.to_string_lossy());
                }
            }
        }

        let downloadable_url = url.clone();
        let mut url = url.clone();
        url.set_query(None);

        Self {
            url,
            downloadable_url,
            target_folder: target_folder.to_string(),
            status: DownloadStatus::Pending,
            file_id,
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn target_folder(&self) -> &str {
        &self.target_folder
    }

    pub fn status(&self) -> &DownloadStatus {
        &self.status
    }

    pub fn file_id(&self) -> &str {
        &self.file_id
    }

    pub(self) fn downloadable_url(&self) -> &Url {
        &self.downloadable_url
    }

    pub fn with_status(&mut self, status: DownloadStatus) {
        self.status = status;
    }
}
