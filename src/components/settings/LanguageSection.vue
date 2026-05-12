<script setup lang="ts">
import { useSettings } from '@/composables/useSettings';
import SettingsSection from '@/components/base/SettingsSection.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import LanguageDropdown from '@/components/shared/LanguageDropdown.vue';

const { isEnglishOnlyModel } = useSettings();
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

    <LanguageDropdown variant="full" />
  </SettingsSection>
</template>
