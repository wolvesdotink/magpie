<script setup lang="ts">
import { computed } from 'vue';

type Variant = 'primary' | 'secondary' | 'ghost' | 'icon' | 'danger' | 'link';
type Size = 'sm' | 'md' | 'lg';

const props = withDefaults(
  defineProps<{
    variant?: Variant;
    size?: Size;
    type?: 'button' | 'submit' | 'reset';
    disabled?: boolean;
    fullWidth?: boolean;
    active?: boolean;
  }>(),
  {
    variant: 'secondary',
    size: 'md',
    type: 'button',
    disabled: false,
    fullWidth: false,
    active: false,
  },
);

const sizeClasses = computed(() => {
  if (props.variant === 'icon') {
    if (props.size === 'sm') return 'w-5 h-5';
    if (props.size === 'lg') return 'w-8 h-8';
    return 'w-6 h-6';
  }
  if (props.variant === 'link') {
    return ''; // link has no padding
  }
  if (props.size === 'sm') return 'px-2 py-1 text-[10px]';
  if (props.size === 'lg') return 'px-4 py-2.5 text-[13px]';
  return 'px-3 py-1.5 text-[12px]';
});

const variantClasses = computed(() => {
  switch (props.variant) {
    case 'primary':
      return 'bg-gradient-to-b from-gold to-gold-hover text-gold-ink font-semibold shadow-press hover:from-gold-hover hover:to-gold-deep hover:shadow-lifted active:scale-[0.97]';
    case 'secondary':
      return 'bg-gradient-to-b from-raised to-hover border border-edge-strong text-ink font-semibold shadow-press hover:shadow-lifted hover:from-hover hover:to-subtle active:scale-[0.97]';
    case 'ghost':
      return `rounded-md font-medium ${props.active ? 'bg-raised text-ink' : 'text-ink-muted hover:bg-raised hover:text-ink'}`;
    case 'icon':
      return 'flex items-center justify-center rounded-md text-ink-faint hover:text-ink-muted hover:bg-raised active:scale-90';
    case 'danger':
      return 'bg-gradient-to-b from-flame to-flame-hover text-white font-semibold shadow-press hover:shadow-glow-flame active:scale-[0.97]';
    case 'link':
      return 'text-ink-faint hover:text-ink hover:underline';
    default:
      return '';
  }
});

const baseClasses = computed(() => {
  const arr = [
    'inline-flex items-center justify-center gap-1.5 transition-all duration-150 disabled:opacity-40 disabled:cursor-not-allowed disabled:active:scale-100',
  ];
  if (props.variant !== 'icon' && props.variant !== 'link') {
    arr.push('rounded-lg');
  }
  if (props.fullWidth) arr.push('w-full');
  return arr.join(' ');
});
</script>

<template>
  <button :type="type" :disabled="disabled" :class="[baseClasses, variantClasses, sizeClasses]">
    <slot />
  </button>
</template>
