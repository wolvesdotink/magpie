use serde::{Deserialize, Serialize};

use crate::constants::{DISTIL_WHISPER_LARGE_V3_BASE, DISTIL_WHISPER_SMALL_EN_BASE, MODEL_BASE_URL};

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
    /// Download size in bytes (GGML weights only)
    pub size_bytes: u64,
    /// Download URL for the GGML weights
    pub url: String,
    /// Whether this is English-only
    pub english_only: bool,
    /// Relative speed (1 = fastest, 5 = slowest)
    pub speed_rating: u8,
    /// Relative accuracy (1 = lowest, 5 = highest)
    pub accuracy_rating: u8,
    /// Optional CoreML encoder package URL (`*-encoder.mlmodelc.zip`).
    /// When present, the downloader will fetch and unpack it next to the
    /// GGML file so whisper.cpp can run the encoder on the ANE.
    pub encoder_url: Option<String>,
    /// Approximate size of the CoreML encoder package in bytes.
    pub encoder_size_bytes: Option<u64>,
    /// Hint for the UI to highlight a recommended default per locale.
    /// Uses string flags rather than booleans so we can extend with
    /// "recommended-multilingual" etc. without growing the type.
    pub recommended_for: Option<String>,
}

/// Build a CoreML encoder URL from the standard whisper.cpp HF layout.
fn ggerganov_encoder(id: &str) -> String {
    format!("{}/ggml-{}-encoder.mlmodelc.zip", MODEL_BASE_URL, id)
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
            encoder_url: Some(ggerganov_encoder("tiny.en")),
            encoder_size_bytes: Some(15_728_640), // ~15 MB
            recommended_for: None,
        },
        ModelInfo {
            id: "base.en".to_string(),
            filename: "ggml-base.en.bin".to_string(),
            display_name: "Base (English)".to_string(),
            description: "Good balance of speed and accuracy. Reliable starting point."
                .to_string(),
            size_bytes: 147_951_465,
            url: format!("{}/ggml-base.en.bin", MODEL_BASE_URL),
            english_only: true,
            speed_rating: 2,
            accuracy_rating: 3,
            encoder_url: Some(ggerganov_encoder("base.en")),
            encoder_size_bytes: Some(39_845_888), // ~38 MB
            recommended_for: None,
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
            encoder_url: Some(ggerganov_encoder("small.en")),
            encoder_size_bytes: Some(170_917_888), // ~163 MB
            recommended_for: None,
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
            encoder_url: Some(ggerganov_encoder("medium.en")),
            encoder_size_bytes: Some(594_542_592), // ~567 MB
            recommended_for: None,
        },
        // ── Distil-Whisper (English) ─────────────────────────────────────
        // Distillations published by Hugging Face that retain near-parent
        // accuracy at a fraction of the inference cost. They live in their
        // own HF repos and have no CoreML encoder packages — Metal still
        // accelerates them, just without the ANE encoder path.
        ModelInfo {
            id: "distil-small.en".to_string(),
            filename: "ggml-distil-small.en.bin".to_string(),
            display_name: "Distil Small (English)".to_string(),
            description:
                "Distilled Whisper Small — similar accuracy to Small at higher speed. English only."
                    .to_string(),
            size_bytes: 352_321_536, // ~336 MB
            url: format!(
                "{}/ggml-distil-small.en.bin",
                DISTIL_WHISPER_SMALL_EN_BASE
            ),
            english_only: true,
            speed_rating: 2,
            accuracy_rating: 4,
            encoder_url: None,
            encoder_size_bytes: None,
            recommended_for: Some("english".to_string()),
        },
        ModelInfo {
            id: "distil-large-v3".to_string(),
            filename: "ggml-distil-large-v3.bin".to_string(),
            display_name: "Distil Large v3 (English)".to_string(),
            description:
                "Distilled Whisper Large v3 — near-best English accuracy at Turbo-class speed."
                    .to_string(),
            size_bytes: 1_632_087_572, // ~1.52 GB
            url: format!(
                "{}/ggml-distil-large-v3.bin",
                DISTIL_WHISPER_LARGE_V3_BASE
            ),
            english_only: true,
            speed_rating: 4,
            accuracy_rating: 5,
            encoder_url: None,
            encoder_size_bytes: None,
            recommended_for: None,
        },
        // ── Multilingual ─────────────────────────────────────────────────
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
            encoder_url: Some(ggerganov_encoder("tiny")),
            encoder_size_bytes: Some(15_728_640), // ~15 MB
            recommended_for: None,
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
            encoder_url: Some(ggerganov_encoder("base")),
            encoder_size_bytes: Some(39_739_392), // ~37.9 MB
            recommended_for: None,
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
            encoder_url: Some(ggerganov_encoder("small")),
            encoder_size_bytes: Some(170_917_888), // ~163 MB
            recommended_for: None,
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
            encoder_url: Some(ggerganov_encoder("medium")),
            encoder_size_bytes: Some(595_591_168), // ~568 MB
            recommended_for: None,
        },
        ModelInfo {
            id: "large-v3-turbo".to_string(),
            filename: "ggml-large-v3-turbo.bin".to_string(),
            display_name: "Large v3 Turbo (Multilingual)".to_string(),
            description: "Best balance of speed and accuracy across 100+ languages.".to_string(),
            size_bytes: 1_623_232_473,
            url: format!("{}/ggml-large-v3-turbo.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 4,
            accuracy_rating: 5,
            encoder_url: Some(ggerganov_encoder("large-v3-turbo")),
            encoder_size_bytes: Some(1_256_390_656), // ~1.17 GB
            recommended_for: Some("multilingual".to_string()),
        },
        ModelInfo {
            id: "large-v3".to_string(),
            filename: "ggml-large-v3.bin".to_string(),
            display_name: "Large v3 (Multilingual)".to_string(),
            description:
                "Highest accuracy across 100+ languages. Significantly slower than Turbo."
                    .to_string(),
            size_bytes: 3_328_535_756, // ~3.1 GB
            url: format!("{}/ggml-large-v3.bin", MODEL_BASE_URL),
            english_only: false,
            speed_rating: 5,
            accuracy_rating: 5,
            encoder_url: Some(ggerganov_encoder("large-v3")),
            encoder_size_bytes: Some(1_267_914_240), // ~1.18 GB
            recommended_for: None,
        },
    ]
}

/// Find a model by its ID
pub fn find_model(id: &str) -> Option<ModelInfo> {
    get_available_models().into_iter().find(|m| m.id == id)
}
