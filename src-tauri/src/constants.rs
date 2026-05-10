/// Whisper sample rate requirement
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Delay before restoring clipboard after paste (ms)
pub const CLIPBOARD_RESTORE_DELAY_MS: u64 = 150;

/// Default number of threads for whisper inference
pub const DEFAULT_WHISPER_THREADS: i32 = 4;

/// Model download base URL (HuggingFace)
pub const MODEL_BASE_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Default number of threads for LLM inference
pub const DEFAULT_LLM_THREADS: i32 = 4;

/// Maximum allowed output/input length ratio for correction validation
pub const CORRECTION_MAX_OUTPUT_MULTIPLIER: f32 = 2.0;

/// Minimum word overlap ratio between original and corrected text
pub const CORRECTION_MIN_WORD_OVERLAP: f64 = 0.3;

/// HTTP connect timeout for model downloads (seconds)
pub const DOWNLOAD_CONNECT_TIMEOUT_SECS: u64 = 30;

/// HTTP read timeout for model downloads — if no data received for this long,
/// the download is considered stalled (seconds)
pub const DOWNLOAD_READ_TIMEOUT_SECS: u64 = 60;
