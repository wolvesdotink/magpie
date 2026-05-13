<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useSettings } from '@/composables/useSettings';
import { useUpdater } from '@/composables/useUpdater';
import SettingsSection from '@/components/base/SettingsSection.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';

const RELEASES_URL = 'https://github.com/wolvesdotink/magpie/releases/latest';

const { state, checkNow, install, restart } = useUpdater();
const { settings, updateUpdateChannel } = useSettings();

const currentVersion = ref<string | null>(null);
onMounted(async () => {
  try {
    const { getVersion } = await import('@tauri-apps/api/app');
    currentVersion.value = await getVersion();
  } catch {
    // Outside Tauri runtime (e.g. plain Vite preview) — leave null.
  }
});

async function toggleBetaChannel(value: boolean) {
  await updateUpdateChannel(value ? 'beta' : 'stable');
}

const percent = computed(() => {
  if (state.value.totalBytes <= 0) return null;
  return Math.min(99, Math.floor((state.value.downloaded / state.value.totalBytes) * 100));
});

const checkButtonLabel = computed(() => {
  switch (state.value.status) {
    case 'checking':
      return 'Checking…';
    case 'downloading':
      return percent.value === null ? 'Downloading…' : `Downloading… ${percent.value}%`;
    default:
      return 'Check now';
  }
});

const checkButtonDisabled = computed(
  () => state.value.status === 'checking' || state.value.status === 'downloading',
);

async function openReleases() {
  try {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(RELEASES_URL);
  } catch {
    window.open(RELEASES_URL, '_blank', 'noopener');
  }
}
</script>

<template>
  <SettingsSection label="Updates">
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
        <path d="M21 12a9 9 0 11-3-6.7L21 8" />
        <polyline points="21 3 21 8 16 8" />
      </svg>
    </template>

    <SettingsRow
      v-if="state.status === 'idle' || state.status === 'checking'"
      label="You're up to date"
      helper="Magpie checks for updates automatically when settings open."
    >
      <BaseButton size="sm" :disabled="checkButtonDisabled" @click="checkNow">
        {{ checkButtonLabel }}
      </BaseButton>
    </SettingsRow>

    <BaseCard v-else-if="state.status === 'available'" tone="gold" padding="lg">
      <div class="flex items-center justify-between mb-1.5">
        <div class="flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full bg-gold shadow-[0_0_4px_rgba(232,175,71,0.5)]" />
          <span class="text-[12px] font-semibold text-ink">Update available</span>
          <span v-if="state.newVersion" class="text-[10px] text-ink-faint tabular-nums">
            {{ state.newVersion }}
          </span>
        </div>
        <BaseButton variant="primary" size="sm" @click="install"> Install </BaseButton>
      </div>
      <p v-if="state.notes" class="text-[11px] text-ink-muted leading-snug whitespace-pre-line">
        {{ state.notes }}
      </p>
    </BaseCard>

    <BaseCard v-else-if="state.status === 'downloading'" padding="lg">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[12px] font-semibold text-ink">Installing update…</span>
        <span v-if="percent !== null" class="text-[11px] text-ink-faint tabular-nums">
          {{ percent }}%
        </span>
      </div>
      <div class="h-1 bg-raised shadow-well rounded-full overflow-hidden">
        <div
          class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full transition-[width] duration-300 ease-out"
          :style="{ width: `${percent ?? 0}%` }"
        />
      </div>
    </BaseCard>

    <SettingsRow
      v-else-if="state.status === 'ready'"
      label="Update installed"
      helper="Restart to start using the new version."
      tone="leaf"
    >
      <BaseButton variant="primary" size="sm" @click="restart"> Restart </BaseButton>
    </SettingsRow>

    <BaseCard v-else-if="state.status === 'error'" tone="flame">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[11px] font-semibold text-flame">Update failed</span>
        <div class="flex gap-1.5">
          <BaseButton size="sm" @click="checkNow">Retry</BaseButton>
          <BaseButton size="sm" @click="openReleases">Manual DL</BaseButton>
        </div>
      </div>
      <p v-if="state.error" class="text-[10px] text-flame/80 leading-snug break-all">
        {{ state.error }}
      </p>
    </BaseCard>

    <SettingsRow
      label="Receive beta updates"
      helper="Get prerelease builds when available. May be less stable."
    >
      <BaseToggle
        :model-value="settings?.updateChannel === 'beta'"
        @update:model-value="toggleBetaChannel"
      />
    </SettingsRow>

    <p v-if="currentVersion" class="text-[10px] text-ink-faint text-center tabular-nums mt-1">
      Magpie v{{ currentVersion }}
    </p>
  </SettingsSection>
</template>
