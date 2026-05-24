<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { UnlistenFn } from '@tauri-apps/api/event';
import {
  clearTranscriptionHistory,
  copyHistoryEntryToClipboard,
  getTranscriptionHistory,
  type HistoryEntry,
} from '@/lib/commands';
import { onHistoryEntryAdded } from '@/lib/events';
import { useSettings } from '@/composables/useSettings';
import BaseInput from '@/components/base/BaseInput.vue';

const appWindow = getCurrentWindow();
const entries = ref<HistoryEntry[]>([]);
const searchQuery = ref('');
const copiedId = ref<number | null>(null);
const unlisteners: UnlistenFn[] = [];

// History window is a separate webview from Settings, so its `useSettings`
// is a different singleton. `reload()` re-fetches so toggling the setting
// over there propagates here on the next HISTORY_ENTRY_ADDED event.
const { settings, reload: reloadSettings } = useSettings();
const historyDisabled = computed(
  () =>
    settings.value !== null &&
    (!settings.value.historyEnabled || (settings.value.historyMaxEntries ?? 0) === 0),
);

const filteredEntries = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return entries.value;
  return entries.value.filter((e) => e.text.toLowerCase().includes(q));
});

async function refresh() {
  try {
    // Reload settings alongside entries so a toggle flip in the Settings
    // window flows through here on the next HISTORY_ENTRY_ADDED event.
    const [, fetched] = await Promise.all([reloadSettings(), getTranscriptionHistory()]);
    entries.value = fetched;
  } catch (e) {
    console.error('Failed to load history:', e);
  }
}

async function copyEntry(entry: HistoryEntry) {
  try {
    await copyHistoryEntryToClipboard(entry.text);
    copiedId.value = entry.id;
    // Brief visual confirmation, then hide so the user can paste into
    // whatever app they actually want.
    setTimeout(() => {
      copiedId.value = null;
      void appWindow.hide();
    }, 400);
  } catch (e) {
    console.error('Failed to copy entry:', e);
  }
}

async function clearAll() {
  // Native confirm() matches the rest of the app — no modal infra in tree.
  if (!confirm('Clear all transcription history? This cannot be undone.')) return;
  try {
    await clearTranscriptionHistory();
    await refresh();
  } catch (e) {
    console.error('Failed to clear history:', e);
  }
}

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

function relativeTime(epochMs: number): string {
  const diffSec = (epochMs - Date.now()) / 1000;
  const absSec = Math.abs(diffSec);
  if (absSec < 60) return rtf.format(Math.round(diffSec), 'second');
  if (absSec < 3600) return rtf.format(Math.round(diffSec / 60), 'minute');
  if (absSec < 86400) return rtf.format(Math.round(diffSec / 3600), 'hour');
  return rtf.format(Math.round(diffSec / 86400), 'day');
}

onMounted(async () => {
  await refresh();
  unlisteners.push(await onHistoryEntryAdded(() => refresh()));
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
});
</script>

<template>
  <div class="flex flex-col flex-1 min-h-0">
    <div v-if="!historyDisabled" class="px-5 pb-3">
      <BaseInput v-model="searchQuery" type="search" placeholder="Search transcripts..." />
    </div>

    <div class="flex-1 overflow-y-auto min-h-0 px-5">
      <div v-if="historyDisabled" class="text-center text-ink-faint text-[11px] py-12 px-4">
        <p>History is disabled.</p>
        <p class="mt-1">Enable it in Settings → General to start saving transcripts.</p>
      </div>
      <div v-else-if="entries.length === 0" class="text-center text-ink-faint text-[11px] py-12">
        No dictations yet — start by holding Fn to record.
      </div>
      <div
        v-else-if="filteredEntries.length === 0"
        class="text-center text-ink-faint text-[11px] py-12"
      >
        No matches for &ldquo;{{ searchQuery }}&rdquo;.
      </div>
      <ul v-else class="flex flex-col gap-2 pb-4">
        <li v-for="entry in filteredEntries" :key="entry.id">
          <button
            class="w-full text-left rounded-lg border border-edge bg-raised hover:bg-hover hover:border-edge-strong transition-all duration-150 p-3 group"
            @click="copyEntry(entry)"
          >
            <p class="text-[12px] text-ink whitespace-pre-wrap line-clamp-3 leading-snug">
              {{ entry.text }}
            </p>
            <div class="flex items-center justify-between mt-2 text-[10px] text-ink-faint">
              <span>{{ relativeTime(entry.createdAt) }}</span>
              <span
                class="font-semibold transition-opacity"
                :class="
                  copiedId === entry.id
                    ? 'opacity-100 text-gold'
                    : 'opacity-0 group-hover:opacity-100 text-ink-muted'
                "
              >
                {{ copiedId === entry.id ? 'Copied!' : 'Click to copy' }}
              </span>
            </div>
          </button>
        </li>
      </ul>
    </div>

    <div
      v-if="!historyDisabled && entries.length > 0"
      class="px-5 py-3 border-t border-edge bg-raised/40 flex items-center justify-between"
    >
      <span class="text-[10px] text-ink-faint">
        {{ entries.length }} {{ entries.length === 1 ? 'entry' : 'entries' }}
      </span>
      <button
        class="text-[11px] text-flame hover:text-flame-hover hover:underline transition-colors"
        @click="clearAll"
      >
        Clear history
      </button>
    </div>
  </div>
</template>
