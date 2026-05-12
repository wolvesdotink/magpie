use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures_util::StreamExt;
use tauri::AppHandle;
use tokio::io::AsyncWriteExt;

use crate::constants;
use crate::events::{self, event_names, ModelDownloadProgress};
use crate::models::{ModelError, Result};
use crate::transcription::backend::CancellationToken;

/// Optional CoreML encoder package to fetch alongside the GGML weights.
pub struct EncoderSpec<'a> {
    pub url: &'a str,
    /// Approximate size in bytes — used to weight progress between the
    /// GGML download and the encoder download (GGML gets the lion's share
    /// since it is typically 5–10× larger than the encoder package).
    pub size_bytes: u64,
}

/// Wrap a `std::io::Error` with the path that caused it.
fn io_at(path: PathBuf) -> impl FnOnce(std::io::Error) -> ModelError {
    move |source| ModelError::Io { path, source }
}

/// Download a model file from the given URL to the models directory.
///
/// If `encoder` is provided, the matching `*-encoder.mlmodelc.zip` is also
/// fetched and unpacked next to the GGML file so whisper.cpp can run the
/// encoder on the ANE. Encoder fetch failures are logged but do **not**
/// fail the call — CoreML acceleration is a perf optimization, not a
/// correctness requirement; Metal still runs the model end-to-end.
///
/// `expected_bytes` is the approximate size of the GGML file from the
/// model registry, used as a sanity check after download completes.
pub async fn download_model(
    app: &AppHandle,
    model_id: &str,
    url: &str,
    filename: &str,
    expected_bytes: u64,
    encoder: Option<EncoderSpec<'_>>,
    cancel: Option<&CancellationToken>,
) -> Result<PathBuf> {
    let dest_path = super::storage::model_path(filename)?;
    let temp_path = dest_path.with_extension("bin.downloading");

    log::info!("Downloading model {} from {}", model_id, url);

    let client = build_client()?;

    // Weight progress so encoder download contributes the tail end of the
    // 0–100% range. If there is no encoder, GGML alone is the full range.
    let ggml_progress_share: f64 = match encoder.as_ref() {
        Some(spec) if spec.size_bytes > 0 && expected_bytes > 0 => {
            let total = expected_bytes as f64 + spec.size_bytes as f64;
            (expected_bytes as f64 / total).clamp(0.5, 0.98)
        }
        _ => 1.0,
    };

    // Inner block so we can clean up the temp file on any error
    let download_result: Result<(PathBuf, u64)> = async {
        let response = client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(ModelError::HttpStatus {
                status: status.as_u16(),
                url: url.to_string(),
            });
        }

        let total_bytes = response.content_length().unwrap_or(0);
        let mut stream = response.bytes_stream();
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(io_at(temp_path.clone()))?;

        let mut downloaded: u64 = 0;
        let mut last_progress_pct: f64 = -1.0;

        while let Some(chunk) = stream.next().await {
            if cancel.is_some_and(|c| c.is_cancelled()) {
                return Err(ModelError::Cancelled);
            }
            let chunk = chunk?;
            file.write_all(&chunk)
                .await
                .map_err(io_at(temp_path.clone()))?;

            downloaded += chunk.len() as u64;

            // Emit progress at most every 1%, scaled to the GGML share
            let raw_pct = if total_bytes > 0 {
                (downloaded as f64 / total_bytes as f64) * 100.0
            } else {
                0.0
            };
            let scaled_pct = raw_pct * ggml_progress_share;

            if (scaled_pct - last_progress_pct) >= 1.0 {
                last_progress_pct = scaled_pct;
                events::emit_event(
                    app,
                    event_names::MODEL_DOWNLOAD_PROGRESS,
                    ModelDownloadProgress {
                        model_id: model_id.to_string(),
                        percent: scaled_pct,
                        bytes_downloaded: downloaded,
                        total_bytes,
                    },
                );
            }
        }

        file.flush().await.map_err(io_at(temp_path.clone()))?;
        drop(file);

        // Validate downloaded size against Content-Length
        if total_bytes > 0 && downloaded != total_bytes {
            return Err(ModelError::SizeMismatch {
                expected: total_bytes,
                actual: downloaded,
            });
        }

        // Validate against expected size from registry (approximate — allow 10% tolerance)
        if expected_bytes > 0 && (downloaded as f64) < (expected_bytes as f64 * 0.9) {
            return Err(ModelError::SizeMismatch {
                expected: expected_bytes,
                actual: downloaded,
            });
        }

        // Rename temp file to final path (atomic on same filesystem)
        tokio::fs::rename(&temp_path, &dest_path)
            .await
            .map_err(io_at(dest_path.clone()))?;

        log::info!(
            "Model {} downloaded successfully ({} bytes)",
            model_id,
            downloaded
        );

        Ok((dest_path, downloaded))
    }
    .await;

    let final_path = match download_result {
        Ok((path, _)) => path,
        Err(e) => {
            log::error!("Download failed, cleaning up temp file: {}", e);
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(e);
        }
    };

    // Best-effort encoder fetch. Any failure here is a perf regression
    // (Metal-only fallback), not a correctness issue, so we keep the
    // GGML on disk and log instead of bubbling up. The one exception is
    // user cancellation — propagate that so the GGML on disk also gets
    // cleaned up by the caller's normal cancel flow.
    if let Some(spec) = encoder {
        let encoder_dir_name = encoder_dir_name_from_filename(filename);
        let encoder_dest = super::storage::models_dir()?.join(&encoder_dir_name);

        if encoder_dest.exists() {
            log::info!(
                "CoreML encoder already present at {}, skipping fetch",
                encoder_dest.display()
            );
        } else {
            match fetch_and_unpack_encoder(
                app,
                model_id,
                spec.url,
                spec.size_bytes,
                &encoder_dest,
                &client,
                ggml_progress_share,
                cancel,
            )
            .await
            {
                Ok(()) => {
                    log::info!("CoreML encoder unpacked at {}", encoder_dest.display());
                }
                Err(ModelError::Cancelled) => {
                    // Cancellation during encoder fetch: also remove the
                    // GGML we already wrote, so a re-download starts clean.
                    let _ = tokio::fs::remove_dir_all(&encoder_dest).await;
                    let _ = tokio::fs::remove_file(&final_path).await;
                    return Err(ModelError::Cancelled);
                }
                Err(e) => {
                    log::warn!(
                        "CoreML encoder fetch failed for {} ({}). Model will run on Metal without ANE acceleration.",
                        model_id,
                        e
                    );
                    // Clean up any partial directory so we can retry next time
                    let _ = tokio::fs::remove_dir_all(&encoder_dest).await;
                }
            }
        }
    }

    Ok(final_path)
}

/// Standalone encoder-only download. Used by the startup backfill path
/// (when an existing GGML model lacks its `.mlmodelc`) and the tray
/// "Repair Active Model" command. Removes any pre-existing directory at
/// `encoder_dest` so a corrupt/partial unpack from a prior run is replaced.
///
/// Returns Ok(()) when the encoder is present and validated. Progress is
/// reported via the standard `MODEL_DOWNLOAD_PROGRESS` event mapped over
/// the full 0–100% range (no GGML share).
pub async fn download_encoder_only(
    app: &AppHandle,
    model_id: &str,
    url: &str,
    expected_bytes: u64,
    encoder_dest: &Path,
    cancel: Option<&CancellationToken>,
) -> Result<()> {
    let client = build_client()?;
    if encoder_dest.exists() {
        log::info!(
            "Removing existing encoder dir before refetch: {}",
            encoder_dest.display()
        );
        let _ = tokio::fs::remove_dir_all(encoder_dest).await;
    }
    // ggml_progress_share=0.0 means the encoder owns the entire 0–100% range,
    // since there's no GGML download in this code path.
    fetch_and_unpack_encoder(
        app,
        model_id,
        url,
        expected_bytes,
        encoder_dest,
        &client,
        0.0,
        cancel,
    )
    .await
}

fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(
            constants::DOWNLOAD_CONNECT_TIMEOUT_SECS,
        ))
        .read_timeout(Duration::from_secs(constants::DOWNLOAD_READ_TIMEOUT_SECS))
        .build()?)
}

/// whisper.cpp looks for a CoreML encoder at `<ggml-name>-encoder.mlmodelc/`
/// next to the GGML file — so for `ggml-base.en.bin` the encoder directory
/// is `ggml-base.en-encoder.mlmodelc`.
pub fn encoder_dir_name_from_filename(ggml_filename: &str) -> String {
    let stem = ggml_filename.strip_suffix(".bin").unwrap_or(ggml_filename);
    format!("{}-encoder.mlmodelc", stem)
}

// 8 args is one over the clippy default; refactoring into a struct here
// would just shuffle fields around — every parameter is independently
// supplied at the single call site and there's no natural grouping.
#[allow(clippy::too_many_arguments)]
async fn fetch_and_unpack_encoder(
    app: &AppHandle,
    model_id: &str,
    url: &str,
    expected_bytes: u64,
    dest_dir: &Path,
    client: &reqwest::Client,
    ggml_progress_share: f64,
    cancel: Option<&CancellationToken>,
) -> Result<()> {
    log::info!("Downloading CoreML encoder for {} from {}", model_id, url);

    let response = client.get(url).send().await?;
    let status = response.status();
    if !status.is_success() {
        return Err(ModelError::HttpStatus {
            status: status.as_u16(),
            url: url.to_string(),
        });
    }

    let total_bytes = response.content_length().unwrap_or(expected_bytes);
    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(total_bytes as usize);
    let mut downloaded: u64 = 0;
    let mut last_progress_pct: f64 = -1.0;

    while let Some(chunk) = stream.next().await {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            return Err(ModelError::Cancelled);
        }
        let chunk = chunk?;
        buf.extend_from_slice(&chunk);
        downloaded += chunk.len() as u64;

        // Map encoder progress into the [ggml_share*100, 100] tail of the
        // overall progress range so the user sees a single monotonic bar.
        let encoder_pct = if total_bytes > 0 {
            downloaded as f64 / total_bytes as f64
        } else {
            0.0
        };
        let scaled_pct =
            ggml_progress_share * 100.0 + encoder_pct * (1.0 - ggml_progress_share) * 100.0;

        if (scaled_pct - last_progress_pct) >= 1.0 {
            last_progress_pct = scaled_pct;
            events::emit_event(
                app,
                event_names::MODEL_DOWNLOAD_PROGRESS,
                ModelDownloadProgress {
                    model_id: model_id.to_string(),
                    percent: scaled_pct,
                    bytes_downloaded: downloaded,
                    total_bytes,
                },
            );
        }
    }

    if total_bytes > 0 && downloaded != total_bytes {
        return Err(ModelError::SizeMismatch {
            expected: total_bytes,
            actual: downloaded,
        });
    }

    // Unpack into a temp dir adjacent to the destination, then rename
    // so a half-extracted directory never gets picked up by whisper.cpp.
    let parent = dest_dir.parent().ok_or_else(|| {
        ModelError::EncoderInvalid(format!(
            "encoder destination {} has no parent directory",
            dest_dir.display()
        ))
    })?;
    let temp_dir = parent.join(format!(
        ".{}.unpacking",
        dest_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("encoder")
    ));
    if temp_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(io_at(temp_dir.clone()))?;

    // zip crate is sync, so do the extraction on a blocking thread.
    // Note: cancellation is not honored during this stage — unzip runs to
    // completion (typically a few seconds) before the next cancel check.
    let temp_dir_clone = temp_dir.clone();
    let dest_name = dest_dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            ModelError::EncoderInvalid(format!(
                "encoder destination {} has no filename",
                dest_dir.display()
            ))
        })?;
    let dest_name_for_task = dest_name.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        unzip_into(&buf, &temp_dir_clone, &dest_name_for_task)
    })
    .await
    .map_err(|e| ModelError::EncoderInvalid(format!("unzip task panicked: {e}")))??;

    // Locate the .mlmodelc directory inside the unpacked tree (the zip may
    // include a top-level wrapper folder).
    let unpacked = locate_mlmodelc(&temp_dir, &dest_name)?;

    // Atomically move into place.
    if dest_dir.exists() {
        let _ = tokio::fs::remove_dir_all(dest_dir).await;
    }
    tokio::fs::rename(&unpacked, dest_dir)
        .await
        .map_err(io_at(dest_dir.to_path_buf()))?;

    // Clean up any leftover scaffolding from the temp dir.
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;

    // Validate the unpacked package looks like a CoreML model directory.
    let has_marker = ["model.mil", "coremldata.bin", "model.espresso.net"]
        .iter()
        .any(|m| dest_dir.join(m).exists());
    if !has_marker {
        return Err(ModelError::EncoderInvalid(format!(
            "unpacked encoder at {} is missing expected CoreML markers",
            dest_dir.display()
        )));
    }

    Ok(())
}

fn unzip_into(zip_bytes: &[u8], temp_dir: &Path, _dest_name: &str) -> Result<()> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let entry_path = match entry.enclosed_name() {
            Some(p) => temp_dir.join(p),
            None => continue, // skip suspicious entries (zip-slip guard)
        };

        if entry.is_dir() {
            std::fs::create_dir_all(&entry_path).map_err(io_at(entry_path.clone()))?;
            continue;
        }

        if let Some(parent) = entry_path.parent() {
            std::fs::create_dir_all(parent).map_err(io_at(parent.to_path_buf()))?;
        }

        let mut out = std::fs::File::create(&entry_path).map_err(io_at(entry_path.clone()))?;
        let mut data = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut data)
            .map_err(io_at(entry_path.clone()))?;
        std::io::copy(&mut Cursor::new(&data), &mut out).map_err(io_at(entry_path))?;
    }

    Ok(())
}

fn locate_mlmodelc(temp_dir: &Path, dest_name: &str) -> Result<PathBuf> {
    // Most common layout: zip extracts directly to the named dir.
    let direct = temp_dir.join(dest_name);
    if direct.is_dir() {
        return Ok(direct);
    }

    // Otherwise scan one level deep for any *.mlmodelc directory.
    for entry in std::fs::read_dir(temp_dir).map_err(io_at(temp_dir.to_path_buf()))? {
        let entry = entry.map_err(io_at(temp_dir.to_path_buf()))?;
        let path = entry.path();
        if path.is_dir()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("mlmodelc"))
                .unwrap_or(false)
        {
            return Ok(path);
        }
        // Some zips wrap the .mlmodelc in another folder.
        if path.is_dir() {
            for inner in std::fs::read_dir(&path).map_err(io_at(path.clone()))? {
                let inner = inner.map_err(io_at(path.clone()))?;
                let inner_path = inner.path();
                if inner_path.is_dir()
                    && inner_path
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.eq_ignore_ascii_case("mlmodelc"))
                        .unwrap_or(false)
                {
                    return Ok(inner_path);
                }
            }
        }
    }

    Err(ModelError::EncoderInvalid(format!(
        "no .mlmodelc directory found in unpacked encoder archive at {}",
        temp_dir.display()
    )))
}
