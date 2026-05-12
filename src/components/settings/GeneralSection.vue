<script setup lang="ts">
import { useSettings } from '@/composables/useSettings';
import SettingsSection from '@/components/base/SettingsSection.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseButton from '@/components/base/BaseButton.vue';

const { settings, launchAtLoginStatus, openLoginItemsSettings, updateAutoStart } = useSettings();

function toggleAutoStart(value: boolean) {
  updateAutoStart(value);
}
</script>

<template>
  <SettingsSection label="General">
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
        <circle cx="12" cy="12" r="3" />
        <path
          d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z"
        />
      </svg>
    </template>

    <SettingsRow label="Launch at login" helper="Start Magpie when you log in">
      <BaseToggle :model-value="!!settings?.autoStart" @update:model-value="toggleAutoStart" />
    </SettingsRow>

    <BaseButton
      v-if="launchAtLoginStatus === 'requiresApproval'"
      variant="link"
      class="text-amber-400 mt-1.5 px-1 text-left hover:no-underline hover:text-amber-300"
      @click="openLoginItemsSettings()"
    >
      Magpie needs approval in System Settings → Login Items. Click to open.
    </BaseButton>
  </SettingsSection>
</template>
