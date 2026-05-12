// Mirror of `src-tauri/src/features.rs::FeatureFlags`.
//
// Resolves on the backend (settings + env overrides) and is fetched once
// at app start via the `get_feature_flags` Tauri command. The frontend
// guards UI affordances behind these — e.g. show the "Transcribe files…"
// tray item only when `fileImport` is true.
//
// Adding a new flag: add the field here AND in features.rs in the same PR
// and update ADR-0002.

export interface FeatureFlags {
  streamingPreview: boolean;
  fileImport: boolean;
  batchTranscription: boolean;
  transcriptionHistory: boolean;
  vocabularyExport: boolean;
}

/** Safe-by-default fallback used before the backend responds. */
export const DEFAULT_FEATURE_FLAGS: FeatureFlags = {
  streamingPreview: false,
  fileImport: false,
  batchTranscription: false,
  transcriptionHistory: false,
  vocabularyExport: false,
};
