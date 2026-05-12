<script setup lang="ts">
defineProps<{
  recording: boolean;
  processing: boolean;
  hasModel: boolean;
}>();
</script>

<template>
  <div
    class="relative p-3.5 rounded-lg border transition-all duration-300"
    :class="{
      'bg-flame/[0.06] border-flame/30 shadow-glow-flame': recording,
      'bg-panel border-edge shadow-soft': !recording,
    }"
  >
    <div v-if="recording" class="flex items-center gap-2.5">
      <div class="relative flex-shrink-0 w-2.5 h-2.5">
        <div class="absolute inset-0 rounded-full bg-flame" />
        <div class="absolute inset-0 rounded-full bg-flame animate-ping" />
      </div>
      <span class="text-[13px] font-semibold text-flame">Recording… release Fn to stop</span>
    </div>

    <div v-else-if="processing" class="flex items-center gap-2.5">
      <div
        class="w-3.5 h-3.5 rounded-full border-2 border-edge border-t-gold animate-spin flex-shrink-0"
      />
      <span class="text-[13px] font-semibold text-gold">Transcribing…</span>
    </div>

    <div v-else-if="!hasModel" class="flex items-center gap-2.5">
      <div class="w-1.5 h-1.5 rounded-full bg-ink-faint flex-shrink-0" />
      <span class="text-[13px] text-ink-faint">No model loaded</span>
    </div>

    <div v-else class="flex items-center gap-2.5">
      <div class="w-1.5 h-1.5 rounded-full bg-leaf flex-shrink-0" />
      <span class="text-[13px] text-ink-muted">
        Ready — hold
        <kbd
          class="inline-flex items-center px-1.5 py-0.5 mx-0.5 text-[10px] font-mono font-semibold leading-none bg-raised rounded border border-edge shadow-soft text-ink-faint"
        >
          Fn
        </kbd>
        to dictate
      </span>
    </div>
  </div>
</template>
