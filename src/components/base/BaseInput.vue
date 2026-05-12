<script setup lang="ts">
import { computed } from 'vue';

type Size = 'sm' | 'md';

const props = withDefaults(
  defineProps<{
    type?: 'text' | 'password' | 'email' | 'number' | 'search';
    placeholder?: string;
    size?: Size;
    disabled?: boolean;
    autofocus?: boolean;
  }>(),
  {
    type: 'text',
    placeholder: '',
    size: 'md',
  },
);

const model = defineModel<string>({ default: '' });

const sizeClasses = computed(() => {
  if (props.size === 'sm') return 'px-2 py-1 text-[11px]';
  return 'px-2.5 py-1.5 text-[12px]';
});
</script>

<template>
  <input
    v-model="model"
    :type="type"
    :placeholder="placeholder"
    :disabled="disabled"
    :autofocus="autofocus"
    class="min-w-0 rounded-md bg-raised border border-edge text-ink placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 focus:shadow-[0_0_0_3px_rgba(232,175,71,0.08)] transition-all duration-150 disabled:opacity-40 disabled:cursor-not-allowed"
    :class="sizeClasses"
  />
</template>
