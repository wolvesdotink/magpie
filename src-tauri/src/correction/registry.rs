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
            id: "qwen3.5-0.8b".to_string(),
            filename: "Qwen3.5-0.8B-Q8_0.gguf".to_string(),
            display_name: "Qwen3.5 0.8B (Fast)".to_string(),
            description: "Lightweight and fast. Stronger than Qwen 0.5B at the same speed tier."
                .to_string(),
            size_bytes: 811_843_840,
            url: "https://huggingface.co/unsloth/Qwen3.5-0.8B-GGUF/resolve/main/Qwen3.5-0.8B-Q8_0.gguf".to_string(),
            speed_rating: 5,
            quality_rating: 3,
        },
        CorrectionModelInfo {
            id: "qwen3.5-2b".to_string(),
            filename: "Qwen3.5-2B-Q4_K_M.gguf".to_string(),
            display_name: "Qwen3.5 2B (Recommended)".to_string(),
            description: "Best balance of speed and accuracy. Recommended.".to_string(),
            size_bytes: 1_280_835_840,
            url: "https://huggingface.co/unsloth/Qwen3.5-2B-GGUF/resolve/main/Qwen3.5-2B-Q4_K_M.gguf".to_string(),
            speed_rating: 4,
            quality_rating: 4,
        },
        CorrectionModelInfo {
            id: "qwen3.5-4b".to_string(),
            filename: "Qwen3.5-4B-Q4_K_M.gguf".to_string(),
            display_name: "Qwen3.5 4B (Best quality)".to_string(),
            description: "Highest correction quality. Needs ~3 GB of disk and a capable machine."
                .to_string(),
            size_bytes: 2_740_937_888,
            url: "https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/main/Qwen3.5-4B-Q4_K_M.gguf".to_string(),
            speed_rating: 2,
            quality_rating: 5,
        },
        CorrectionModelInfo {
            id: "qwen2.5-0.5b".to_string(),
            filename: "qwen2.5-0.5b-instruct-q8_0.gguf".to_string(),
            display_name: "Qwen 0.5B (Fast, previous gen)".to_string(),
            description: "Previous generation. Prefer Qwen3.5 0.8B for new downloads.".to_string(),
            size_bytes: 676_000_000,
            url: "https://huggingface.co/Qwen/Qwen2.5-0.5B-Instruct-GGUF/resolve/main/qwen2.5-0.5b-instruct-q8_0.gguf".to_string(),
            speed_rating: 5,
            quality_rating: 2,
        },
        CorrectionModelInfo {
            id: "qwen2.5-1.5b".to_string(),
            filename: "qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            display_name: "Qwen 1.5B (previous gen)".to_string(),
            description: "Previous generation. Prefer Qwen3.5 2B for new downloads.".to_string(),
            size_bytes: 1_120_000_000,
            url: "https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/qwen2.5-1.5b-instruct-q4_k_m.gguf".to_string(),
            speed_rating: 3,
            quality_rating: 3,
        },
    ]
}

/// Find a correction model by its ID
pub fn find_correction_model(id: &str) -> Option<CorrectionModelInfo> {
    get_available_correction_models()
        .into_iter()
        .find(|m| m.id == id)
}
