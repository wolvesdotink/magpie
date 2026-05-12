<script setup lang="ts">
import { ref, computed } from 'vue';
import UpdatesSection from '@/components/UpdatesSection.vue';
import ActivationSection from '@/components/settings/ActivationSection.vue';
import GeneralSection from '@/components/settings/GeneralSection.vue';
import LanguageSection from '@/components/settings/LanguageSection.vue';
import ModelSection from '@/components/settings/ModelSection.vue';
import TranscriptionSection from '@/components/settings/TranscriptionSection.vue';
import VocabularySection from '@/components/settings/VocabularySection.vue';

withDefaults(
  defineProps<{
    standalone?: boolean;
  }>(),
  { standalone: false },
);

const emit = defineEmits<{
  back: [];
  modelChanged: [];
}>();

// ── Sidebar navigation ────────────────────────────────────────────────

type SectionId =
  | 'model'
  | 'language'
  | 'activation'
  | 'transcription'
  | 'vocabulary'
  | 'general'
  | 'updates';

const SECTIONS: { id: SectionId; label: string; icon: string }[] = [
  {
    id: 'model',
    label: 'Model',
    icon: '<path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"/><polyline points="3.27 6.96 12 12.01 20.73 6.96"/><line x1="12" y1="22.08" x2="12" y2="12"/>',
  },
  {
    id: 'language',
    label: 'Language',
    icon: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20"/><path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"/>',
  },
  {
    id: 'activation',
    label: 'Activation',
    icon: '<rect x="2" y="4" width="20" height="16" rx="2"/><path d="M6 8h.001M10 8h.001M14 8h.001M18 8h.001M8 12h.001M12 12h.001M16 12h.001M8 16h8"/>',
  },
  {
    id: 'transcription',
    label: 'Transcription',
    icon: '<path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"/>',
  },
  {
    id: 'vocabulary',
    label: 'Vocabulary',
    icon: '<path d="M4 19.5A2.5 2.5 0 016.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z"/>',
  },
  {
    id: 'general',
    label: 'General',
    icon: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>',
  },
  {
    id: 'updates',
    label: 'Updates',
    icon: '<polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>',
  },
];

const activeSectionId = ref<SectionId>('model');

const searchQuery = ref('');

// Flat index of searchable settings. Synonyms in `keywords` capture how a
// user might describe a feature (e.g. "hotkey" → activation), so the search
// works even when the visible label uses a different word.
const SEARCH_INDEX: { section: SectionId; label: string; keywords: string }[] = [
  {
    section: 'model',
    label: 'Whisper model',
    keywords: 'model speech recognition whisper transcribe download size accuracy speed',
  },
  {
    section: 'language',
    label: 'Language',
    keywords: 'language locale auto-detect english multilingual',
  },
  {
    section: 'activation',
    label: 'Activation mode',
    keywords: 'activation fn hold tap double shortcut hotkey trigger',
  },
  {
    section: 'activation',
    label: 'Global hotkey',
    keywords: 'shortcut hotkey keybinding custom capture',
  },
  {
    section: 'transcription',
    label: 'Remove filler words',
    keywords: 'um uh hmm filler clean transcription disfluency',
  },
  {
    section: 'transcription',
    label: 'Live preview while recording',
    keywords: 'streaming partial captions overlay live preview',
  },
  {
    section: 'transcription',
    label: 'Self-correction cleanup',
    keywords: 'correction restate no wait revise cleanup llm',
  },
  {
    section: 'transcription',
    label: 'Correction model',
    keywords: 'llm correction model download',
  },
  {
    section: 'vocabulary',
    label: 'Learn from corrections',
    keywords: 'vocabulary automatic learning words',
  },
  {
    section: 'vocabulary',
    label: 'Add word manually',
    keywords: 'vocabulary manual word add custom',
  },
  { section: 'general', label: 'Launch at login', keywords: 'autostart startup login boot launch' },
  { section: 'updates', label: 'Updates', keywords: 'version update upgrade release changelog' },
];

function sectionLabel(id: SectionId): string {
  return SECTIONS.find((s) => s.id === id)?.label ?? id;
}

const searchResults = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return [];
  return SEARCH_INDEX.filter(
    (i) => i.label.toLowerCase().includes(q) || i.keywords.toLowerCase().includes(q),
  );
});

function jumpToSection(id: SectionId) {
  activeSectionId.value = id;
  searchQuery.value = '';
}
</script>

<template>
  <div
    class="flex flex-col overflow-hidden"
    :class="standalone ? 'flex-1 min-h-0' : 'h-full bg-canvas rounded-xl relative surface-grain'"
  >
    <!-- Decorative top edge (embedded only) -->
    <div
      v-if="!standalone"
      class="h-[1.5px] bg-gradient-to-r from-transparent via-ink-faint/20 to-transparent"
    />

    <!-- ── Header (embedded only) ── -->
    <div v-if="!standalone" class="flex items-center gap-3 px-5 pt-5 pb-3">
      <button
        class="flex items-center justify-center w-7 h-7 rounded-lg bg-raised border border-edge text-ink-faint hover:text-ink hover:border-edge-strong hover:bg-hover transition-all duration-150 active:scale-95"
        @click="$emit('back')"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
      <h1 class="text-[15px] font-bold tracking-tight text-ink">Settings</h1>
    </div>

    <!-- ── Search bar ── -->
    <div class="search-bar">
      <div class="search-input-wrap">
        <svg
          class="search-icon"
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
          v-model="searchQuery"
          type="text"
          placeholder="Search settings…"
          class="search-input"
        />
        <button
          v-if="searchQuery"
          type="button"
          aria-label="Clear search"
          class="search-clear"
          @click="searchQuery = ''"
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

      <!-- Search results dropdown -->
      <div v-if="searchQuery.trim() && searchResults.length > 0" class="search-dropdown">
        <button
          v-for="r in searchResults"
          :key="r.section + ':' + r.label"
          class="search-result"
          @click="jumpToSection(r.section)"
        >
          <span class="search-result-label">{{ r.label }}</span>
          <span class="search-result-section">{{ sectionLabel(r.section) }}</span>
        </button>
      </div>
      <div v-else-if="searchQuery.trim()" class="search-empty">No matches.</div>
    </div>

    <!-- ── Body: sidebar + content ── -->
    <div class="flex-1 flex min-h-0 overflow-hidden">
      <!-- Sidebar -->
      <nav class="settings-sidebar">
        <button
          v-for="s in SECTIONS"
          :key="s.id"
          class="nav-item"
          :class="{ 'nav-item--active': activeSectionId === s.id }"
          @click="activeSectionId = s.id"
        >
          <svg
            class="nav-icon"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            v-html="s.icon"
          />
          <span class="nav-label">{{ s.label }}</span>
        </button>
      </nav>

      <!-- Content panel -->
      <div class="flex-1 overflow-y-auto min-h-0 px-5 pt-4 pb-5">
        <!-- ═══════════════ MODEL SECTION ═══════════════ -->
        <ModelSection v-show="activeSectionId === 'model'" @model-changed="emit('modelChanged')" />

        <!-- ═══════════════ LANGUAGE SECTION ═══════════════ -->
        <LanguageSection v-show="activeSectionId === 'language'" />

        <!-- ═══════════════ ACTIVATION SECTION ═══════════════ -->
        <ActivationSection v-show="activeSectionId === 'activation'" />

        <!-- ═══════════════ TRANSCRIPTION SECTION ═══════════════ -->
        <TranscriptionSection v-show="activeSectionId === 'transcription'" />

        <!-- ═══════════════ VOCABULARY SECTION ═══════════════ -->
        <VocabularySection v-show="activeSectionId === 'vocabulary'" />

        <!-- ═══════════════ GENERAL SECTION ═══════════════ -->
        <GeneralSection v-show="activeSectionId === 'general'" />
        <!-- ═══════════════ UPDATES SECTION ═══════════════ -->
        <div v-show="activeSectionId === 'updates'">
          <UpdatesSection />
        </div>

        <!-- Bottom spacer for safe scrolling -->
        <div class="h-2" />
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ── Search bar ─────────────────────────────────────────── */
.search-bar {
  position: relative;
  padding: 12px 16px 10px;
  border-bottom: 1px solid var(--color-edge);
}

.search-input-wrap {
  position: relative;
  display: flex;
  align-items: center;
}

.search-icon {
  position: absolute;
  left: 9px;
  color: var(--color-ink-faint);
  pointer-events: none;
}

.search-input {
  width: 100%;
  padding: 6px 26px 6px 26px;
  font-size: 12px;
  border-radius: 6px;
  background: var(--color-raised);
  border: 1px solid var(--color-edge);
  color: var(--color-ink);
  transition:
    border-color 0.12s ease,
    box-shadow 0.12s ease;
}

.search-input::placeholder {
  color: var(--color-ink-faint);
}

.search-input:focus {
  outline: none;
  border-color: rgba(232, 175, 71, 0.4);
  box-shadow: 0 0 0 3px rgba(232, 175, 71, 0.08);
}

.search-clear {
  position: absolute;
  right: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--color-edge-strong);
  color: var(--color-canvas);
  transition: background 0.12s ease;
}

.search-clear:hover {
  background: var(--color-ink-faint);
}

.search-dropdown,
.search-empty {
  position: absolute;
  top: calc(100% - 4px);
  left: 16px;
  right: 16px;
  z-index: 60;
  background: var(--color-panel);
  border: 1px solid var(--color-edge);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.3);
  overflow: hidden;
}

.search-dropdown {
  max-height: 260px;
  overflow-y: auto;
  padding: 4px;
}

.search-empty {
  padding: 10px 12px;
  font-size: 11px;
  color: var(--color-ink-faint);
}

.search-result {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 6px 10px;
  border-radius: 5px;
  text-align: left;
  transition: background 0.1s ease;
}

.search-result:hover {
  background: var(--color-raised);
}

.search-result-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--color-ink);
}

.search-result-section {
  font-size: 9px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--color-ink-faint);
}

/* ── Sidebar nav ────────────────────────────────────────── */
.settings-sidebar {
  flex-shrink: 0;
  width: 138px;
  padding: 10px 8px;
  border-right: 1px solid var(--color-edge);
  display: flex;
  flex-direction: column;
  gap: 1px;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-radius: 6px;
  text-align: left;
  font-size: 12px;
  font-weight: 500;
  color: var(--color-ink-muted);
  transition:
    background 0.12s ease,
    color 0.12s ease;
  cursor: pointer;
}

.nav-item:hover {
  background: var(--color-raised);
  color: var(--color-ink);
}

.nav-item--active {
  background: rgba(232, 175, 71, 0.08);
  color: var(--color-ink);
  font-weight: 600;
}

.nav-item--active .nav-icon {
  color: var(--color-gold);
}

.nav-icon {
  flex-shrink: 0;
  color: var(--color-ink-faint);
  transition: color 0.12s ease;
}

.nav-item:hover .nav-icon {
  color: var(--color-ink-muted);
}

.nav-label {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ── Existing styles ────────────────────────────────────── */
.settings-section {
  animation: section-enter 0.3s ease-out both;
}

@keyframes section-enter {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.section-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}

.section-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 5px;
  background: var(--color-raised);
  border: 1px solid var(--color-edge);
  color: var(--color-ink-faint);
}

.section-label {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--color-ink-faint);
}

.subsection-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--color-ink-faint);
  letter-spacing: 0.02em;
}

.rating-label {
  font-size: 8px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  color: var(--color-ink-faint);
}

.model-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid;
  transition: all 0.15s ease;
}

.model-badge {
  flex-shrink: 0;
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 8px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  background: var(--color-raised);
  border: 1px solid var(--color-edge);
  color: var(--color-ink-faint);
}

.model-badge-multi {
  background: var(--color-gold);
  background: rgba(232, 175, 71, 0.08);
  border-color: rgba(232, 175, 71, 0.2);
  color: var(--color-gold);
}

.lang-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 5px 10px;
  text-align: left;
  transition: all 0.1s ease;
}

/* ── Toggle Switch ── */
.toggle-switch {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 9px;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
}

.toggle-on {
  background: var(--color-gold);
  box-shadow: 0 0 8px rgba(232, 175, 71, 0.2);
}

.toggle-off {
  background: var(--color-edge-strong);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.toggle-on .toggle-thumb {
  left: 16px;
}

.toggle-off .toggle-thumb {
  left: 2px;
}
</style>
