<script setup lang="ts" generic="T extends string | number">
interface Option {
  value: T;
  label: string;
  desc?: string;
}

defineProps<{
  options: Option[];
}>();

const model = defineModel<T>({ required: true });
</script>

<template>
  <div class="flex flex-col gap-1.5">
    <button
      v-for="option in options"
      :key="option.value"
      type="button"
      class="flex items-center gap-2.5 p-2.5 rounded-lg border text-left transition-all duration-150"
      :class="
        model === option.value
          ? 'bg-gold/[0.04] border-gold/20'
          : 'bg-panel border-edge hover:border-edge-strong hover:bg-raised'
      "
      @click="model = option.value"
    >
      <div
        class="w-3.5 h-3.5 rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 transition-all"
        :class="model === option.value ? 'border-gold bg-gold/10' : 'border-edge-strong'"
      >
        <div v-if="model === option.value" class="w-1.5 h-1.5 rounded-full bg-gold" />
      </div>
      <div class="flex flex-col min-w-0">
        <span
          class="text-[12px] font-semibold"
          :class="model === option.value ? 'text-ink' : 'text-ink-muted'"
        >
          {{ option.label }}
        </span>
        <span v-if="option.desc" class="text-[10px] text-ink-faint leading-snug">
          {{ option.desc }}
        </span>
      </div>
    </button>
  </div>
</template>
