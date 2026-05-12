<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    size?: number;
    tone?: 'gold' | 'flame' | 'ink';
  }>(),
  {
    size: 14,
    tone: 'gold',
  },
);

const ringColor = computed(() => {
  switch (props.tone) {
    case 'flame':
      return 'border-t-flame';
    case 'ink':
      return 'border-t-ink';
    case 'gold':
    default:
      return 'border-t-gold';
  }
});
</script>

<template>
  <div
    class="rounded-full border-2 border-edge animate-spin flex-shrink-0"
    :class="ringColor"
    :style="{ width: `${size}px`, height: `${size}px` }"
    role="status"
    aria-label="Loading"
  />
</template>
