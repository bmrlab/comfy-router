use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadTask {
    /// The URL used to identify the file, without any search parameters.
    url: Url,
    downloadable_url: Url,
    status: DownloadStatus,
    file_id: String,
}

impl DownloadTask {
    pub fn new(url: &Url) -> Self {
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
            status: DownloadStatus::Pending,
            file_id,
        }
    }

    #[tracing::instrument(skip_all, fields(file_id = self.file_id))]
    pub async fn run(&self, cache_dir: impl AsRef<Path>) -> anyhow::Result<()> {
        tracing::info!("task started {}", &self.file_id);

        let cache_path = cache_dir.as_ref().to_path_buf().join(self.file_id());
        let download_path = cache_path.with_extension("download");

        if let Some(parent) = cache_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let client = Client::new();
        let res = client.get(self.downloadable_url().clone()).send().await?;
        let total_size = res.content_length().unwrap_or(0);

        let mut file = tokio::fs::File::create(&download_path).await?;
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            file.write_all(&chunk).await?;
            let new = std::cmp::min(downloaded + (chunk.len() as u64), total_size);
            downloaded = new;
        }

        file.flush().await?;
        tokio::fs::rename(&download_path, &cache_path).await?;

        tracing::debug!("task completed {}", self.file_id());

        Ok(())
    }

    pub fn url(&self) -> &Url {
        &self.url
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
