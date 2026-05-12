<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(
  defineProps<{
    value: number;
    max?: number;
    /** When true, treat value as "rank-from-best" — invert so dot 1 fills first. */
    reverse?: boolean;
    /** Dot size class. */
    size?: 'xs' | 'sm';
    /** Active dot intensity. */
    intensity?: 'normal' | 'soft';
  }>(),
  {
    max: 5,
    reverse: false,
    size: 'xs',
    intensity: 'normal',
  },
);

const filledThreshold = computed(() => (props.reverse ? props.max + 1 - props.value : props.value));

const dotSize = computed(() => (props.size === 'sm' ? 'w-[4px] h-[4px]' : 'w-[3px] h-[3px]'));

const activeClass = computed(() => (props.intensity === 'soft' ? 'bg-gold/70' : 'bg-gold'));

const gap = computed(() => (props.size === 'sm' ? 'gap-[3px]' : 'gap-[2px]'));
</script>

<template>
  <div class="flex" :class="gap">
    <span
      v-for="i in max"
      :key="i"
      class="rounded-full"
      :class="[dotSize, i <= filledThreshold ? activeClass : 'bg-edge']"
    />
  </div>
</template>
