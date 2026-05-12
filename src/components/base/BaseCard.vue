<script setup lang="ts">
import { computed } from 'vue';

type Tone = 'neutral' | 'gold' | 'flame' | 'leaf' | 'dashed';
type Padding = 'none' | 'sm' | 'md' | 'lg';

const props = withDefaults(
  defineProps<{
    tone?: Tone;
    padding?: Padding;
    interactive?: boolean;
    glow?: boolean;
  }>(),
  {
    tone: 'neutral',
    padding: 'md',
    interactive: false,
    glow: false,
  },
);

const paddingClass = computed(() => {
  if (props.padding === 'none') return '';
  if (props.padding === 'sm') return 'p-2';
  if (props.padding === 'lg') return 'p-3.5';
  return 'p-2.5';
});

const toneClass = computed(() => {
  switch (props.tone) {
    case 'gold':
      return `bg-gold/[0.06] border border-gold/25${props.glow ? ' shadow-glow-gold' : ''}`;
    case 'flame':
      return `bg-flame/[0.06] border border-flame/30${props.glow ? ' shadow-glow-flame' : ''}`;
    case 'leaf':
      return 'bg-leaf/[0.06] border border-leaf/25';
    case 'dashed':
      return 'bg-panel/50 border border-dashed border-edge/60 shadow-well';
    case 'neutral':
    default:
      return 'bg-panel border border-edge shadow-soft';
  }
});

const interactiveClass = computed(() =>
  props.interactive
    ? 'transition-all duration-150 hover:border-edge-strong hover:bg-raised cursor-pointer'
    : '',
);
</script>

<template>
  <div class="rounded-lg" :class="[paddingClass, toneClass, interactiveClass]">
    <slot />
  </div>
</template>
