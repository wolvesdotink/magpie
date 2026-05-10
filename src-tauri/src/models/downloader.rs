use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

use crate::constants;
use crate::events::{self, event_names, ModelDownloadProgress};

/// Download a model file from the given URL to the models directory.
/// Emits progress events to the frontend during download.
///
/// `expected_bytes` is the approximate size from the model registry,
/// used as a sanity check after download completes.
pub async fn download_model(
    app: &AppHandle,
    model_id: &str,
    url: &str,
    filename: &str,
    expected_bytes: u64,
) -> Result<PathBuf> {
    let dest_path = super::storage::model_path(filename)?;
    let temp_path = dest_path.with_extension("bin.downloading");

    log::info!("Downloading model {} from {}", model_id, url);

    // Use a configured client with connect and read timeouts
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(constants::DOWNLOAD_CONNECT_TIMEOUT_SECS))
        .read_timeout(Duration::from_secs(constants::DOWNLOAD_READ_TIMEOUT_SECS))
        .build()
        .context("Failed to create HTTP client")?;

    // Inner block so we can clean up the temp file on any error
    let download_result: Result<(PathBuf, u64)> = async {
        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to start download")?
            .error_for_status()
            .context("Server returned an error")?;

        let total_bytes = response.content_length().unwrap_or(0);
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .context("Failed to create temp file")?;

        let mut downloaded: u64 = 0;
        let mut last_progress_pct: f64 = -1.0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Download stream error")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk")?;

            downloaded += chunk.len() as u64;

            // Emit progress at most every 1%
            let pct = if total_bytes > 0 {
                (downloaded as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };

            if (pct - last_progress_pct) >= 1.0 {
                last_progress_pct = pct;
                events::emit_event(
                    app,
                    event_names::MODEL_DOWNLOAD_PROGRESS,
                    ModelDownloadProgress {
                        model_id: model_id.to_string(),
                        percent: pct,
                        bytes_downloaded: downloaded,
                        total_bytes,
                    },
                );
            }
        }

        file.flush().await?;
        drop(file);

        // Validate downloaded size against Content-Length
        if total_bytes > 0 && downloaded != total_bytes {
            anyhow::bail!(
                "Incomplete download: got {} bytes, expected {} from Content-Length",
                downloaded,
                total_bytes,
            );
        }

        // Validate against expected size from registry (approximate — allow 10% tolerance)
        if expected_bytes > 0 && (downloaded as f64) < (expected_bytes as f64 * 0.9) {
            anyhow::bail!(
                "Downloaded file too small: got {} bytes, expected ~{} bytes",
                downloaded,
                expected_bytes,
            );
        }

        // Rename temp file to final path (atomic on same filesystem)
        tokio::fs::rename(&temp_path, &dest_path)
            .await
            .context("Failed to rename downloaded file")?;

        log::info!(
            "Model {} downloaded successfully ({} bytes)",
            model_id,
            downloaded
        );

        Ok((dest_path, downloaded))
    }
    .await;

    // Clean up temp file on any error
    match download_result {
        Ok((path, _)) => Ok(path),
        Err(e) => {
            log::error!("Download failed, cleaning up temp file: {}", e);
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(e)
        }
    }
}
