//! On-disk transcript history.
//!
//! A bounded ring of the most recent dictation transcripts, written to
//! `history.json` next to `settings.json` in the same `ProjectDirs` data
//! directory. The cap is supplied by the caller from [`UserSettings`] —
//! the struct itself is cap-agnostic so a shrink in Settings can use
//! [`History::truncate_to`] without re-plumbing.
//!
//! On-disk shape (versioned envelope, mirrors `settings/`):
//! ```json
//! {
//!   "version": 1,
//!   "history": { "nextId": 42, "entries": [ ... newest first ... ] }
//! }
//! ```

pub mod error;

pub use error::{HistoryError, Result};

use std::collections::VecDeque;
use std::path::PathBuf;

use chrono::Utc;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Minimum allowed cap. Below this the history is barely useful.
pub const HISTORY_MIN_ENTRIES: u32 = 10;
/// Upper bound to keep `history.json` from growing unboundedly.
pub const HISTORY_MAX_ENTRIES: u32 = 500;
/// Default cap for fresh installs.
pub const HISTORY_DEFAULT_ENTRIES: u32 = 50;

const CURRENT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: u64,
    pub text: String,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    /// Decode time reported by the transcription backend.
    pub duration_ms: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct HistoryPayload {
    next_id: u64,
    entries: VecDeque<HistoryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryFile {
    version: u32,
    history: Value,
}

/// In-memory ring. Front = newest.
#[derive(Debug, Default)]
pub struct History {
    next_id: u64,
    entries: VecDeque<HistoryEntry>,
}

fn history_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "magpie", "Magpie").ok_or(HistoryError::NoDataDir)?;
    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|source| HistoryError::Io {
        path: data_dir.to_path_buf(),
        source,
    })?;
    Ok(data_dir.join("history.json"))
}

/// Parse the on-disk JSON string into a [`History`]. Pure — no filesystem
/// access — so unit tests can exercise the shape without touching disk.
pub fn parse_versioned_history(contents: &str) -> Result<History> {
    let raw: Value = serde_json::from_str(contents)?;

    let (payload_v, payload) = match raw.get("version").and_then(Value::as_u64) {
        Some(v) => {
            let inner = raw
                .get("history")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            (v as u32, inner)
        }
        // No envelope: treat the whole document as the v1 payload.
        None => (CURRENT_VERSION, raw),
    };

    if payload_v > CURRENT_VERSION {
        return Err(HistoryError::VersionTooNew {
            found: payload_v,
            supported: CURRENT_VERSION,
        });
    }

    let payload: HistoryPayload = serde_json::from_value(payload).unwrap_or_default();
    Ok(History {
        next_id: payload.next_id,
        entries: payload.entries,
    })
}

/// Serialize a [`History`] into the current versioned envelope string.
pub fn serialize_versioned_history(h: &History) -> Result<String> {
    let payload = HistoryPayload {
        next_id: h.next_id,
        entries: h.entries.clone(),
    };
    let file = HistoryFile {
        version: CURRENT_VERSION,
        history: serde_json::to_value(payload)?,
    };
    Ok(serde_json::to_string_pretty(&file)?)
}

impl History {
    /// Load from disk. A missing, unreadable, corrupt, or future-version
    /// file logs a warning and yields an empty ring — never panics,
    /// mirroring [`UserSettings::load`].
    pub fn load() -> Self {
        let path = match history_path() {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Could not determine history path, using empty ring: {}", e);
                return Self::default();
            }
        };

        if !path.exists() {
            return Self::default();
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("Failed to read history file, using empty ring: {}", e);
                return Self::default();
            }
        };

        match parse_versioned_history(&contents) {
            Ok(h) => {
                log::info!(
                    "Loaded {} history entries from {}",
                    h.entries.len(),
                    path.display()
                );
                h
            }
            Err(e) => {
                log::warn!("History file unusable ({}); using empty ring", e);
                Self::default()
            }
        }
    }

    /// Persist via tmp-file + rename so a crash mid-write cannot leave a
    /// truncated `history.json` on disk.
    pub fn save(&self) -> Result<()> {
        let path = history_path()?;
        let json = serialize_versioned_history(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|source| HistoryError::Io {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, &path).map_err(|source| HistoryError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(())
    }

    /// Prepend a new entry with a monotonic id, then drop oldest from the
    /// back until `len <= cap`. `cap` is clamped to at least 1 to avoid a
    /// completely silent eviction loop if the caller passes 0.
    pub fn push(&mut self, text: String, duration_ms: u64, cap: usize) {
        let entry = HistoryEntry {
            id: self.next_id,
            text,
            created_at: Utc::now().timestamp_millis(),
            duration_ms,
        };
        self.next_id = self.next_id.saturating_add(1);
        self.entries.push_front(entry);
        let cap = cap.max(1);
        while self.entries.len() > cap {
            self.entries.pop_back();
        }
    }

    /// Shrink to `cap` by dropping the oldest entries. No-op when
    /// `cap >= len`. Used by the settings command when the user lowers
    /// `historyMaxEntries`.
    pub fn truncate_to(&mut self, cap: usize) {
        let cap = cap.max(1);
        while self.entries.len() > cap {
            self.entries.pop_back();
        }
    }

    /// Clone all entries, newest first.
    pub fn all(&self) -> Vec<HistoryEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_empty() {
        let h = History::default();
        let s = serialize_versioned_history(&h).expect("serialize empty");
        let h2 = parse_versioned_history(&s).expect("parse empty");
        assert_eq!(h2.entries.len(), 0);
        assert_eq!(h2.next_id, 0);
    }

    #[test]
    fn push_assigns_monotonic_id_and_trims_to_cap() {
        let mut h = History::default();
        for i in 0..15 {
            h.push(format!("entry {}", i), 0, 10);
        }
        assert_eq!(h.entries.len(), 10);
        assert_eq!(h.entries.front().expect("has front").text, "entry 14");
        assert_eq!(h.entries.back().expect("has back").text, "entry 5");
        assert_eq!(h.next_id, 15);
    }

    #[test]
    fn next_id_survives_round_trip() {
        let mut h = History::default();
        for i in 0..5 {
            h.push(format!("e{}", i), 0, 100);
        }
        let s = serialize_versioned_history(&h).expect("serialize");
        let mut h2 = parse_versioned_history(&s).expect("parse");
        assert_eq!(h2.next_id, 5);
        h2.push("after-reload".into(), 0, 100);
        assert_eq!(h2.entries.front().expect("has front").id, 5);
    }

    #[test]
    fn truncate_to_evicts_oldest() {
        let mut h = History::default();
        for i in 0..10 {
            h.push(format!("e{}", i), 0, 100);
        }
        h.truncate_to(3);
        assert_eq!(h.entries.len(), 3);
        assert_eq!(h.entries.front().expect("has front").text, "e9");
        assert_eq!(h.entries.back().expect("has back").text, "e7");
    }

    #[test]
    fn truncate_to_is_noop_when_cap_larger_than_len() {
        let mut h = History::default();
        h.push("only".into(), 0, 100);
        h.truncate_to(100);
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn truncate_to_zero_keeps_at_least_one_entry() {
        // Defensive: a 0 cap shouldn't wipe out a just-pushed entry. We
        // clamp internally to 1 — the public API contract is "never silently
        // delete everything".
        let mut h = History::default();
        h.push("one".into(), 0, 100);
        h.truncate_to(0);
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn clear_empties_but_preserves_next_id() {
        let mut h = History::default();
        for i in 0..3 {
            h.push(format!("e{}", i), 0, 10);
        }
        let nid = h.next_id;
        h.clear();
        assert_eq!(h.entries.len(), 0);
        assert_eq!(h.next_id, nid);
    }

    #[test]
    fn rejects_future_version() {
        let json = format!(
            r#"{{"version": {}, "history": {{}}}}"#,
            CURRENT_VERSION + 1
        );
        let err = parse_versioned_history(&json).expect_err("future version rejected");
        assert!(matches!(err, HistoryError::VersionTooNew { .. }));
    }

    #[test]
    fn corrupt_json_returns_parse_error() {
        let err = parse_versioned_history("nope {").expect_err("corrupt JSON rejected");
        assert!(matches!(err, HistoryError::Parse(_)));
    }

    #[test]
    fn empty_envelope_payload_loads_as_empty() {
        let json = format!(
            r#"{{"version": {}, "history": {{}}}}"#,
            CURRENT_VERSION
        );
        let h = parse_versioned_history(&json).expect("empty envelope loads");
        assert_eq!(h.entries.len(), 0);
        assert_eq!(h.next_id, 0);
    }
}
