<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    /** Value in 0–100 (percent) or 0–1 (ratio). Auto-detected by magnitude. */
    value: number;
    height?: 'thin' | 'normal';
  }>(),
  {
    height: 'thin',
  },
);

const percent = computed(() => {
  const v = props.value;
  const pct = v > 1 ? v : v * 100;
  return Math.max(0, Math.min(100, pct));
});

const heightClass = computed(() => (props.height === 'normal' ? 'h-1.5' : 'h-1'));
</script>

<template>
  <div
    class="bg-raised shadow-well rounded-full overflow-hidden"
    :class="heightClass"
    role="progressbar"
    :aria-valuenow="percent"
    aria-valuemin="0"
    aria-valuemax="100"
  >
    <div
      class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full transition-[width] duration-300 ease-out"
      :style="{ width: `${percent}%` }"
    />
  </div>
</template>
