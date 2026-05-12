<script setup lang="ts">
import { ref, computed, watch, watchEffect, onUnmounted } from 'vue';

const props = defineProps<{
  /** Normalized 0..1 RMS amplitude driving the wave-bar heights. */
  amplitude: number;
  /** Increments each time recording restarts. Drives the label-swap timer
   *  reset (the parent owns the value; we just react to changes). */
  generation: number;
}>();

// ── Label → wave-bars swap ──
// Recording starts with the "Recording" word visible, then crossfades to the
// wave-bar visualizer after 1.4 s so the user gets one clear acknowledgment
// before the animation takes over.
const showLabel = ref(true);
let recTimer: ReturnType<typeof setTimeout> | null = null;

watch(
  () => props.generation,
  () => {
    if (recTimer) clearTimeout(recTimer);
    showLabel.value = true;
    recTimer = setTimeout(() => {
      showLabel.value = false;
    }, 1400);
  },
  { immediate: true },
);

// ── Voice-driven wave bars ──
const smoothAmplitude = ref(0);
let animationFrame: number | null = null;

// 9 bars — asymmetric bell-curve for organic, natural feel
const barWeights = [0.3, 0.55, 0.78, 0.92, 1.0, 0.88, 0.68, 0.45, 0.25];
const MIN_SCALE = 0.1;
const MAX_HEIGHT = 24;

// Per-bar sine noise gives each bar its own "personality"
const barNoises = ref<number[]>(new Array(barWeights.length).fill(0));
let elapsed = 0;

watchEffect((onCleanup) => {
  const tick = () => {
    // Lerp toward target amplitude — 0.15 = smooth but responsive
    smoothAmplitude.value += (props.amplitude - smoothAmplitude.value) * 0.15;

    // Per-bar organic noise — overlapping sine waves at different frequencies
    // so each bar dances slightly differently, like a real visualizer.
    elapsed += 0.018;
    barNoises.value = barWeights.map(
      (_, i) =>
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
});

const barHeights = computed(() => {
  const amp = smoothAmplitude.value;
  return barWeights.map((w, i) => {
    const noise = barNoises.value[i] || 0;
    // Noise contributes more when amplitude is present — bars feel alive
    // when speaking.
    const noiseContrib = noise * (0.04 + amp * 0.18);
    const scale = Math.min(
      1,
      Math.max(MIN_SCALE, MIN_SCALE + amp * (1 - MIN_SCALE) + noiseContrib),
    );
    return Math.max(2, Math.round(w * scale * MAX_HEIGHT));
  });
});

onUnmounted(() => {
  if (recTimer) clearTimeout(recTimer);
});
</script>

<template>
  <div class="pill recording">
    <div class="dot-wrap">
      <div class="pulse-dot" />
      <div class="sonar-ring" />
      <div class="sonar-ring delay" />
    </div>
    <div class="content-swap">
      <span class="label swap-item" :class="{ active: showLabel }">Recording</span>
      <div class="wave-bars swap-item" :class="{ active: !showLabel }">
        <div
          v-for="(h, i) in barHeights"
          :key="i"
          class="wave-bar"
          :style="{ height: h + 'px', '--i': i }"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
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

.pill.recording {
  color: var(--recording);
  border-color: rgba(224, 85, 85, 0.3);
  animation: border-glow-red 2s ease-in-out infinite;
}

@keyframes border-glow-red {
  0%,
  100% {
    box-shadow:
      0 0 0 0 rgba(224, 85, 85, 0),
      0 0 6px rgba(224, 85, 85, 0.08);
  }
  50% {
    box-shadow:
      0 0 0 2px rgba(224, 85, 85, 0.06),
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
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.5;
    transform: scale(0.85);
  }
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

/* ---- Label / wave-bars swap ---- */
.label {
  position: relative;
}

.content-swap {
  display: grid;
  align-items: center;
  min-height: 14px;
}

.swap-item {
  grid-area: 1 / 1;
  opacity: 0;
  transform: translateY(3px);
  transition:
    opacity 0.4s cubic-bezier(0.4, 0, 0.2, 1),
    transform 0.4s cubic-bezier(0.4, 0, 0.2, 1);
}

.swap-item.active {
  opacity: 1;
  transform: translateY(0);
}

.wave-bars {
  display: flex;
  align-items: center;
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
  box-shadow:
    0 0 6px rgba(224, 85, 85, 0.3),
    0 0 12px rgba(224, 85, 85, 0.06);
  transition: height 0.06s cubic-bezier(0.22, 1, 0.36, 1);
  will-change: height;
  animation: bar-breathe 2.8s ease-in-out calc(var(--i, 0) * 0.18s) infinite;
}

@keyframes bar-breathe {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.6;
  }
}
</style>
