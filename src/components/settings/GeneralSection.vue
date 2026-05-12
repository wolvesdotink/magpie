<script setup lang="ts">
import { useSettings } from '@/composables/useSettings';

const { settings, launchAtLoginStatus, openLoginItemsSettings, updateAutoStart } = useSettings();
</script>

<template>
  <section class="settings-section">
    <div class="section-header">
      <div class="section-icon">
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
      </div>
      <span class="section-label">General</span>
    </div>

    <div class="flex items-center justify-between p-2.5 rounded-lg bg-panel border border-edge">
      <div class="flex flex-col min-w-0 mr-3">
        <span class="text-[12px] font-semibold text-ink"> Launch at login </span>
        <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
          Start Magpie when you log in
        </span>
      </div>
      <button
        class="toggle-switch flex-shrink-0"
        :class="settings?.autoStart ? 'toggle-on' : 'toggle-off'"
        @click="updateAutoStart(!settings?.autoStart)"
      >
        <div class="toggle-thumb" />
      </button>
    </div>
    <button
      v-if="launchAtLoginStatus === 'requiresApproval'"
      class="text-[10px] text-amber-400 mt-1.5 px-1 text-left hover:underline cursor-pointer"
      @click="openLoginItemsSettings()"
    >
      Magpie needs approval in System Settings → Login Items. Click to open.
    </button>
  </section>
</template>
