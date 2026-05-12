use std::path::PathBuf;

use directories::ProjectDirs;

use crate::models::{ModelError, Result};

/// Wrap a `std::io::Error` with the path that caused it.
fn io_at(path: PathBuf) -> impl FnOnce(std::io::Error) -> ModelError {
    move |source| ModelError::Io { path, source }
}

/// Get the models directory, creating it if necessary
pub fn models_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie").ok_or(ModelError::NoDataDir)?;

    let models_dir = proj_dirs.data_dir().join("models");
    std::fs::create_dir_all(&models_dir).map_err(io_at(models_dir.clone()))?;

    Ok(models_dir)
}

/// Get the full path for a model file
pub fn model_path(filename: &str) -> Result<PathBuf> {
    Ok(models_dir()?.join(filename))
}

/// List all downloaded model files
pub fn list_downloaded_models() -> Result<Vec<String>> {
    let dir = models_dir()?;
    let mut models = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir).map_err(io_at(dir.clone()))? {
            let entry = entry.map_err(io_at(dir.clone()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("ggml-") && name.ends_with(".bin") {
                models.push(name);
            }
        }
    }

    Ok(models)
}

/// Delete a model file from disk
pub fn delete_model(filename: &str) -> Result<()> {
    let path = model_path(filename)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(io_at(path))?;
    }
    Ok(())
}

/// List all downloaded correction model files (GGUF format)
pub fn list_downloaded_correction_models() -> Result<Vec<String>> {
    let dir = models_dir()?;
    let mut models = Vec::new();

    if dir.exists() {
        for entry in std::fs::read_dir(&dir).map_err(io_at(dir.clone()))? {
            let entry = entry.map_err(io_at(dir.clone()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".gguf") {
                models.push(name);
            }
        }
    }

    Ok(models)
}

/// Delete a correction model file from disk
pub fn delete_correction_model(filename: &str) -> Result<()> {
    let path = model_path(filename)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(io_at(path))?;
    }
    Ok(())
}
