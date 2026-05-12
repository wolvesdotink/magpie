<script setup lang="ts">
defineProps<{
  label: string;
}>();
</script>

<template>
  <section class="animate-float-up flex flex-col gap-1.5">
    <div class="flex items-center gap-1.5 mb-2">
      <div
        class="flex items-center justify-center w-5 h-5 rounded-[5px] bg-raised border border-edge text-ink-faint"
      >
        <slot name="icon" />
      </div>
      <span class="text-[10px] font-bold uppercase tracking-[0.08em] text-ink-faint">
        {{ label }}
      </span>
      <slot name="header-extra" />
    </div>
    <slot />
  </section>
</template>
