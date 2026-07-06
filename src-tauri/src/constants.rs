/// Whisper sample rate requirement
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Streaming preview decodes only the trailing window of audio, in seconds.
/// The live caption is transient UI text that only ever shows the most
/// recent phrases, so a sliding window keeps each partial decode
/// O(window) instead of O(recording length). The final on-stop pass still
/// transcribes the full clip.
pub const PARTIAL_WINDOW_SECS: usize = 12;

/// Delay before restoring clipboard after paste (ms)
pub const CLIPBOARD_RESTORE_DELAY_MS: u64 = 150;

/// Threads for whisper inference. Detected once per process: the macOS
/// performance-core count (`hw.perflevel0.physicalcpu`), falling back to the
/// physical core count elsewhere, clamped to [2, 8]. Efficiency cores slow a
/// ggml thread pool down (the pool runs at the pace of its slowest member),
/// and whisper.cpp scales poorly past ~8 threads, hence the cap.
pub fn whisper_threads() -> i32 {
    inference_threads()
}

/// Threads for LLM (self-correction) inference. Same detection as
/// [`whisper_threads`]; correction runs are short and latency-bound, so they
/// get the full performance-core pool too.
pub fn llm_threads() -> i32 {
    inference_threads()
}

fn inference_threads() -> i32 {
    static THREADS: std::sync::OnceLock<i32> = std::sync::OnceLock::new();
    *THREADS.get_or_init(|| {
        let physical = performance_cores().unwrap_or_else(num_cpus::get_physical);
        let threads = (physical as i32).clamp(2, 8);
        log::info!(
            "Inference thread pool: {threads} threads ({physical} performance/physical cores detected)"
        );
        threads
    })
}

/// Performance-core count on Apple Silicon. `num_cpus::get_physical` counts
/// efficiency cores too (e.g. 10 on an 8P+2E M1 Pro), which overcommits the
/// ggml pool, so prefer the perflevel0 sysctl. Runs once, behind the
/// `inference_threads` OnceLock.
#[cfg(target_os = "macos")]
fn performance_cores() -> Option<usize> {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "hw.perflevel0.physicalcpu"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

#[cfg(not(target_os = "macos"))]
fn performance_cores() -> Option<usize> {
    None
}

/// Model download base URL (HuggingFace)
pub const MODEL_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// Distil-Whisper variants live in their own HuggingFace repos because the
/// distillations were not folded back into ggerganov/whisper.cpp.
pub const DISTIL_WHISPER_SMALL_EN_BASE: &str =
    "https://huggingface.co/distil-whisper/distil-small.en/resolve/main";
pub const DISTIL_WHISPER_LARGE_V3_BASE: &str =
    "https://huggingface.co/distil-whisper/distil-large-v3-ggml/resolve/main";

/// Maximum allowed output/input length ratio for correction validation
pub const CORRECTION_MAX_OUTPUT_MULTIPLIER: f32 = 2.0;

/// Minimum word overlap ratio between original and corrected text
pub const CORRECTION_MIN_WORD_OVERLAP: f64 = 0.3;

/// HTTP connect timeout for model downloads (seconds)
pub const DOWNLOAD_CONNECT_TIMEOUT_SECS: u64 = 30;

/// HTTP read timeout for model downloads — if no data received for this long,
/// the download is considered stalled (seconds)
pub const DOWNLOAD_READ_TIMEOUT_SECS: u64 = 60;

/// Memory Saver: unload resident models after this many seconds without any
/// dictation activity. ~5 minutes balances "free the RAM promptly when the
/// user walks away" against "don't pay a reload on every brief pause".
pub const IDLE_UNLOAD_SECS: u64 = 300;

/// How often the Memory Saver idle-unload watchdog wakes to check inactivity.
pub const IDLE_CHECK_INTERVAL_SECS: u64 = 30;
