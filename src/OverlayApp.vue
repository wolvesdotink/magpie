<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useAppState } from '@/composables/useAppState';
import RecordingPill from '@/components/overlay/RecordingPill.vue';
import TranscribingPill from '@/components/overlay/TranscribingPill.vue';
import PartialCaption from '@/components/overlay/PartialCaption.vue';
import ErrorPill from '@/components/overlay/ErrorPill.vue';

const {
  recording,
  processing,
  correcting,
  transitionSource,
  recordingGeneration,
  amplitude,
  partialText,
  error,
  showError,
} = useAppState();

// The processing → recording transition is intentionally instant — see
// the .pill-instant-* CSS classes below. Every other entry uses the
// pop/blur curve of `pill`.
const pillTransitionName = computed(() =>
  recording.value && transitionSource.value === 'processing' ? 'pill-instant' : 'pill',
);

function onAfterEnter() {
  transitionSource.value = null;
}

// Mirror of `recordingGeneration` for the transcribing state. We bump it
// whenever processing flips true so TranscribingPill's label-swap timer
// resets without us having to add another field to AppState.
const transcribingGeneration = ref(0);
watch(processing, (isProc) => {
  if (isProc) transcribingGeneration.value++;
});
</script>

<template>
  <div class="overlay-container">
    <div class="pill-stack">
      <Transition :name="pillTransitionName" @after-enter="onAfterEnter">
        <div v-if="recording" :key="'recording-' + recordingGeneration" class="pill-outer">
          <RecordingPill :amplitude="amplitude" :generation="recordingGeneration" />
        </div>
        <div v-else-if="processing" key="processing" class="pill-outer">
          <TranscribingPill :correcting="correcting" :generation="transcribingGeneration" />
        </div>
      </Transition>
    </div>
    <Transition name="caption">
      <PartialCaption v-if="recording && partialText" :text="partialText" />
    </Transition>
    <Transition name="caption">
      <ErrorPill v-if="showError" :message="error ?? ''" />
    </Transition>
  </div>
</template>

<style scoped>
.overlay-container {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  height: 100%;
  background: transparent;
}

/* Stacking grid so the recording / processing pills can crossfade
   through the same cell without bumping the caption below. */
.pill-stack {
  display: grid;
  place-items: center;
}

/* transform / filter / opacity animate here, so they never touch the
   inner .pill that carries backdrop-filter.                            */
.pill-outer {
  will-change: transform, opacity, filter;
  grid-area: 1 / 1; /* Both pills stack in the same cell for crossfade */
}

/* ---- Pill enter / leave transition (on .pill-outer) ---- */
.pill-enter-from {
  opacity: 0;
  transform: scale(0.82) translateY(3px);
  filter: blur(4px);
}
.pill-enter-active {
  transition:
    opacity 0.35s cubic-bezier(0.34, 1.56, 0.64, 1),
    transform 0.35s cubic-bezier(0.34, 1.56, 0.64, 1),
    filter 0.35s cubic-bezier(0.34, 1.56, 0.64, 1);
}
.pill-enter-to {
  opacity: 1;
  transform: scale(1) translateY(0);
  filter: blur(0);
}

.pill-leave-from {
  opacity: 1;
  transform: scale(1);
  filter: blur(0);
}
.pill-leave-active {
  transition:
    opacity 0.2s ease-in,
    transform 0.2s ease-in,
    filter 0.2s ease-in;
}
.pill-leave-to {
  opacity: 0;
  transform: scale(0.92);
  filter: blur(2px);
}

/* Instant enter (processing → recording) — skips the pop-in curve so the
   user perceives a continuous "still working" sense rather than two
   discrete state changes. */
.pill-instant-enter-from,
.pill-instant-enter-to {
  opacity: 1;
  transform: scale(1) translateY(0);
  filter: blur(0);
}
.pill-instant-enter-active {
  transition: none;
}
.pill-instant-leave-from {
  opacity: 1;
  transform: scale(1);
  filter: blur(0);
}
.pill-instant-leave-active {
  transition:
    opacity 0.2s ease-in,
    transform 0.2s ease-in,
    filter 0.2s ease-in;
}
.pill-instant-leave-to {
  opacity: 0;
  transform: scale(0.92);
  filter: blur(2px);
}

/* ---- Caption transition (shared by PartialCaption + ErrorPill) ---- */
.caption-enter-from {
  opacity: 0;
  transform: translateY(-2px);
  filter: blur(2px);
}
.caption-enter-active {
  transition:
    opacity 0.25s ease-out,
    transform 0.25s ease-out,
    filter 0.25s ease-out;
}
.caption-enter-to {
  opacity: 1;
  transform: translateY(0);
  filter: blur(0);
}

.caption-leave-active {
  transition: opacity 0.18s ease-in;
}
.caption-leave-to {
  opacity: 0;
}
</style>
