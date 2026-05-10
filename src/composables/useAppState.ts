import { computed, ref, onMounted, onUnmounted } from "vue";
import { getAppState } from "@/lib/commands";
import {
  onRecordingStarted,
  onRecordingStopped,
  onTranscriptionStarted,
  onTranscriptionComplete,
  onTranscriptionError,
  onAppStateChanged,
  onCorrectionStarted,
  onCorrectionComplete,
  onAudioAmplitude,
  onPartialTranscription,
  type TranscriptionResult,
  type AudioAmplitudePayload,
} from "@/lib/events";
import type { UnlistenFn } from "@tauri-apps/api/event";

export function useAppState() {
  const recording = ref(false);
  const processing = ref(false);
  const hasModel = ref(false);
  const lastTranscription = ref("");
  const correcting = ref(false);
  const error = ref<string | null>(null);
  const transitionSource = ref<'idle' | 'processing' | null>(null);
  const recordingGeneration = ref(0);
  const amplitude = ref(0);
  const partialText = ref("");
  // Drives the auto-dismissing error pill in OverlayApp.vue. We use a
  // tick counter rather than a Date.now() comparison so the pill clears
  // reactively without polling. The setTimeout is held in a ref so a new
  // error replaces a still-visible older one cleanly.
  const errorTick = ref(0);
  let errorTimer: ReturnType<typeof setTimeout> | null = null;
  const ERROR_VISIBLE_MS = 4000;
  const showError = computed(() => !!error.value && errorTick.value > 0);

  function flashError(message: string) {
    error.value = message;
    errorTick.value++;
    if (errorTimer) clearTimeout(errorTimer);
    errorTimer = setTimeout(() => {
      errorTick.value = 0;
      error.value = null;
    }, ERROR_VISIBLE_MS);
  }

  const unlisteners: UnlistenFn[] = [];

  onMounted(async () => {
    // Get initial state
    try {
      const state = await getAppState();
      recording.value = state.recording;
      processing.value = state.processing;
      hasModel.value = state.hasModel;
      lastTranscription.value = state.lastTranscription;
    } catch (e) {
      console.error("Failed to get app state:", e);
    }

    // Listen for state changes
    unlisteners.push(
      await onRecordingStarted(() => {
        transitionSource.value = processing.value ? 'processing' : 'idle';
        recording.value = true;
        processing.value = false;
        error.value = null;
        // Dismiss any auto-fading error pill as soon as the user retries.
        errorTick.value = 0;
        if (errorTimer) {
          clearTimeout(errorTimer);
          errorTimer = null;
        }
        recordingGeneration.value++;
        // New recording wipes any leftover partial caption from the prior session.
        partialText.value = "";
      }),
    );

    unlisteners.push(
      await onRecordingStopped(() => {
        recording.value = false;
        amplitude.value = 0;
      }),
    );

    unlisteners.push(
      await onTranscriptionStarted(() => {
        processing.value = true;
      }),
    );

    unlisteners.push(
      await onCorrectionStarted(() => {
        correcting.value = true;
      }),
    );

    unlisteners.push(
      await onCorrectionComplete(() => {
        correcting.value = false;
      }),
    );

    unlisteners.push(
      await onTranscriptionComplete((result: TranscriptionResult) => {
        processing.value = false;
        correcting.value = false;
        lastTranscription.value = result.text;
        // Final result has replaced the live preview — clear so the
        // overlay's processing pill renders without a stale caption.
        partialText.value = "";
      }),
    );

    unlisteners.push(
      await onPartialTranscription((data) => {
        partialText.value = data.partial;
      }),
    );

    unlisteners.push(
      await onTranscriptionError((err) => {
        processing.value = false;
        correcting.value = false;
        // Drop any stale partial caption — a failed final pass means the
        // preview text is no longer the user's intended output.
        partialText.value = "";
        flashError(err.error);
      }),
    );

    unlisteners.push(
      await onAudioAmplitude((data: AudioAmplitudePayload) => {
        amplitude.value = data.amplitude;
      }),
    );

    unlisteners.push(
      await onAppStateChanged((state) => {
        recording.value = state.recording;
        processing.value = state.processing;
        hasModel.value = state.hasModel;
        lastTranscription.value = state.lastTranscription;
      }),
    );
  });

  onUnmounted(() => {
    unlisteners.forEach((unlisten) => unlisten());
    if (errorTimer) {
      clearTimeout(errorTimer);
      errorTimer = null;
    }
  });

  return {
    recording,
    processing,
    correcting,
    hasModel,
    lastTranscription,
    error,
    showError,
    transitionSource,
    recordingGeneration,
    amplitude,
    partialText,
  };
}
