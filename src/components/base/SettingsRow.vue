<script setup lang="ts">
import BaseCard from './BaseCard.vue';

withDefaults(
  defineProps<{
    label: string;
    helper?: string;
    tone?: 'neutral' | 'gold' | 'flame' | 'leaf';
  }>(),
  {
    helper: '',
    tone: 'neutral',
  },
);
</script>

<template>
  <BaseCard :tone="tone">
    <div class="flex items-center justify-between gap-3">
      <div class="flex flex-col min-w-0 flex-1">
        <span class="text-[12px] font-semibold text-ink">{{ label }}</span>
        <span v-if="helper" class="text-[10px] text-ink-faint leading-snug mt-0.5">
          {{ helper }}
        </span>
      </div>
      <div class="flex-shrink-0">
        <slot />
      </div>
    </div>
  </BaseCard>
</template>
