<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';

withDefaults(
  defineProps<{
    /** Place menu above the trigger instead of below. */
    placement?: 'bottom' | 'top';
    /** Width of the menu — defaults to matching the trigger. */
    menuClass?: string;
    /** Disable opening (e.g. when only one option is available). */
    disabled?: boolean;
  }>(),
  {
    placement: 'bottom',
    menuClass: '',
  },
);

const open = defineModel<boolean>('open', { default: false });
const rootRef = ref<HTMLElement | null>(null);

function close() {
  open.value = false;
}

function onClickOutside(e: MouseEvent) {
  if (rootRef.value && !rootRef.value.contains(e.target as Node)) {
    open.value = false;
  }
}

onMounted(() => document.addEventListener('mousedown', onClickOutside));
onUnmounted(() => document.removeEventListener('mousedown', onClickOutside));
</script>

<template>
  <div ref="rootRef" class="relative">
    <slot name="trigger" :open="open" :toggle="() => (open = !open)" />

    <Transition
      :enter-active-class="`transition duration-150 ease-out`"
      :enter-from-class="
        placement === 'top'
          ? 'opacity-0 translate-y-1 scale-[0.98]'
          : 'opacity-0 -translate-y-1 scale-[0.98]'
      "
      enter-to-class="opacity-100 translate-y-0 scale-100"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100 translate-y-0 scale-100"
      :leave-to-class="
        placement === 'top'
          ? 'opacity-0 translate-y-1 scale-[0.98]'
          : 'opacity-0 -translate-y-1 scale-[0.98]'
      "
    >
      <div
        v-if="open"
        class="absolute z-50 bg-panel border border-edge rounded-lg shadow-elevated overflow-hidden"
        :class="[
          placement === 'top' ? 'bottom-full mb-1.5' : 'top-full mt-1.5',
          menuClass || 'left-0 right-0',
        ]"
      >
        <slot name="menu" :close="close" />
      </div>
    </Transition>
  </div>
</template>
