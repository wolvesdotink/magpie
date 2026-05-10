use serde::{Deserialize, Serialize};

use crate::constants::MODEL_BASE_URL;

/// Information about an available whisper model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Unique identifier (e.g. "base.en")
    pub id: String,
    /// Filename on disk (e.g. "ggml-base.en.bin")
    pub filename: String,
    /// Human-readable display name
    pub display_name: String,
    /// Description of capabilities
    pub description: String,
    /// Download size in bytes
    pub size_bytes: u64,
    /// Download URL
    pub url: String,
    /// Whether this is English-only
    pub english_only: bool,
    /// Relative speed (1 = fastest, 5 = slowest)
    pub speed_rating: u8,
    /// Relative accuracy (1 = lowest, 5 = highest)
    pub accuracy_rating: u8,
}

/// Get the full list of available models
pub fn get_available_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "tiny.en".to_string(),
            filename: "ggml-tiny.en.bin".to_string(),
            display_name: "Tiny (English)".to_string(),
            description: "Fastest model, good for quick dictation. English only.".to_string(),
            size_bytes: 77_704_715,
            url: format!("{}/ggml-tiny.en.bin", MODEL_BASE_URL),
            english_only: true,
            speed_rating: 1,
            accuracy_rating: 2,
        },
        ModelInfo {
            id: "base.en".to_string(),
            filename: "ggml-base.en.bin".to_string(),
            display_name: "Base (English)".to_string(),
            description: "Good balance of speed and accuracy. Recommended for most users."
                .to_string(),
            size_bytes: 147_951_465,
            url: format!("{}/ggml-base.en.bin", MODEL_BASE_URL),
            english_only: true,
            speed_rating: 2,
            accuracy_rating: 3,
        },
        ModelInfo {
            id: "small.en".to_string(),
            filename: "ggml-small.en.bin".to_string(),
            display_name: "Small (English)".to_string(),
            description: "Better accuracy for accents and complex speech. English only."
                .to_string(),
            size_bytes: 487_601_967,
            url: format!("{}/ggml-small.en.bin", MODEL_BASE_URL),
            english_only: true,
            speed_rating: 3,
            accuracy_rating: 4,
        },
        ModelInfo {
            id: "medium.en".to_string(),
            filename: "ggml-medium.en.bin".to_string(),
            display_name: "Medium (English)".to_string(),
            description: "High accuracy, slower transcription. English only.".to_string(),
            size_bytes: 1_533_774_781,
            url: format!("{}/ggml-medium.en.bin", MODEL_BASE_URL),
            english_only: true,
            speed_rating: 4,
            accuracy_rating: 5,
        },
        ModelInfo {
            id: "tiny".to_string(),
            filename: "ggml-tiny.bin".to_string(),
            display_name: "Tiny (Multilingual)".to_string(),
            description: "Fastest model with multilingual support. Good for quick dictation in any language.".to_string(),
            size_bytes: 77_691_713,
            url: format!("{}/ggml-tiny.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 1,
            accuracy_rating: 2,
        },
        ModelInfo {
            id: "base".to_string(),
            filename: "ggml-base.bin".to_string(),
            display_name: "Base (Multilingual)".to_string(),
            description: "Good balance of speed and accuracy. Supports 100+ languages.".to_string(),
            size_bytes: 147_964_211,
            url: format!("{}/ggml-base.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 2,
            accuracy_rating: 3,
        },
        ModelInfo {
            id: "small".to_string(),
            filename: "ggml-small.bin".to_string(),
            display_name: "Small (Multilingual)".to_string(),
            description: "Better accuracy for accents and complex speech. Supports 100+ languages.".to_string(),
            size_bytes: 487_626_497,
            url: format!("{}/ggml-small.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 3,
            accuracy_rating: 4,
        },
        ModelInfo {
            id: "medium".to_string(),
            filename: "ggml-medium.bin".to_string(),
            display_name: "Medium (Multilingual)".to_string(),
            description: "High accuracy, slower transcription. Supports 100+ languages.".to_string(),
            size_bytes: 1_533_774_781,
            url: format!("{}/ggml-medium.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 4,
            accuracy_rating: 5,
        },
        ModelInfo {
            id: "large-v3-turbo".to_string(),
            filename: "ggml-large-v3-turbo.bin".to_string(),
            display_name: "Large V3 Turbo (Multilingual)".to_string(),
            description: "Best quality, supports 100+ languages. Larger download.".to_string(),
            size_bytes: 1_623_232_473,
            url: format!("{}/ggml-large-v3-turbo.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 4,
            accuracy_rating: 5,
        },
    ]
}

/// Find a model by its ID
pub fn find_model(id: &str) -> Option<ModelInfo> {
    get_available_models().into_iter().find(|m| m.id == id)
}
