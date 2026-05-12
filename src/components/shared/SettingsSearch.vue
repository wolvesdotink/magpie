<script setup lang="ts" generic="T extends string">
import { computed } from 'vue';

interface SearchEntry {
  section: T;
  label: string;
  keywords: string;
}

const props = defineProps<{
  index: SearchEntry[];
  sectionLabel: (id: T) => string;
}>();

const query = defineModel<string>('query', { default: '' });

const emit = defineEmits<{
  jump: [section: T];
}>();

const results = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return [];
  return props.index.filter(
    (i) => i.label.toLowerCase().includes(q) || i.keywords.toLowerCase().includes(q),
  );
});

function onJump(section: T) {
  emit('jump', section);
  query.value = '';
}
</script>

<template>
  <div class="relative px-4 pt-3 pb-2.5 border-b border-edge">
    <div class="relative flex items-center">
      <svg
        class="absolute left-[9px] text-ink-faint pointer-events-none"
        width="11"
        height="11"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="11" cy="11" r="8" />
        <line x1="21" y1="21" x2="16.65" y2="16.65" />
      </svg>
      <input
        v-model="query"
        type="text"
        placeholder="Search settings…"
        class="w-full px-[26px] py-1.5 text-[12px] rounded-md bg-raised border border-edge text-ink placeholder:text-ink-faint focus:outline-none focus:border-gold/40 focus:shadow-[0_0_0_3px_rgba(232,175,71,0.08)] transition-all duration-150"
      />
      <button
        v-if="query"
        type="button"
        aria-label="Clear search"
        class="absolute right-[6px] flex items-center justify-center w-4 h-4 rounded-full bg-edge-strong text-canvas transition-colors duration-150 hover:bg-ink-faint"
        @click="query = ''"
      >
        <svg
          width="9"
          height="9"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="3"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <line x1="18" y1="6" x2="6" y2="18" />
          <line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>

    <div
      v-if="query.trim() && results.length > 0"
      class="absolute top-[calc(100%-4px)] left-4 right-4 z-[60] bg-panel border border-edge rounded-lg shadow-elevated overflow-hidden max-h-[260px] overflow-y-auto p-1"
    >
      <button
        v-for="r in results"
        :key="r.section + ':' + r.label"
        class="flex items-center justify-between w-full px-2.5 py-1.5 rounded-[5px] text-left transition-colors duration-100 hover:bg-raised"
        @click="onJump(r.section)"
      >
        <span class="text-[12px] font-medium text-ink">{{ r.label }}</span>
        <span class="text-[9px] font-bold uppercase tracking-[0.08em] text-ink-faint">
          {{ sectionLabel(r.section) }}
        </span>
      </button>
    </div>
    <div
      v-else-if="query.trim()"
      class="absolute top-[calc(100%-4px)] left-4 right-4 z-[60] bg-panel border border-edge rounded-lg shadow-elevated px-3 py-2.5 text-[11px] text-ink-faint"
    >
      No matches.
    </div>
  </div>
</template>
