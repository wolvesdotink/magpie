<script setup lang="ts">
import { useSettings } from '@/composables/useSettings';
import SettingsSection from '@/components/base/SettingsSection.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import LanguageDropdown from '@/components/shared/LanguageDropdown.vue';
import { LANGUAGES } from '@/lib/languages';
import SettingsRow from '@/components/base/SettingsRow.vue';

const { isEnglishOnlyModel, settings, updateTranslationLanguage } = useSettings();

function selectTranslation(event: Event) {
  const code = (event.target as HTMLSelectElement).value;
  void updateTranslationLanguage(code || null);
}
</script>

<template>
  <SettingsSection label="Language">
    <template #icon>
      <svg
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M2 12h20" />
        <path
          d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"
        />
      </svg>
    </template>

    <BaseCard v-if="isEnglishOnlyModel" tone="gold" padding="sm" class="mb-2.5">
      <span class="text-[10px] text-gold leading-snug">
        Switch to a multilingual model to unlock other languages.
      </span>
    </BaseCard>

    <p class="text-[10px] text-ink-faint mb-2">Spoken language</p>
    <LanguageDropdown variant="full" />
    <SettingsRow
      label="Output language"
      helper="Translate final text and live captions on your Mac"
    >
      <select
        aria-label="Output language"
        :value="settings?.translationLanguage ?? ''"
        class="max-w-40 rounded-md bg-panel border border-edge text-ink text-[11px] p-1.5"
        @change="selectTranslation"
      >
        <option value="">Same as spoken</option>
        <option v-for="language in LANGUAGES.slice(1)" :key="language.code" :value="language.code">
          {{ language.name }}
        </option>
      </select>
    </SettingsRow>
    <BaseCard v-if="settings?.translationLanguage" tone="gold" padding="sm">
      <span class="text-[10px] text-gold leading-snug">
        {{
          settings.selectedCorrectionModel
            ? 'Uses your local language model. Live translation adds a short delay; quality varies by language and model. Qwen3.5 2B or 4B is recommended.'
            : 'Download and select a language model in Transcription settings to enable translation.'
        }}
      </span>
    </BaseCard>
  </SettingsSection>
</template>
