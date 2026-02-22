//! Asset management for local image caching.
//!
//! Downloads and stores game cover art and screenshots from IGDB CDN
//! to the local filesystem for offline access.

use std::path::PathBuf;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use tauri_plugin_http::reqwest::{self, Client};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Errors that can occur during asset management operations.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// An HTTP request to download an image failed.
    #[error("http request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// File system operation failed.
    #[error("filesystem error: {0}")]
    Filesystem(#[from] std::io::Error),

    /// Failed to download image after all retry attempts.
    #[error("failed to download {0} after 3 attempts")]
    DownloadFailed(String),
}

/// Manages downloading and storing game images locally.
pub struct AssetManager {
    assets_dir: PathBuf,
    client: Client,
}

impl AssetManager {
    /// Creates a new AssetManager instance.
    ///
    /// # Arguments
    ///
    /// * `app_dir` — Application data directory path.
    ///
    /// # Returns
    ///
    /// Returns `Result<Self, AssetError>` with the initialized manager.
    pub async fn new(app_dir: PathBuf) -> Result<Self, AssetError> {
        let assets_dir = app_dir.join("assets");

        // Create assets directories if they don't exist
        fs::create_dir_all(assets_dir.join("covers")).await?;
        fs::create_dir_all(assets_dir.join("artworks")).await?;

        Ok(AssetManager {
            assets_dir,
            client: Client::new(),
        })
    }

    /// Downloads a batch of cover images concurrently.
    ///
    /// Downloads up to 5 images in parallel with retry logic. Skips images
    /// that already exist locally.
    ///
    /// # Arguments
    ///
    /// * `image_ids` — List of IGDB image IDs to download.
    ///
    /// # Returns
    ///
    /// Returns `Vec<(String, String)>` containing tuples of (image_id, local_path)
    /// for successfully downloaded images.
    pub async fn download_batch_covers(
        &self,
        image_ids: Vec<String>,
    ) -> Result<Vec<(String, String)>, AssetError> {
        let results: Vec<_> = stream::iter(image_ids)
            .map(|image_id| self.download_cover(image_id))
            .buffer_unordered(5) // Limit to 5 concurrent downloads
            .collect()
            .await;

        // Filter out errors and collect successful downloads
        let successful: Vec<_> = results.into_iter().filter_map(Result::ok).collect();

        Ok(successful)
    }

    /// Downloads a batch of artwork images concurrently.
    ///
    /// Downloads up to 5 images in parallel with retry logic. Skips images
    /// that already exist locally.
    ///
    /// # Arguments
    ///
    /// * `image_ids` — List of IGDB image IDs to download.
    ///
    /// # Returns
    ///
    /// Returns `Vec<(String, String)>` containing tuples of (image_id, local_path)
    /// for successfully downloaded images.
    pub async fn download_batch_artworks(
        &self,
        image_ids: Vec<String>,
    ) -> Result<Vec<(String, String)>, AssetError> {
        let results: Vec<_> = stream::iter(image_ids)
            .map(|image_id| self.download_artwork(image_id))
            .buffer_unordered(5) // Limit to 5 concurrent downloads
            .collect()
            .await;

        // Filter out errors and collect successful downloads
        let successful: Vec<_> = results.into_iter().filter_map(Result::ok).collect();

        Ok(successful)
    }

    /// Downloads a single cover image with retry logic.
    async fn download_cover(&self, image_id: String) -> Result<(String, String), AssetError> {
        let local_path = self
            .assets_dir
            .join("covers")
            .join(format!("{}.jpg", image_id));

        // Skip if already exists
        if local_path.exists() {
            return Ok((image_id, local_path.to_string_lossy().to_string()));
        }

        let url = format!(
            "https://images.igdb.com/igdb/image/upload/t_cover_small/{}.jpg",
            image_id
        );

        self.download_with_retry(&url, &local_path).await?;

        Ok((image_id, local_path.to_string_lossy().to_string()))
    }

    /// Downloads a single artwork image with retry logic.
    async fn download_artwork(&self, image_id: String) -> Result<(String, String), AssetError> {
        let local_path = self
            .assets_dir
            .join("artworks")
            .join(format!("{}.jpg", image_id));

        // Skip if already exists
        if local_path.exists() {
            return Ok((image_id, local_path.to_string_lossy().to_string()));
        }

        let url = format!(
            "https://images.igdb.com/igdb/image/upload/t_1080p/{}.jpg",
            image_id
        );

        self.download_with_retry(&url, &local_path).await?;

        Ok((image_id, local_path.to_string_lossy().to_string()))
    }

    /// Downloads a file from URL to local path with exponential backoff retry.
    ///
    /// Attempts download up to 3 times with delays of 1s, 2s, 4s between attempts.
    /// Uses atomic write pattern (download to .tmp file, then rename).
    async fn download_with_retry(&self, url: &str, local_path: &PathBuf) -> Result<(), AssetError> {
        let tmp_path = local_path.with_extension("tmp");
        let max_attempts = 3;

        for attempt in 0..max_attempts {
            match self.try_download(url, &tmp_path).await {
                Ok(_) => {
                    // Atomic rename from .tmp to final path
                    fs::rename(&tmp_path, local_path).await?;
                    return Ok(());
                }
                Err(_) => {
                    // Clean up tmp file on error
                    let _ = fs::remove_file(&tmp_path).await;

                    if attempt < max_attempts - 1 {
                        // Exponential backoff: 1s, 2s, 4s
                        let delay = Duration::from_secs(2_u64.pow(attempt as u32));
                        tokio::time::sleep(delay).await;
                    } else {
                        return Err(AssetError::DownloadFailed(url.to_string()));
                    }
                }
            }
        }

        Err(AssetError::DownloadFailed(url.to_string()))
    }

    /// Attempts a single download operation.
    async fn try_download(&self, url: &str, tmp_path: &PathBuf) -> Result<(), AssetError> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(AssetError::Request(
                response.error_for_status().unwrap_err(),
            ));
        }

        let bytes = response.bytes().await?;

        let mut file = fs::File::create(tmp_path).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;

        Ok(())
    }

    /// Clears all locally cached images.
    ///
    /// Removes the entire assets directory and recreates it empty.
    /// Called during database refresh to prevent orphaned files.
    pub async fn clear_all(&self) -> Result<(), AssetError> {
        // Remove the entire assets directory
        if self.assets_dir.exists() {
            fs::remove_dir_all(&self.assets_dir).await?;
        }

        // Recreate empty directories
        fs::create_dir_all(self.assets_dir.join("covers")).await?;
        fs::create_dir_all(self.assets_dir.join("artworks")).await?;

        Ok(())
    }
}
