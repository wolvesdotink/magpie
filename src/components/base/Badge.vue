<script setup lang="ts">
import { computed } from 'vue';

type Tone = 'neutral' | 'gold' | 'flame' | 'leaf';
type Size = 'xs' | 'sm';

const props = withDefaults(
  defineProps<{
    tone?: Tone;
    size?: Size;
  }>(),
  {
    tone: 'neutral',
    size: 'xs',
  },
);

const sizeClasses = computed(() =>
  props.size === 'sm' ? 'px-1.5 py-0.5 text-[9px]' : 'px-1 py-px text-[8px]',
);

const toneClasses = computed(() => {
  switch (props.tone) {
    case 'gold':
      return 'bg-gold/[0.08] border-gold/20 text-gold';
    case 'flame':
      return 'bg-flame/10 border-flame/30 text-flame';
    case 'leaf':
      return 'bg-leaf/10 border-leaf/30 text-leaf';
    case 'neutral':
    default:
      return 'bg-raised border-edge text-ink-faint';
  }
});
</script>

<template>
  <span
    class="inline-flex items-center rounded border font-bold uppercase tracking-[0.06em]"
    :class="[sizeClasses, toneClasses]"
  >
    <slot />
  </span>
</template>
