<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue';

const props = defineProps<{
  /** True while the LLM correction pass is running (vs. the raw whisper
   *  decode). Just swaps the label text — visuals are identical. */
  correcting: boolean;
  /** Increments each time we transition INTO transcribing so the label-swap
   *  timer resets and the user sees "Transcribing" briefly before the dots
   *  take over. The parent owns the value; we react to changes. */
  generation: number;
}>();

// ── Label → typing-dots swap ──
const showLabel = ref(true);
let timer: ReturnType<typeof setTimeout> | null = null;

watch(
  () => props.generation,
  () => {
    if (timer) clearTimeout(timer);
    showLabel.value = true;
    timer = setTimeout(() => {
      showLabel.value = false;
    }, 1400);
  },
  { immediate: true },
);

onUnmounted(() => {
  if (timer) clearTimeout(timer);
});
</script>

<template>
  <div class="pill processing">
    <div class="spinner-wrap">
      <svg class="spinner" viewBox="0 0 18 18">
        <circle class="spinner-track" cx="9" cy="9" r="7" />
        <circle class="spinner-arc" cx="9" cy="9" r="7" />
      </svg>
    </div>
    <div class="content-swap">
      <span class="label shimmer swap-item" :class="{ active: showLabel }">
        {{ correcting ? 'Cleaning up' : 'Transcribing' }}
      </span>
      <div class="typing-dots swap-item" :class="{ active: !showLabel }">
        <div class="t-dot" style="--d: 0s; --warmth: 0deg" />
        <div class="t-dot" style="--d: 0.2s; --warmth: -10deg" />
        <div class="t-dot" style="--d: 0.4s; --warmth: -20deg" />
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

.pill.processing {
  color: var(--processing);
  border-color: rgba(245, 166, 35, 0.3);
  animation: border-glow-gold 2.4s ease-in-out infinite;
}

@keyframes border-glow-gold {
  0%,
  100% {
    box-shadow:
      0 0 0 0 rgba(245, 166, 35, 0),
      0 0 6px rgba(245, 166, 35, 0.06);
  }
  50% {
    box-shadow:
      0 0 0 2px rgba(245, 166, 35, 0.05),
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
  transform-origin: 50% 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
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

/* ---- Label / dots swap ---- */
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

/* ---- Bouncing dots (cartoon physics) ---- */
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
  box-shadow:
    0 0 6px rgba(245, 166, 35, 0.3),
    0 0 12px rgba(245, 166, 35, 0.08);
  filter: hue-rotate(var(--warmth, 0deg));
  animation: dot-hop 1.6s cubic-bezier(0.4, 0, 0.2, 1) var(--d, 0s) infinite;
  will-change: transform;
}

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
  0%,
  82%,
  100% {
    transform: translateY(4px) scaleY(1) scaleX(1);
    opacity: 0.3;
  }
  10% {
    transform: translateY(5.5px) scaleY(0.5) scaleX(1.5);
    opacity: 0.55;
  }
  24% {
    transform: translateY(-3px) scaleY(1.3) scaleX(0.7);
    opacity: 1;
  }
  32% {
    transform: translateY(-4px) scaleY(1) scaleX(1);
    opacity: 1;
  }
  44% {
    transform: translateY(2px) scaleY(1.2) scaleX(0.8);
    opacity: 0.85;
  }
  52% {
    transform: translateY(5px) scaleY(0.45) scaleX(1.55);
    opacity: 0.7;
  }
  64% {
    transform: translateY(1px) scaleY(0.9) scaleX(1.1);
    opacity: 0.5;
  }
  74% {
    transform: translateY(4px) scaleY(0.9) scaleX(1.1);
    opacity: 0.38;
  }
}

@keyframes dot-shadow {
  0%,
  82%,
  100% {
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
  0% {
    background-position: 100% 0;
  }
  100% {
    background-position: -50% 0;
  }
}
</style>
