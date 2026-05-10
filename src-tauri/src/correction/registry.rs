use serde::{Deserialize, Serialize};

/// Information about an available correction model
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionModelInfo {
    /// Unique identifier (e.g. "qwen2.5-0.5b")
    pub id: String,
    /// Filename on disk (e.g. "qwen2.5-0.5b-instruct-q8_0.gguf")
    pub filename: String,
    /// Human-readable display name
    pub display_name: String,
    /// Description of capabilities
    pub description: String,
    /// Download size in bytes
    pub size_bytes: u64,
    /// Download URL
    pub url: String,
    /// Relative speed (1 = slowest, 5 = fastest)
    pub speed_rating: u8,
    /// Relative quality (1 = lowest, 5 = highest)
    pub quality_rating: u8,
}

/// Get the full list of available correction models
pub fn get_available_correction_models() -> Vec<CorrectionModelInfo> {
    vec![
        CorrectionModelInfo {
            id: "qwen2.5-0.5b".to_string(),
            filename: "qwen2.5-0.5b-instruct-q8_0.gguf".to_string(),
            display_name: "Qwen 0.5B (Fast)".to_string(),
            description: "Lightweight and fast. Good for simple corrections.".to_string(),
            size_bytes: 676_000_000,
            url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf".to_string(),
            speed_rating: 5,
            quality_rating: 3,
        },
        CorrectionModelInfo {
            id: "qwen2.5-1.5b".to_string(),
            filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            display_name: "Qwen 1.5B (Quality)".to_string(),
            description: "Better accuracy for subtle corrections. Recommended.".to_string(),
            size_bytes: 1_120_000_000,
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            speed_rating: 3,
            quality_rating: 5,
        },
    ]
}

/// Find a correction model by its ID
pub fn find_correction_model(id: &str) -> Option<CorrectionModelInfo> {
    get_available_correction_models()
        .into_iter()
        .find(|m| m.id == id)
}
