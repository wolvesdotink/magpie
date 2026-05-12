<script setup lang="ts">
defineProps<{
  text: string | null;
  showEmpty: boolean;
}>();
</script>

<template>
  <div v-if="text" class="flex-1 flex flex-col gap-1.5 min-h-0">
    <span class="text-[10px] uppercase tracking-[0.08em] font-semibold text-ink-faint">
      Last transcription
    </span>
    <div
      class="flex-1 p-3.5 text-[13px] leading-relaxed text-ink-muted bg-panel rounded-lg border border-edge shadow-well overflow-y-auto"
    >
      {{ text }}
    </div>
  </div>

  <div
    v-else-if="showEmpty"
    class="flex-1 flex flex-col items-center justify-center gap-2.5 bg-panel/50 rounded-lg border border-dashed border-edge/60 shadow-well"
  >
    <svg
      width="28"
      height="28"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="1.2"
      class="text-ink-faint/40"
    >
      <path
        d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"
        stroke-linecap="round"
      />
      <path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5" stroke-linecap="round" />
      <line x1="12" y1="19" x2="12" y2="22" stroke-linecap="round" />
      <line x1="9" y1="22" x2="15" y2="22" stroke-linecap="round" />
    </svg>
    <span class="text-[11px] text-ink-faint/50 font-medium">Transcriptions appear here</span>
  </div>
</template>
