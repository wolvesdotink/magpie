<script setup lang="ts">
defineProps<{
  label: string;
  description?: string;
  granted: boolean;
}>();
</script>

<template>
  <div
    class="w-full p-3.5 rounded-lg border transition-colors duration-200"
    :class="granted ? 'bg-leaf/[0.06] border-leaf/25' : 'bg-panel border-edge'"
  >
    <div class="flex items-center justify-between gap-3" :class="{ 'mb-2': description }">
      <div class="flex items-center gap-2.5">
        <div
          class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
          :class="granted ? 'bg-leaf/15 text-leaf' : 'bg-raised text-ink-faint'"
        >
          <slot name="icon" />
        </div>
        <div>
          <span class="text-[12px] font-semibold text-ink">{{ label }}</span>
          <p v-if="description" class="text-[9px] text-ink-faint leading-snug mt-0.5">
            {{ description }}
          </p>
        </div>
      </div>
      <span
        v-if="granted"
        class="flex items-center gap-1 text-[9px] font-bold text-leaf uppercase tracking-wider flex-shrink-0"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="3"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="20 6 9 17 4 12" />
        </svg>
        Granted
      </span>
    </div>
    <div v-if="!granted" class="mt-2">
      <slot name="actions" />
    </div>
  </div>
</template>
