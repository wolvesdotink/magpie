//! Automatic correction detection.
//!
//! After text is pasted into the active application, this module monitors the
//! focused text field via the macOS Accessibility API. It takes two snapshots —
//! one shortly after paste and another after a delay — then diffs them to
//! detect word-level corrections the user made. Detected corrections are
//! stored in the vocabulary for future transcription biasing.

use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;

use crate::accessibility;
use crate::events::{self, event_names, VocabularyLearnedPayload};
use crate::frontmost_app::FrontmostApp;
use crate::state::{lock_or_recover, AppState};
use crate::vocabulary::VocabularySource;

/// How long to wait after paste before taking the first snapshot (ms).
const SETTLE_DELAY_MS: u64 = 500;

/// How long to wait for the user to make corrections (ms).
const DETECTION_WINDOW_MS: u64 = 5000;

/// Minimum ratio of text that must remain for us to consider it a correction
/// rather than a deletion. If the user deleted >80% of the text, skip.
const MIN_TEXT_RETENTION_RATIO: f64 = 0.2;

/// Start monitoring the focused text field for corrections.
///
/// Spawns a background thread that:
/// 1. Waits for paste to settle
/// 2. Reads the focused element's text (snapshot 1)
/// 3. Waits for the user to make corrections
/// 4. Reads again (snapshot 2)
/// 5. Diffs to find word-level corrections
/// 6. Stores any corrections in the matched profile's vocabulary (if any) or
///    the global vocabulary, based on the resolution captured at recording time.
pub fn start_detection(
    pasted_text: String,
    state: Arc<AppState>,
    app: AppHandle,
    recording_app: Option<FrontmostApp>,
) {
    let _ = std::thread::Builder::new()
        .name("correction-detector".into())
        .spawn(move || {
            if let Err(e) = run_detection(&pasted_text, &state, &app, recording_app.as_ref()) {
                log::debug!("Correction detection skipped: {}", e);
            }
        })
        .map_err(|e| {
            log::warn!("Failed to spawn correction detector thread: {}", e);
        });
}

fn run_detection(
    pasted_text: &str,
    state: &Arc<AppState>,
    app: &AppHandle,
    recording_app: Option<&FrontmostApp>,
) -> Result<(), String> {
    // Wait for paste to settle
    std::thread::sleep(Duration::from_millis(SETTLE_DELAY_MS));

    // Abort if the user started another recording
    if state.is_recording() || state.is_processing() {
        return Err("Recording or processing started, aborting detection".into());
    }

    // Take snapshot 1: read the focused text field
    let snapshot1 = accessibility::get_focused_element_value()
        .ok_or("Could not read focused element (AX API unavailable or unsupported app)")?;

    // Verify the pasted text appears in the field
    if !snapshot1.contains(pasted_text) {
        return Err("Pasted text not found in focused element".into());
    }

    // Wait for user to make corrections
    std::thread::sleep(Duration::from_millis(DETECTION_WINDOW_MS));

    // Abort if the user started another recording
    if state.is_recording() || state.is_processing() {
        return Err("Recording started during detection window, aborting".into());
    }

    // Take snapshot 2: read again
    let snapshot2 = accessibility::get_focused_element_value()
        .ok_or("Could not read focused element for second snapshot")?;

    // If the text is the same, no corrections were made
    if snapshot1 == snapshot2 {
        log::debug!("No corrections detected (text unchanged)");
        return Ok(());
    }

    // Find where the pasted text was in snapshot1
    let paste_start = snapshot1
        .find(pasted_text)
        .ok_or("Pasted text disappeared from snapshot")?;
    let paste_end = paste_start + pasted_text.len();

    // Extract the region that was pasted in snapshot1
    let original_region = &snapshot1[paste_start..paste_end];

    // In snapshot2, extract the corresponding region.
    // The text before the paste region should be the same (prefix).
    let prefix = &snapshot1[..paste_start];
    if !snapshot2.starts_with(prefix) {
        return Err("Text before paste region changed, can't reliably diff".into());
    }

    // The suffix after the pasted region in snapshot1
    let suffix = &snapshot1[paste_end..];

    // Find where the suffix starts in snapshot2
    let modified_region = if suffix.is_empty() {
        // Pasted text was at the end
        &snapshot2[paste_start..]
    } else if let Some(suffix_pos) = snapshot2[paste_start..].find(suffix) {
        &snapshot2[paste_start..paste_start + suffix_pos]
    } else {
        // Suffix not found — text around the paste region changed significantly
        return Err("Text after paste region changed significantly".into());
    };

    // Check text retention ratio
    if modified_region.len() as f64 / original_region.len() as f64 <= MIN_TEXT_RETENTION_RATIO {
        return Err("Too much text deleted, skipping (not a correction)".into());
    }

    // Diff at word level
    let corrections = diff_for_corrections(original_region, modified_region);

    if corrections.is_empty() {
        log::debug!("No word-level corrections detected");
        return Ok(());
    }

    // Find the target profile (if any). Profile lookup is independent of
    // global vocabulary, so do it BEFORE acquiring the vocabulary lock.
    let target_profile_id = recording_app.and_then(|app| {
        let p = lock_or_recover(&state.profiles);
        p.find_by_bundle(&app.bundle_id).map(|p| p.id.clone())
    });

    if let Some(ref profile_id) = target_profile_id {
        // Profile-scoped attribution.
        let mut profiles = lock_or_recover(&state.profiles);
        for (wrong, correct) in &corrections {
            if let Err(e) =
                profiles.add_vocab_to_profile(profile_id, wrong, correct, VocabularySource::Auto)
            {
                log::warn!(
                    "Failed to attribute vocab to profile {}: {}; falling back to global",
                    profile_id,
                    e
                );
                let mut vocab = lock_or_recover(&state.vocabulary);
                vocab.add_or_update(wrong, correct, VocabularySource::Auto);
                if let Err(e) = vocab.save() {
                    log::error!("Failed to save vocabulary after fallback learning: {}", e);
                }
                continue;
            }
            events::emit_event(
                app,
                event_names::VOCABULARY_LEARNED,
                VocabularyLearnedPayload {
                    wrong: wrong.clone(),
                    correct: correct.clone(),
                    profile_id: Some(profile_id.clone()),
                },
            );
            log::info!(
                "Auto-learned vocabulary correction for profile {}: \"{}\" -> \"{}\"",
                profile_id,
                wrong,
                correct
            );
        }
        if let Err(e) = profiles.save() {
            log::error!(
                "Failed to save profiles after profile-scoped learning: {}",
                e
            );
        }
    } else {
        // Global attribution.
        let mut vocab = lock_or_recover(&state.vocabulary);
        for (wrong, correct) in &corrections {
            vocab.add_or_update(wrong, correct, VocabularySource::Auto);
            events::emit_event(
                app,
                event_names::VOCABULARY_LEARNED,
                VocabularyLearnedPayload {
                    wrong: wrong.clone(),
                    correct: correct.clone(),
                    profile_id: None,
                },
            );
            log::info!(
                "Auto-learned vocabulary correction (global): \"{}\" -> \"{}\"",
                wrong,
                correct
            );
        }
        if let Err(e) = vocab.save() {
            log::error!("Failed to save vocabulary after learning: {}", e);
        }
    }

    Ok(())
}

/// Extract word-level substitutions between two texts.
///
/// Uses a simple LCS-based diff to find words that were replaced
/// (not inserted or deleted). Returns (wrong, correct) pairs.
fn diff_for_corrections(original: &str, modified: &str) -> Vec<(String, String)> {
    let orig_words: Vec<&str> = original.split_whitespace().collect();
    let mod_words: Vec<&str> = modified.split_whitespace().collect();

    if orig_words.is_empty() || mod_words.is_empty() {
        return Vec::new();
    }

    // Build LCS table
    let n = orig_words.len();
    let m = mod_words.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];

    for i in 1..=n {
        for j in 1..=m {
            if orig_words[i - 1].to_lowercase() == mod_words[j - 1].to_lowercase() {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to find the alignment
    let mut corrections = Vec::new();
    let mut i = n;
    let mut j = m;

    // Collect edit operations in reverse
    #[derive(Debug)]
    enum Op {
        Match,
        Delete(usize), // index in orig_words
        Insert(usize), // index in mod_words
    }

    let mut ops = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && orig_words[i - 1].to_lowercase() == mod_words[j - 1].to_lowercase() {
            ops.push(Op::Match);
            i -= 1;
            j -= 1;
        } else if i > 0 && (j == 0 || dp[i - 1][j] >= dp[i][j - 1]) {
            ops.push(Op::Delete(i - 1));
            i -= 1;
        } else if j > 0 {
            ops.push(Op::Insert(j - 1));
            j -= 1;
        }
    }

    ops.reverse();

    // Look for adjacent Delete+Insert or Insert+Delete pairs (substitutions).
    // The backtracking order can produce either sequence depending on the
    // LCS table tie-breaking direction.
    let mut idx = 0;
    while idx < ops.len() {
        let pair = match (&ops[idx], ops.get(idx + 1)) {
            (Op::Delete(d_idx), Some(Op::Insert(i_idx))) => {
                Some((orig_words[*d_idx], mod_words[*i_idx]))
            }
            (Op::Insert(i_idx), Some(Op::Delete(d_idx))) => {
                Some((orig_words[*d_idx], mod_words[*i_idx]))
            }
            _ => None,
        };

        if let Some((orig_word, mod_word)) = pair {
            // Only record if the words are actually different
            if orig_word.to_lowercase() != mod_word.to_lowercase() {
                // Strip trailing punctuation for the vocabulary entry
                let clean_orig = orig_word.trim_matches(|c: char| !c.is_alphanumeric());
                let clean_mod = mod_word.trim_matches(|c: char| !c.is_alphanumeric());
                if !clean_orig.is_empty() && !clean_mod.is_empty() {
                    corrections.push((clean_orig.to_string(), clean_mod.to_string()));
                }
            }
            idx += 2;
        } else {
            idx += 1;
        }
    }

    corrections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_single_word_substitution() {
        let corrections = diff_for_corrections(
            "Hello Marshal nice to meet you",
            "Hello Marcel nice to meet you",
        );
        assert_eq!(corrections.len(), 1);
        assert_eq!(
            corrections[0],
            ("Marshal".to_string(), "Marcel".to_string())
        );
    }

    #[test]
    fn test_diff_multiple_substitutions() {
        let corrections =
            diff_for_corrections("I use cubernetes and dok", "I use Kubernetes and Docker");
        assert_eq!(corrections.len(), 2);
        assert!(corrections.contains(&("cubernetes".to_string(), "Kubernetes".to_string())));
        assert!(corrections.contains(&("dok".to_string(), "Docker".to_string())));
    }

    #[test]
    fn test_diff_no_changes() {
        let corrections = diff_for_corrections("Hello world", "Hello world");
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_diff_insertion_ignored() {
        // Pure insertions should not be treated as corrections
        let corrections = diff_for_corrections("Hello world", "Hello beautiful world");
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_diff_deletion_ignored() {
        // Pure deletions should not be treated as corrections
        let corrections = diff_for_corrections("Hello beautiful world", "Hello world");
        assert!(corrections.is_empty());
    }

    #[test]
    fn test_diff_with_punctuation() {
        let corrections =
            diff_for_corrections("Hello Marshal, how are you?", "Hello Marcel, how are you?");
        assert_eq!(corrections.len(), 1);
        assert_eq!(
            corrections[0],
            ("Marshal".to_string(), "Marcel".to_string())
        );
    }

    #[test]
    fn test_diff_empty_strings() {
        let corrections = diff_for_corrections("", "");
        assert!(corrections.is_empty());

        let corrections = diff_for_corrections("hello", "");
        assert!(corrections.is_empty());
    }
}
