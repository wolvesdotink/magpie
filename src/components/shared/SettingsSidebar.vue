<script setup lang="ts" generic="T extends string">
interface NavItem {
  id: T;
  label: string;
  icon: string;
}

defineProps<{
  items: NavItem[];
}>();

const model = defineModel<T>({ required: true });
</script>

<!-- eslint-disable vue/no-v-html — trusted in-source SVG path strings -->
<template>
  <nav
    class="flex-shrink-0 w-[138px] px-2 py-2.5 border-r border-edge flex flex-col gap-px overflow-y-auto"
  >
    <button
      v-for="item in items"
      :key="item.id"
      class="flex items-center gap-2 px-2 py-1.5 rounded-md text-left text-[12px] cursor-pointer transition-all duration-150"
      :class="
        model === item.id
          ? 'bg-gold/[0.08] text-ink font-semibold'
          : 'text-ink-muted font-medium hover:bg-raised hover:text-ink'
      "
      @click="model = item.id"
    >
      <svg
        width="13"
        height="13"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="flex-shrink-0 transition-colors duration-150"
        :class="model === item.id ? 'text-gold' : 'text-ink-faint'"
        v-html="item.icon"
      />
      <span class="flex-1 min-w-0 truncate">{{ item.label }}</span>
    </button>
  </nav>
</template>
