import { invoke } from '@tauri-apps/api/core';

// ── Types ──────────────────────────────────────────────────────────

export interface AppState {
  recording: boolean;
  processing: boolean;
  hasModel: boolean;
  lastTranscription: string;
}

export interface ModelInfo {
  id: string;
  filename: string;
  displayName: string;
  description: string;
  sizeBytes: number;
  url: string;
  englishOnly: boolean;
  speedRating: number;
  accuracyRating: number;
  /** Optional CoreML encoder package URL fetched alongside the GGML file. */
  encoderUrl: string | null;
  /** Approximate encoder package size in bytes (when present). */
  encoderSizeBytes: number | null;
  /**
   * Tag the picker uses to highlight a recommended default. Currently
   * "english" or "multilingual"; null means no special treatment.
   */
  recommendedFor: string | null;
}

export interface CorrectionModelInfo {
  id: string;
  filename: string;
  displayName: string;
  description: string;
  sizeBytes: number;
  url: string;
  speedRating: number;
  qualityRating: number;
}

export interface PermissionsStatus {
  microphone: boolean;
  /** Accessibility — needed by enigo to post Cmd+V keystrokes. */
  accessibility: boolean;
  /** Input Monitoring — needed by CGEventTap to receive Fn key events.
   *  On macOS 10.15+ this is a separate TCC pane from Accessibility. */
  inputMonitoring: boolean;
}

export interface UserSettings {
  activationMode: 'holdFn' | 'tapFn' | 'doubleTapFn' | 'shortcut';
  language: string | null;
  selectedModel: string | null;
  autoStart: boolean;
  fillerWords: string[];
  removeFillers: boolean;
  selfCorrection: boolean;
  selectedCorrectionModel: string | null;
  vocabularyLearning: boolean;
  setupComplete: boolean;
  /** Whether the streaming-preview worker emits live partial captions while
   *  recording. Default false. */
  streamingPreview: boolean;
  /** User's custom global shortcut in Tauri format (e.g. "CmdOrCtrl+Shift+R").
   *  `null` = use the built-in default ("CmdOrCtrl+Shift+Space"). */
  customShortcut: string | null;
  /** Which update channel the in-app updater polls. Default 'stable'.
   *  Read in Rust at every update check, so the toggle takes effect on
   *  the next check — no app relaunch required. */
  updateChannel: 'stable' | 'beta';
}

export interface VocabularyEntry {
  wrong: string;
  correct: string;
  source: 'auto' | 'manual';
  confidence: number;
  createdAt: string;
  lastUsed: string;
}

// ── Recording ──────────────────────────────────────────────────────

export const startRecording = () => invoke('start_recording');
export const stopRecording = () => invoke('stop_recording');
export const toggleRecording = () => invoke('toggle_recording');
export const cancelRecording = () => invoke('cancel_recording');

// ── App State ──────────────────────────────────────────────────────

export const getAppState = () => invoke<AppState>('get_app_state');

// ── Models ─────────────────────────────────────────────────────────

export const getAvailableModels = () => invoke<ModelInfo[]>('get_available_models');

export const getDownloadedModels = () => invoke<string[]>('get_downloaded_models');

export const downloadModel = (modelId: string) => invoke('download_model', { modelId });

export const cancelDownload = (modelId: string) => invoke('cancel_download', { modelId });

export const selectModel = (modelId: string) => invoke('select_model', { modelId });

export const deleteModelFile = (modelId: string) => invoke('delete_model_file', { modelId });

// ── Correction Models ─────────────────────────────────────────────

export const getAvailableCorrectionModels = () =>
  invoke<CorrectionModelInfo[]>('get_available_correction_models');

export const getDownloadedCorrectionModels = () =>
  invoke<string[]>('get_downloaded_correction_models');

export const downloadCorrectionModel = (modelId: string) =>
  invoke('download_correction_model', { modelId });

export const selectCorrectionModel = (modelId: string) =>
  invoke('select_correction_model', { modelId });

export const deleteCorrectionModelFile = (modelId: string) =>
  invoke('delete_correction_model_file', { modelId });

// ── Hotkey ─────────────────────────────────────────────────────────

export const restartFnKeyMonitor = () => invoke<boolean>('restart_fn_key_monitor');

export const getFnKeyMonitorStatus = () => invoke<boolean>('get_fn_key_monitor_status');

/** Re-register the global shortcut. Pass `null` to revert to the default. */
export const updateGlobalShortcut = (shortcut: string | null) =>
  invoke<void>('update_global_shortcut', { shortcut });

// ── Permissions ────────────────────────────────────────────────────

export const checkPermissions = () => invoke<PermissionsStatus>('check_permissions');

export const requestMicrophonePermission = () => invoke<boolean>('request_microphone_permission');

export const openMicrophoneSettings = () => invoke('open_microphone_settings');

export const openAccessibilitySettings = () => invoke('open_accessibility_settings');

export const requestInputMonitoringPermission = () =>
  invoke<boolean>('request_input_monitoring_permission');

export const openInputMonitoringSettings = () => invoke('open_input_monitoring_settings');

export const restartApp = () => invoke<void>('restart_app');

// ── Settings ───────────────────────────────────────────────────────

export const getSettings = () => invoke<UserSettings>('get_settings');

export const updateSettings = (settings: UserSettings) => invoke('update_settings', { settings });

// ── Launch at login ────────────────────────────────────────────────

export type LaunchAtLoginStatus = 'enabled' | 'notRegistered' | 'requiresApproval' | 'notFound';

export const getLaunchAtLoginStatus = () =>
  invoke<LaunchAtLoginStatus>('get_launch_at_login_status');

export const openLoginItemsSettings = () => invoke<void>('open_login_items_settings');

// ── Updater (channel-aware) ───────────────────────────────────────

/** Metadata returned by `magpie_updater_check` when an update is available. */
export interface UpdaterCheckResult {
  /** Version advertised by the channel manifest (e.g. "0.2.0" or "0.2.0-beta.3"). */
  version: string;
  /** Version baked into the running binary at build time. */
  currentVersion: string;
  /** Release notes / changelog from the manifest, if any. */
  body: string | null;
  /** ISO-8601 publish date from the manifest, if any. */
  date: string | null;
}

export const magpieUpdaterCheck = () => invoke<UpdaterCheckResult | null>('magpie_updater_check');

export const magpieUpdaterInstall = () => invoke<void>('magpie_updater_install');

// ── Vocabulary ────────────────────────────────────────────────────

export const getVocabulary = () => invoke<VocabularyEntry[]>('get_vocabulary');

export const addVocabularyEntry = (wrong: string, correct: string) =>
  invoke('add_vocabulary_entry', { wrong, correct });

export const removeVocabularyEntry = (wrong: string) =>
  invoke('remove_vocabulary_entry', { wrong });

export const clearVocabulary = () => invoke('clear_vocabulary');
