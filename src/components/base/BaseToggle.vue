<script setup lang="ts">
defineProps<{
  disabled?: boolean;
}>();

const model = defineModel<boolean>({ required: true });
</script>

<template>
  <button
    type="button"
    role="switch"
    :aria-checked="model"
    :disabled="disabled"
    class="relative w-8 h-[18px] rounded-[9px] flex-shrink-0 transition-all duration-[250ms] cursor-pointer ease-[cubic-bezier(0.4,0,0.2,1)] disabled:cursor-not-allowed disabled:opacity-40"
    :class="model ? 'bg-gold shadow-[0_0_8px_rgba(232,175,71,0.2)]' : 'bg-edge-strong'"
    @click="model = !model"
  >
    <span
      class="absolute top-[2px] w-[14px] h-[14px] rounded-full bg-white shadow-[0_1px_3px_rgba(0,0,0,0.3)] transition-all duration-[250ms] ease-[cubic-bezier(0.4,0,0.2,1)]"
      :class="model ? 'left-[16px]' : 'left-[2px]'"
    />
  </button>
</template>
