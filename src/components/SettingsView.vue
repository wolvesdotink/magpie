<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import type { UnlistenFn } from '@tauri-apps/api/event';
import UpdatesSection from '@/components/UpdatesSection.vue';
import ActivationSection from '@/components/settings/ActivationSection.vue';
import GeneralSection from '@/components/settings/GeneralSection.vue';
import LanguageSection from '@/components/settings/LanguageSection.vue';
import ModelSection from '@/components/settings/ModelSection.vue';
import ProfilesSection from '@/components/settings/ProfilesSection.vue';
import StylesSection from '@/components/settings/StylesSection.vue';
import TranscriptionSection from '@/components/settings/TranscriptionSection.vue';
import VocabularySection from '@/components/settings/VocabularySection.vue';
import SettingsSidebar from '@/components/shared/SettingsSidebar.vue';
import SettingsSearch from '@/components/shared/SettingsSearch.vue';

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

type SectionId =
  | 'model'
  | 'language'
  | 'activation'
  | 'transcription'
  | 'vocabulary'
  | 'profiles'
  | 'styles'
  | 'general'
  | 'updates';

const SECTIONS: { id: SectionId; label: string; icon: string }[] = [
  {
    id: 'general',
    label: 'General',
    icon: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"/>',
  },
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
    id: 'profiles',
    label: 'App Profiles',
    icon: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',
  },
  {
    id: 'styles',
    label: 'Styles',
    icon: '<circle cx="13.5" cy="6.5" r="2.5"/><path d="M19 19H5l4-6 3 4 2-3z"/>',
  },
  {
    id: 'updates',
    label: 'Updates',
    icon: '<polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>',
  },
];

const activeSectionId = ref<SectionId>('model');
const searchQuery = ref('');

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
  {
    section: 'profiles',
    label: 'Per-app profiles',
    keywords: 'profiles app slack mail terminal vscode cursor bundle id frontmost detect',
  },
  {
    section: 'styles',
    label: 'Reusable styles',
    keywords:
      'styles casing snake camel pascal kebab punctuation casual formal prompt regex custom rules',
  },
  { section: 'general', label: 'Launch at login', keywords: 'autostart startup login boot launch' },
  {
    section: 'updates',
    label: 'Receive beta updates',
    keywords: 'beta channel prerelease updates opt-in early access',
  },
  { section: 'updates', label: 'Updates', keywords: 'version update upgrade release changelog' },
];

function sectionLabel(id: SectionId): string {
  return SECTIONS.find((s) => s.id === id)?.label ?? id;
}

function jumpToSection(id: SectionId) {
  activeSectionId.value = id;
}

// Tray "Check for Updates…" also jumps us to the Updates tab so the
// concurrent check kicked off by `useUpdater` is visible.
let unlistenCheckUpdates: UnlistenFn | null = null;

onMounted(async () => {
  try {
    const { listen } = await import('@tauri-apps/api/event');
    unlistenCheckUpdates = await listen('menu://check-for-updates', () => {
      jumpToSection('updates');
    });
  } catch (e) {
    console.debug('[settings] menu listener not registered:', e);
  }
});

onUnmounted(() => {
  if (unlistenCheckUpdates) {
    unlistenCheckUpdates();
    unlistenCheckUpdates = null;
  }
});
</script>

<template>
  <div
    class="flex flex-col overflow-hidden"
    :class="standalone ? 'flex-1 min-h-0' : 'h-full bg-canvas rounded-xl relative surface-grain'"
  >
    <div
      v-if="!standalone"
      class="h-[1.5px] bg-gradient-to-r from-transparent via-ink-faint/20 to-transparent"
    />

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

    <SettingsSearch
      v-model:query="searchQuery"
      :index="SEARCH_INDEX"
      :section-label="sectionLabel"
      @jump="jumpToSection"
    />

    <div class="flex-1 flex min-h-0 overflow-hidden">
      <SettingsSidebar v-model="activeSectionId" :items="SECTIONS" />

      <div class="flex-1 overflow-y-auto min-h-0 px-5 pt-4 pb-5">
        <ModelSection v-show="activeSectionId === 'model'" @model-changed="emit('modelChanged')" />
        <LanguageSection v-show="activeSectionId === 'language'" />
        <ActivationSection v-show="activeSectionId === 'activation'" />
        <TranscriptionSection v-show="activeSectionId === 'transcription'" />
        <VocabularySection v-show="activeSectionId === 'vocabulary'" />
        <ProfilesSection
          v-show="activeSectionId === 'profiles'"
          @navigate-to-style="jumpToSection('styles')"
        />
        <StylesSection v-show="activeSectionId === 'styles'" />
        <GeneralSection v-show="activeSectionId === 'general'" />
        <div v-show="activeSectionId === 'updates'">
          <UpdatesSection />
        </div>

        <div class="h-2" />
      </div>
    </div>
  </div>
</template>
