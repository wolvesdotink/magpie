<script setup lang="ts">
import { computed, ref, watch, watchEffect, onUnmounted } from "vue";
import { useAppState } from "@/composables/useAppState";

const { recording, processing, correcting, transitionSource, recordingGeneration, amplitude, partialText, error, showError } =
  useAppState();

// Truncate long error messages so the pill doesn't grow unbounded; the
// full text still ends up in the Tauri log + dev console.
const ERROR_MAX_CHARS = 80;
const errorShort = computed(() => {
  const msg = error.value ?? "";
  return msg.length > ERROR_MAX_CHARS ? msg.slice(0, ERROR_MAX_CHARS - 1) + "…" : msg;
});

const pillTransitionName = computed(() =>
  recording.value && transitionSource.value === "processing"
    ? "pill-instant"
    : "pill",
);

function onAfterEnter() {
  transitionSource.value = null;
}

/* ---- Label → animation swap timers ---- */
const showRecordingLabel = ref(true);
const showTranscribingLabel = ref(true);
let recTimer: ReturnType<typeof setTimeout> | null = null;
let transTimer: ReturnType<typeof setTimeout> | null = null;

watch(
  [recording, recordingGeneration],
  ([isRec]) => {
    if (recTimer) clearTimeout(recTimer);
    if (isRec) {
      showRecordingLabel.value = true;
      recTimer = setTimeout(() => {
        showRecordingLabel.value = false;
      }, 1400);
    } else {
      showRecordingLabel.value = true;
    }
  },
  { immediate: true },
);

watch(
  processing,
  (isProc) => {
    if (transTimer) clearTimeout(transTimer);
    if (isProc) {
      showTranscribingLabel.value = true;
      transTimer = setTimeout(() => {
        showTranscribingLabel.value = false;
      }, 1400);
    } else {
      showTranscribingLabel.value = true;
    }
  },
  { immediate: true },
);

onUnmounted(() => {
  if (recTimer) clearTimeout(recTimer);
  if (transTimer) clearTimeout(transTimer);
});

/* ---- Voice-driven wave bars ---- */
const smoothAmplitude = ref(0);
let animationFrame: number | null = null;

// 9 bars — asymmetric bell-curve for organic, natural feel
const barWeights = [0.3, 0.55, 0.78, 0.92, 1.0, 0.88, 0.68, 0.45, 0.25];
const MIN_SCALE = 0.10;
const MAX_HEIGHT = 24;

// Per-bar sine noise gives each bar its own "personality"
const barNoises = ref<number[]>(new Array(barWeights.length).fill(0));
let elapsed = 0;

watchEffect((onCleanup) => {
  if (recording.value) {
    const tick = () => {
      // Lerp toward target amplitude — 0.15 = smooth but responsive
      smoothAmplitude.value += (amplitude.value - smoothAmplitude.value) * 0.15;

      // Per-bar organic noise — overlapping sine waves at different frequencies
      // so each bar dances slightly differently, like a real visualizer
      elapsed += 0.018;
      barNoises.value = barWeights.map((_, i) =>
        Math.sin(elapsed * (1.8 + i * 0.4) + i * 1.1) *
        Math.cos(elapsed * (0.7 + i * 0.2) + i * 0.5),
      );

      animationFrame = requestAnimationFrame(tick);
    };
    animationFrame = requestAnimationFrame(tick);

    onCleanup(() => {
      if (animationFrame !== null) {
        cancelAnimationFrame(animationFrame);
        animationFrame = null;
      }
    });
  } else {
    smoothAmplitude.value = 0;
    barNoises.value = new Array(barWeights.length).fill(0);
    elapsed = 0;
  }
});

const barHeights = computed(() => {
  const amp = smoothAmplitude.value;
  return barWeights.map((w, i) => {
    const noise = barNoises.value[i] || 0;
    // Noise contributes more when amplitude is present — bars feel alive when speaking
    const noiseContrib = noise * (0.04 + amp * 0.18);
    const scale = Math.min(1, Math.max(MIN_SCALE, MIN_SCALE + amp * (1 - MIN_SCALE) + noiseContrib));
    return Math.max(2, Math.round(w * scale * MAX_HEIGHT));
  });
});
</script>

<template>
  <div class="overlay-container">
    <div class="pill-stack">
      <Transition :name="pillTransitionName" @after-enter="onAfterEnter">
        <div v-if="recording" :key="'recording-' + recordingGeneration" class="pill-outer">
          <div class="pill recording">
            <div class="dot-wrap">
              <div class="pulse-dot" />
              <div class="sonar-ring" />
              <div class="sonar-ring delay" />
            </div>
            <div class="content-swap">
              <span class="label swap-item" :class="{ active: showRecordingLabel }">Recording</span>
              <div class="wave-bars swap-item" :class="{ active: !showRecordingLabel }">
                <div
                  v-for="(h, i) in barHeights"
                  :key="i"
                  class="wave-bar"
                  :style="{ height: h + 'px', '--i': i }"
                />
              </div>
            </div>
          </div>
        </div>
        <div v-else-if="processing" key="processing" class="pill-outer">
          <div class="pill processing">
            <div class="spinner-wrap">
              <svg class="spinner" viewBox="0 0 18 18">
                <circle class="spinner-track" cx="9" cy="9" r="7" />
                <circle class="spinner-arc" cx="9" cy="9" r="7" />
              </svg>
            </div>
            <div class="content-swap">
              <span class="label shimmer swap-item" :class="{ active: showTranscribingLabel }">{{ correcting ? 'Cleaning up' : 'Transcribing' }}</span>
              <div class="typing-dots swap-item" :class="{ active: !showTranscribingLabel }">
                <div class="t-dot" style="--d: 0s; --warmth: 0deg" />
                <div class="t-dot" style="--d: 0.2s; --warmth: -10deg" />
                <div class="t-dot" style="--d: 0.4s; --warmth: -20deg" />
              </div>
            </div>
          </div>
        </div>
      </Transition>
    </div>
    <Transition name="caption">
      <div v-if="recording && partialText" class="partial-caption">{{ partialText }}</div>
    </Transition>
    <Transition name="caption">
      <div v-if="showError" class="error-pill" role="alert">
        <span class="error-icon" aria-hidden="true">!</span>
        <span class="error-text">{{ errorShort }}</span>
      </div>
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

/* ---- Transition wrapper ---- */
/* transform / filter / opacity animate here, so they never
   touch the inner .pill that carries backdrop-filter.        */
.pill-outer {
  will-change: transform, opacity, filter;
  grid-area: 1 / 1; /* Both pills stack in the same cell for crossfade */
}

/* ---- Live partial-transcription caption ---- */
.partial-caption {
  max-width: 320px;
  padding: 4px 12px;
  border-radius: 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  color: var(--text-secondary, rgba(255, 255, 255, 0.85));
  font-size: 11px;
  font-weight: 400;
  line-height: 1.3;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  will-change: opacity, transform;
}

.caption-enter-from {
  opacity: 0;
  transform: translateY(-2px);
  filter: blur(2px);
}
.caption-enter-active {
  transition: opacity 0.25s ease-out,
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

/* ---- Error pill (auto-dismissing) ---- */
.error-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 320px;
  padding: 5px 12px;
  border-radius: 14px;
  background: var(--bg-elevated);
  border: 1px solid rgba(224, 85, 85, 0.45);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  color: var(--recording, #e05555);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.3;
  text-align: left;
  box-shadow: 0 0 12px rgba(224, 85, 85, 0.18);
  will-change: opacity, transform;
}
.error-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: rgba(224, 85, 85, 0.18);
  font-size: 10px;
  font-weight: 700;
  flex-shrink: 0;
}
.error-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ---- Pill base ---- */
.pill {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 6px 14px;
  border-radius: 18px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  font-size: 12px;
  font-weight: 500;
  white-space: nowrap;
}

/* ---- Enter / leave transition (on .pill-outer) ---- */
.pill-enter-from {
  opacity: 0;
  transform: scale(0.82) translateY(3px);
  filter: blur(4px);
}
.pill-enter-active {
  transition: opacity 0.35s cubic-bezier(0.34, 1.56, 0.64, 1),
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
  transition: opacity 0.2s ease-in,
              transform 0.2s ease-in,
              filter 0.2s ease-in;
}
.pill-leave-to {
  opacity: 0;
  transform: scale(0.92);
  filter: blur(2px);
}

/* ---- Recording state ---- */
.pill.recording {
  color: var(--recording);
  border-color: rgba(224, 85, 85, 0.3);
  animation: border-glow-red 2s ease-in-out infinite;
}

@keyframes border-glow-red {
  0%, 100% {
    box-shadow: 0 0 0 0 rgba(224, 85, 85, 0),
                0 0 6px rgba(224, 85, 85, 0.08);
  }
  50% {
    box-shadow: 0 0 0 2px rgba(224, 85, 85, 0.06),
                0 0 12px rgba(224, 85, 85, 0.15);
  }
}

/* ---- Dot with sonar rings ---- */
.dot-wrap {
  position: relative;
  width: 8px;
  height: 8px;
  flex-shrink: 0;
}

.pulse-dot {
  position: relative;
  z-index: 1;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--recording);
  animation: dot-pulse 1.4s ease-in-out infinite;
}

@keyframes dot-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.5; transform: scale(0.85); }
}

.sonar-ring {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 8px;
  height: 8px;
  margin: -4px 0 0 -4px;
  border-radius: 50%;
  border: 1.5px solid var(--recording);
  opacity: 0;
  animation: sonar 2.2s cubic-bezier(0.2, 0, 0.3, 1) infinite;
}

.sonar-ring.delay {
  animation-delay: 0.8s;
}

@keyframes sonar {
  0% {
    opacity: 0.6;
    transform: scale(1);
  }
  100% {
    opacity: 0;
    transform: scale(3.5);
  }
}

/* ---- Processing state ---- */
.pill.processing {
  color: var(--processing);
  border-color: rgba(245, 166, 35, 0.3);
  animation: border-glow-gold 2.4s ease-in-out infinite;
}

@keyframes border-glow-gold {
  0%, 100% {
    box-shadow: 0 0 0 0 rgba(245, 166, 35, 0),
                0 0 6px rgba(245, 166, 35, 0.06);
  }
  50% {
    box-shadow: 0 0 0 2px rgba(245, 166, 35, 0.05),
                0 0 10px rgba(245, 166, 35, 0.12);
  }
}

/* ---- SVG spinner ---- */
.spinner-wrap {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
}

.spinner {
  width: 14px;
  height: 14px;
  animation: spin 1s linear infinite;
}

.spinner-track {
  fill: none;
  stroke: var(--border);
  stroke-width: 1.5;
}

.spinner-arc {
  fill: none;
  stroke: var(--processing);
  stroke-width: 1.5;
  stroke-linecap: round;
  stroke-dasharray: 12 32;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ---- Label base ---- */
.label {
  position: relative;
}

/* ---- Content swap (label ↔ animation) ---- */
.content-swap {
  display: grid;
  align-items: center;
  min-height: 14px;
}

.swap-item {
  grid-area: 1 / 1;
  opacity: 0;
  transform: translateY(3px);
  transition: opacity 0.4s cubic-bezier(0.4, 0, 0.2, 1),
              transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.swap-item.active {
  opacity: 1;
  transform: translateY(0);
}

/* ---- Recording: audio wave bars ---- */
.wave-bars {
  display: flex;
  align-items: center;       /* center-anchored = proper waveform feel */
  justify-content: center;
  gap: 2.5px;
  height: 14px;
  padding: 0 8px;
}

.wave-bar {
  width: 2.5px;
  height: 2px;
  border-radius: 2px;
  background: linear-gradient(
    to top,
    var(--recording),
    color-mix(in oklch, var(--recording) 65%, #ffb8a8)
  );
  box-shadow: 0 0 6px rgba(224, 85, 85, 0.3),
              0 0 12px rgba(224, 85, 85, 0.06);
  transition: height 0.06s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: height;
  animation: bar-breathe 2.8s ease-in-out calc(var(--i, 0) * 0.18s) infinite;
}

@keyframes bar-breathe {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.6; }
}

/* ---- Transcribing: bouncing dots (cartoon physics) ---- */
.typing-dots {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  height: 14px;
  padding: 0 14px;
}

.t-dot {
  position: relative;
  width: 5.5px;
  height: 5.5px;
  border-radius: 50%;
  background: var(--processing);
  box-shadow: 0 0 6px rgba(245, 166, 35, 0.3),
              0 0 12px rgba(245, 166, 35, 0.08);
  filter: hue-rotate(var(--warmth, 0deg));
  animation: dot-hop 1.6s cubic-bezier(0.4, 0, 0.2, 1) var(--d, 0s) infinite;
  will-change: transform;
}

/* Ground shadow under each dot */
.t-dot::after {
  content: '';
  position: absolute;
  bottom: -3px;
  left: 50%;
  width: 6px;
  height: 2px;
  border-radius: 50%;
  background: rgba(245, 166, 35, 0.12);
  transform: translateX(-50%);
  animation: dot-shadow 1.6s cubic-bezier(0.4, 0, 0.2, 1) var(--d, 0s) infinite;
}

@keyframes dot-hop {
  0%, 82%, 100% {
    transform: translateY(4px) scaleY(1) scaleX(1);
    opacity: 0.3;
  }
  /* Charge-up squash — anticipation before the jump */
  10% {
    transform: translateY(5.5px) scaleY(0.5) scaleX(1.5);
    opacity: 0.55;
  }
  /* Launch! Stretch tall on the way up */
  24% {
    transform: translateY(-3px) scaleY(1.3) scaleX(0.7);
    opacity: 1;
  }
  /* Peak hang — brief float at the top */
  32% {
    transform: translateY(-4px) scaleY(1.0) scaleX(1.0);
    opacity: 1;
  }
  /* Falling — stretch vertically */
  44% {
    transform: translateY(2px) scaleY(1.2) scaleX(0.8);
    opacity: 0.85;
  }
  /* Landing squash — big, satisfying impact */
  52% {
    transform: translateY(5px) scaleY(0.45) scaleX(1.55);
    opacity: 0.7;
  }
  /* Mini bounce — secondary action */
  64% {
    transform: translateY(1px) scaleY(0.9) scaleX(1.1);
    opacity: 0.5;
  }
  /* Settle back */
  74% {
    transform: translateY(4px) scaleY(0.9) scaleX(1.1);
    opacity: 0.38;
  }
}

/* Shadow scales inversely to dot height — wider when dot is closer */
@keyframes dot-shadow {
  0%, 82%, 100% {
    transform: translateX(-50%) scaleX(1);
    opacity: 0.15;
  }
  10% {
    transform: translateX(-50%) scaleX(1.3);
    opacity: 0.25;
  }
  24% {
    transform: translateX(-50%) scaleX(0.4);
    opacity: 0.05;
  }
  32% {
    transform: translateX(-50%) scaleX(0.3);
    opacity: 0.03;
  }
  52% {
    transform: translateX(-50%) scaleX(1.6);
    opacity: 0.3;
  }
  64% {
    transform: translateX(-50%) scaleX(0.7);
    opacity: 0.1;
  }
}

/* ---- Shimmer sweep on transcribing text ---- */
.label.shimmer {
  -webkit-background-clip: text;
  background-clip: text;
  color: var(--processing);
  background-image: linear-gradient(
    100deg,
    var(--processing) 0%,
    var(--processing) 35%,
    rgba(255, 230, 180, 0.95) 50%,
    var(--processing) 65%,
    var(--processing) 100%
  );
  background-size: 250% 100%;
  -webkit-text-fill-color: transparent;
  animation: shimmer-sweep 2.8s ease-in-out infinite;
}

@keyframes shimmer-sweep {
  0%   { background-position: 100% 0; }
  100% { background-position: -50% 0; }
}

/* ---- Instant enter (processing → recording) ---- */
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
  transition: opacity 0.2s ease-in,
              transform 0.2s ease-in,
              filter 0.2s ease-in;
}
.pill-instant-leave-to {
  opacity: 0;
  transform: scale(0.92);
  filter: blur(2px);
}
</style>
