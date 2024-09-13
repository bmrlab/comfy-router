use super::state::DownloadState;
use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;

pub async fn manage_cache(
    cache_dir: impl AsRef<Path>,
    download_state: Arc<RwLock<DownloadState>>,
    max_cache_bytes: u64,
) -> anyhow::Result<()> {
    let mut read_dir = tokio::fs::read_dir(cache_dir).await?;
    let mut files = vec![];

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        files.push(entry);
    }

    let mut files_with_info = vec![];
    for file in &files {
        let metadata = file.metadata().await?;
        if metadata.is_file() {
            if let Ok(modified) = metadata.modified() {
                files_with_info.push((file, metadata, modified));
            }
        }
    }

    files_with_info.sort_by(|a, b| {
        // NOTICE here compare right with left, which should output reversed order
        b.2.cmp(&a.2)
    });

    let mut current_size = 0u64;
    for (_, metadata, _) in &files_with_info {
        current_size += metadata.size();
    }

    // Remove oldest files if cache size exceeds the limit
    while current_size > max_cache_bytes && !files_with_info.is_empty() {
        if let Some((oldest_file, metadata, _)) = files_with_info.pop() {
            tracing::info!(
                "cache exceed limit ({} Bytes), deleting {}",
                max_cache_bytes,
                oldest_file.path().to_string_lossy()
            );

            let file_size = metadata.size();
            tokio::fs::remove_file(oldest_file.path()).await?;
            current_size -= file_size;

            // Remove the corresponding download entry and symlink
            let file_id = oldest_file.file_name().to_string_lossy().into_owned();
            let mut state = download_state.write().await;
            if let Ok(Some(download)) = state.remove(&file_id).await {
                tokio::fs::remove_file(PathBuf::from(download.target_folder()).join(&file_id))
                    .await?;
            }
        }
    }

    Ok(())
}
