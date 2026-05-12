<script setup lang="ts">
import { computed } from 'vue';
import { useUpdater } from '@/composables/useUpdater';

const RELEASES_URL = 'https://github.com/wolvesdotink/magpie/releases/latest';

const { state, checkNow, install, restart } = useUpdater();

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
  <section class="settings-section" style="animation-delay: 240ms">
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
          <path d="M21 12a9 9 0 11-3-6.7L21 8" />
          <polyline points="21 3 21 8 16 8" />
        </svg>
      </div>
      <span class="section-label">Updates</span>
    </div>

    <!-- Idle / no-update — show check button -->
    <div
      v-if="state.status === 'idle' || state.status === 'checking'"
      class="flex items-center justify-between p-2.5 rounded-lg bg-panel border border-edge"
    >
      <div class="flex flex-col min-w-0 mr-3">
        <span class="text-[12px] font-semibold text-ink"> You're up to date </span>
        <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
          Magpie checks for updates automatically when settings open.
        </span>
      </div>
      <button
        class="flex-shrink-0 px-2.5 py-1 rounded-md bg-raised border border-edge text-[10px] font-semibold text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed"
        :disabled="checkButtonDisabled"
        @click="checkNow"
      >
        {{ checkButtonLabel }}
      </button>
    </div>

    <!-- Update available -->
    <div
      v-else-if="state.status === 'available'"
      class="p-3 rounded-lg bg-gold/[0.06] border border-gold/20"
    >
      <div class="flex items-center justify-between mb-1.5">
        <div class="flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full bg-gold shadow-[0_0_4px_rgba(232,175,71,0.5)]" />
          <span class="text-[12px] font-semibold text-ink"> Update available </span>
          <span v-if="state.newVersion" class="text-[10px] text-ink-faint tabular-nums">
            {{ state.newVersion }}
          </span>
        </div>
        <button
          class="flex-shrink-0 px-2.5 py-1 rounded-md bg-gold text-canvas border-0 text-[10px] font-semibold hover:bg-gold-hover transition-all duration-150 active:scale-95"
          @click="install"
        >
          Install
        </button>
      </div>
      <p v-if="state.notes" class="text-[11px] text-ink-muted leading-snug whitespace-pre-line">
        {{ state.notes }}
      </p>
    </div>

    <!-- Downloading -->
    <div
      v-else-if="state.status === 'downloading'"
      class="p-3 rounded-lg bg-panel border border-edge"
    >
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[12px] font-semibold text-ink"> Installing update… </span>
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
    </div>

    <!-- Ready to restart -->
    <div
      v-else-if="state.status === 'ready'"
      class="flex items-center justify-between p-2.5 rounded-lg bg-leaf/[0.06] border border-leaf/20"
    >
      <div class="flex flex-col min-w-0 mr-3">
        <span class="text-[12px] font-semibold text-ink"> Update installed </span>
        <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
          Restart to start using the new version.
        </span>
      </div>
      <button
        class="flex-shrink-0 px-2.5 py-1 rounded-md bg-ink text-canvas border-0 text-[10px] font-semibold hover:bg-ink/90 transition-all duration-150 active:scale-95"
        @click="restart"
      >
        Restart
      </button>
    </div>

    <!-- Error -->
    <div
      v-else-if="state.status === 'error'"
      class="p-2.5 rounded-lg bg-flame/10 border border-flame/20"
    >
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[11px] font-semibold text-flame"> Update failed </span>
        <div class="flex gap-1.5">
          <button
            class="px-2 py-0.5 rounded-md text-[10px] font-semibold bg-raised border border-edge text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95"
            @click="checkNow"
          >
            Retry
          </button>
          <button
            class="px-2 py-0.5 rounded-md text-[10px] font-semibold bg-raised border border-edge text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95"
            @click="openReleases"
          >
            Manual DL
          </button>
        </div>
      </div>
      <p v-if="state.error" class="text-[10px] text-flame/80 leading-snug break-all">
        {{ state.error }}
      </p>
    </div>
  </section>
</template>
