import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface TranscriptionResult {
  text: string;
  durationMs: number;
}

export interface TranscriptionError {
  error: string;
}

export interface PartialTranscriptionPayload {
  partial: string;
  isFinal: boolean;
}

export interface ModelDownloadProgress {
  modelId: string;
  percent: number;
  bytesDownloaded: number;
  totalBytes: number;
}

export interface AppStatePayload {
  recording: boolean;
  processing: boolean;
  hasModel: boolean;
  lastTranscription: string;
}

export interface PermissionsPayload {
  microphone: boolean;
  accessibility: boolean;
  inputMonitoring: boolean;
}

export interface AudioAmplitudePayload {
  amplitude: number;
}

export interface VocabularyLearnedPayload {
  wrong: string;
  correct: string;
}

// ── Event Listeners ────────────────────────────────────────────────

export function onRecordingStarted(callback: () => void): Promise<UnlistenFn> {
  return listen("recording-started", callback);
}

export function onRecordingStopped(callback: () => void): Promise<UnlistenFn> {
  return listen("recording-stopped", callback);
}

export function onTranscriptionStarted(
  callback: () => void,
): Promise<UnlistenFn> {
  return listen("transcription-started", callback);
}

export function onTranscriptionComplete(
  callback: (result: TranscriptionResult) => void,
): Promise<UnlistenFn> {
  return listen("transcription-complete", (event) => {
    callback(event.payload as TranscriptionResult);
  });
}

export function onTranscriptionError(
  callback: (error: TranscriptionError) => void,
): Promise<UnlistenFn> {
  return listen("transcription-error", (event) => {
    callback(event.payload as TranscriptionError);
  });
}

export function onPartialTranscription(
  callback: (data: PartialTranscriptionPayload) => void,
): Promise<UnlistenFn> {
  return listen("partial-transcription", (event) => {
    callback(event.payload as PartialTranscriptionPayload);
  });
}

export function onModelDownloadProgress(
  callback: (progress: ModelDownloadProgress) => void,
): Promise<UnlistenFn> {
  return listen("model-download-progress", (event) => {
    callback(event.payload as ModelDownloadProgress);
  });
}

export function onModelDownloadComplete(
  callback: (data: { modelId: string }) => void,
): Promise<UnlistenFn> {
  return listen("model-download-complete", (event) => {
    callback(event.payload as { modelId: string });
  });
}

export function onModelDownloadCancelled(
  callback: (data: { modelId: string }) => void,
): Promise<UnlistenFn> {
  return listen("model-download-cancelled", (event) => {
    callback(event.payload as { modelId: string });
  });
}

export function onAppStateChanged(
  callback: (state: AppStatePayload) => void,
): Promise<UnlistenFn> {
  return listen("app-state-changed", (event) => {
    callback(event.payload as AppStatePayload);
  });
}

export function onPermissionsStatus(
  callback: (status: PermissionsPayload) => void,
): Promise<UnlistenFn> {
  return listen("permissions-status", (event) => {
    callback(event.payload as PermissionsPayload);
  });
}

export function onCorrectionStarted(
  callback: () => void,
): Promise<UnlistenFn> {
  return listen("correction-started", callback);
}

export function onCorrectionComplete(
  callback: () => void,
): Promise<UnlistenFn> {
  return listen("correction-complete", callback);
}

export function onAudioAmplitude(
  callback: (data: AudioAmplitudePayload) => void,
): Promise<UnlistenFn> {
  return listen("audio-amplitude", (event) => {
    callback(event.payload as AudioAmplitudePayload);
  });
}

export function onVocabularyLearned(
  callback: (data: VocabularyLearnedPayload) => void,
): Promise<UnlistenFn> {
  return listen("vocabulary-learned", (event) => {
    callback(event.payload as VocabularyLearnedPayload);
  });
}
