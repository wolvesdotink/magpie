<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useSettings } from '@/composables/useSettings';
import SettingsSection from '@/components/base/SettingsSection.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import {
  getDownloadedCorrectionModels,
  downloadCorrectionModel,
  selectCorrectionModel,
  deleteCorrectionModelFile,
  cancelDownload,
  getDefaultVoiceCommandsPrompt,
  type CorrectionModelInfo,
} from '@/lib/commands';
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from '@/lib/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

const {
  settings,
  correctionModels,
  currentModel,
  updateRemoveFillers,
  updateSelfCorrection,
  updateStreamingPreview,
  updateSelectedCorrectionModel,
  updateVoiceCommandsEnabled,
  updateVoiceCommandsPrompt,
} = useSettings();

const voiceCommandsAvailable = computed(
  () => !!settings.value?.selfCorrection && !!settings.value?.selectedCorrectionModel,
);

const voiceCommandsHelper = computed(() =>
  voiceCommandsAvailable.value
    ? 'Recognize spoken edits like "scratch that", "new line", "all caps that"'
    : 'Requires self-correction with a loaded model',
);

// Built-in default voice-commands instructions (fetched once on mount). Used
// both as the textarea's effective text when the user hasn't customized and
// as the target for the "Restore default" button.
const defaultVoiceCommandsPrompt = ref<string>('');

// Effective text shown in the textarea: the user's override if they've set
// one, otherwise the built-in default. Bound via get/set so typing saves the
// override and clearing back to the default discards it (stores null).
const voiceCommandsPromptText = computed<string>({
  get: () => settings.value?.voiceCommandsPrompt ?? defaultVoiceCommandsPrompt.value,
  set: (next) => {
    const trimmed = next.trim();
    if (trimmed === '' || trimmed === defaultVoiceCommandsPrompt.value.trim()) {
      void updateVoiceCommandsPrompt(null);
    } else {
      void updateVoiceCommandsPrompt(next);
    }
  },
});

const voiceCommandsPromptIsCustom = computed(() => settings.value?.voiceCommandsPrompt != null);

function restoreDefaultVoiceCommandsPrompt() {
  void updateVoiceCommandsPrompt(null);
}

// ── Correction-model state ──
const downloadedCorrectionFiles = ref<string[]>([]);
const downloadingCorrection = ref(false);
const correctionDownloadProgress = ref(0);
const downloadingCorrectionModelId = ref<string | null>(null);
const correctionModelError = ref<string | null>(null);
const confirmDeleteCorrection = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

// Rough resident-memory estimate for a correction model (weights ≈ file size
// plus a small allowance for the llama.cpp context). Always shown with a "≈".
function ramEstimateBytes(bytes: number): number {
  return Math.round(bytes * 1.15);
}

function isCorrectionDownloaded(model: CorrectionModelInfo): boolean {
  return downloadedCorrectionFiles.value.includes(model.filename);
}

function isCorrectionActive(model: CorrectionModelInfo): boolean {
  return settings.value?.selectedCorrectionModel === model.id;
}

const sortedCorrectionModels = computed(() =>
  [...correctionModels.value].sort((a, b) => a.sizeBytes - b.sizeBytes),
);

const downloadedCorrectionModels = computed(() =>
  sortedCorrectionModels.value.filter((m) => isCorrectionDownloaded(m)),
);

const availableCorrectionModels = computed(() =>
  sortedCorrectionModels.value.filter((m) => !isCorrectionDownloaded(m)),
);

async function handleSelectCorrectionModel(model: CorrectionModelInfo) {
  if (!isCorrectionDownloaded(model)) return;
  correctionModelError.value = null;
  try {
    await selectCorrectionModel(model.id);
    await updateSelectedCorrectionModel(model.id);
  } catch (e) {
    correctionModelError.value = `Failed to load: ${e}`;
  }
}

async function handleDownloadCorrection(model: CorrectionModelInfo) {
  correctionModelError.value = null;
  downloadingCorrection.value = true;
  correctionDownloadProgress.value = 0;
  downloadingCorrectionModelId.value = model.id;
  try {
    await downloadCorrectionModel(model.id);
    await selectCorrectionModel(model.id);
    await updateSelectedCorrectionModel(model.id);
  } catch (e) {
    if (!String(e).toLowerCase().includes('cancel')) {
      correctionModelError.value = `Download failed: ${e}`;
    }
  } finally {
    downloadingCorrection.value = false;
    downloadingCorrectionModelId.value = null;
    downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();
  }
}

async function handleCancelCorrectionDownload() {
  if (!downloadingCorrectionModelId.value) return;
  try {
    await cancelDownload(downloadingCorrectionModelId.value);
  } catch (e) {
    console.error('Cancel failed:', e);
  }
}

async function handleDeleteCorrection(model: CorrectionModelInfo) {
  if (confirmDeleteCorrection.value !== model.id) {
    confirmDeleteCorrection.value = model.id;
    setTimeout(() => {
      if (confirmDeleteCorrection.value === model.id) confirmDeleteCorrection.value = null;
    }, 3000);
    return;
  }
  confirmDeleteCorrection.value = null;
  try {
    await deleteCorrectionModelFile(model.id);
    downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();
  } catch (e) {
    correctionModelError.value = `Delete failed: ${e}`;
  }
}

onMounted(async () => {
  downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();

  try {
    defaultVoiceCommandsPrompt.value = await getDefaultVoiceCommandsPrompt();
  } catch (e) {
    console.error('Failed to load default voice-commands prompt:', e);
  }

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      if (downloadingCorrectionModelId.value === progress.modelId) {
        correctionDownloadProgress.value = progress.percent;
      }
    }),
  );
  unlisteners.push(
    await onModelDownloadComplete(async () => {
      if (!downloadingCorrectionModelId.value) return;
      downloadingCorrection.value = false;
      downloadingCorrectionModelId.value = null;
      downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();
    }),
  );
  unlisteners.push(
    await onModelDownloadCancelled((data) => {
      if (downloadingCorrectionModelId.value === data.modelId) {
        downloadingCorrection.value = false;
        downloadingCorrectionModelId.value = null;
        correctionDownloadProgress.value = 0;
        correctionModelError.value = null;
      }
    }),
  );
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
});
</script>

<template>
  <SettingsSection label="Transcription">
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
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
      </svg>
    </template>

    <SettingsRow label="Remove filler words" helper='Strips "um", "uh", "hmm" and similar'>
      <BaseToggle
        :model-value="!!settings?.removeFillers"
        @update:model-value="updateRemoveFillers($event)"
      />
    </SettingsRow>

    <SettingsRow
      label="Live preview while recording"
      helper="Show partial captions in the overlay as you dictate (extra CPU load; final transcript on stop is unaffected)"
    >
      <BaseToggle
        :model-value="!!settings?.streamingPreview"
        @update:model-value="updateStreamingPreview($event)"
      />
    </SettingsRow>

    <BaseCard v-if="settings?.streamingPreview && !currentModel" tone="gold" class="mt-2">
      <span class="text-[10px] text-gold leading-snug">
        Select a transcription model below to see live captions.
      </span>
    </BaseCard>

    <SettingsRow
      label="Self-correction cleanup"
      helper='Detect and remove corrections like "no wait" or restated phrases'
    >
      <BaseToggle
        :model-value="!!settings?.selfCorrection"
        @update:model-value="updateSelfCorrection($event)"
      />
    </SettingsRow>

    <template v-if="settings?.selfCorrection">
      <BaseCard
        v-if="downloadedCorrectionModels.length === 0 && !downloadingCorrection"
        tone="gold"
        class="mt-2"
      >
        <span class="text-[10px] text-gold leading-snug">
          Download a correction model below to enable self-correction cleanup.
        </span>
      </BaseCard>

      <BaseCard
        v-if="downloadedCorrectionModels.some((m) => isCorrectionActive(m))"
        tone="gold"
        class="mt-2"
      >
        <div class="flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full bg-leaf shadow-[0_0_4px_rgba(95,183,96,0.5)]" />
          <span class="text-[11px] font-semibold text-ink">
            {{ downloadedCorrectionModels.find((m) => isCorrectionActive(m))?.displayName }}
          </span>
          <span class="text-[9px] text-ink-faint tabular-nums ml-auto">
            ≈{{
              formatBytes(
                ramEstimateBytes(
                  downloadedCorrectionModels.find((m) => isCorrectionActive(m))?.sizeBytes ?? 0,
                ),
              )
            }}
            in memory
          </span>
        </div>
      </BaseCard>

      <div v-if="downloadedCorrectionModels.length > 0" class="mt-2">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
          Correction models
        </span>
        <div class="flex flex-col gap-1.5 mt-1.5">
          <div
            v-for="model in downloadedCorrectionModels"
            :key="model.id"
            class="group flex items-center gap-2 px-2.5 py-2 rounded-lg border transition-all duration-150"
            :class="{
              'bg-gold/[0.03] border-gold/15': isCorrectionActive(model),
              'bg-panel border-edge hover:border-edge-strong hover:bg-raised':
                !isCorrectionActive(model),
            }"
          >
            <button
              class="flex-1 flex items-center gap-2.5 min-w-0 text-left"
              @click="handleSelectCorrectionModel(model)"
            >
              <div
                class="w-3.5 h-3.5 rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 transition-all duration-200"
                :class="
                  isCorrectionActive(model)
                    ? 'border-gold bg-gold/10'
                    : 'border-edge-strong group-hover:border-ink-faint'
                "
              >
                <div v-if="isCorrectionActive(model)" class="w-1.5 h-1.5 rounded-full bg-gold" />
              </div>
              <div class="flex flex-col min-w-0">
                <span
                  class="text-[12px] font-semibold truncate"
                  :class="isCorrectionActive(model) ? 'text-ink' : 'text-ink-muted'"
                >
                  {{ model.displayName }}
                </span>
                <span class="text-[10px] text-ink-faint tabular-nums">
                  {{ formatBytes(model.sizeBytes) }}
                </span>
              </div>
            </button>
            <button
              v-if="!isCorrectionActive(model)"
              class="flex-shrink-0 p-1 rounded-md opacity-0 group-hover:opacity-100 transition-all duration-150"
              :class="
                confirmDeleteCorrection === model.id
                  ? 'bg-flame/15 text-flame opacity-100'
                  : 'text-ink-faint hover:text-flame hover:bg-flame/10'
              "
              @click.stop="handleDeleteCorrection(model)"
            >
              <svg
                v-if="confirmDeleteCorrection !== model.id"
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <polyline points="3 6 5 6 21 6" />
                <path
                  d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"
                />
              </svg>
              <span v-else class="text-[9px] font-bold uppercase tracking-wider px-0.5">
                Delete?
              </span>
            </button>
          </div>
        </div>
      </div>

      <div v-if="downloadingCorrection" class="mt-2">
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-[11px] text-ink-muted font-medium">Downloading…</span>
          <div class="flex items-center gap-2">
            <span class="text-[11px] text-ink-faint tabular-nums">
              {{ correctionDownloadProgress.toFixed(0) }}%
            </span>
            <button
              type="button"
              aria-label="Cancel download"
              title="Cancel download"
              class="flex items-center justify-center w-[18px] h-[18px] rounded-full bg-raised border border-edge text-ink-faint transition-colors duration-150 hover:bg-panel hover:text-ink hover:border-edge-strong active:scale-95"
              @click="handleCancelCorrectionDownload"
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
        </div>
        <div class="h-1 bg-raised shadow-well rounded-full overflow-hidden">
          <div
            class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full transition-[width] duration-300 ease-out"
            :style="{ width: `${correctionDownloadProgress}%` }"
          />
        </div>
      </div>

      <div v-if="availableCorrectionModels.length > 0" class="mt-2">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
          Available to download
        </span>
        <div class="flex flex-col gap-1.5 mt-1.5">
          <div
            v-for="model in availableCorrectionModels"
            :key="model.id"
            class="flex items-center gap-2 px-2.5 py-2 rounded-lg border bg-panel border-edge transition-all duration-150"
          >
            <div class="flex-1 flex flex-col min-w-0">
              <span class="text-[12px] font-semibold text-ink-muted truncate">
                {{ model.displayName }}
              </span>
              <div class="flex items-center gap-3 mt-0.5">
                <span class="text-[10px] text-ink-faint tabular-nums">
                  {{ formatBytes(model.sizeBytes) }}
                </span>
                <div class="flex items-center gap-1">
                  <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
                    Spd
                  </span>
                  <div class="flex gap-[2px]">
                    <span
                      v-for="i in 5"
                      :key="'csp-' + model.id + i"
                      class="w-[3px] h-[3px] rounded-full"
                      :class="i <= model.speedRating ? 'bg-gold/70' : 'bg-edge'"
                    />
                  </div>
                </div>
                <div class="flex items-center gap-1">
                  <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
                    Qual
                  </span>
                  <div class="flex gap-[2px]">
                    <span
                      v-for="i in 5"
                      :key="'cql-' + model.id + i"
                      class="w-[3px] h-[3px] rounded-full"
                      :class="i <= model.qualityRating ? 'bg-gold/70' : 'bg-edge'"
                    />
                  </div>
                </div>
              </div>
            </div>
            <button
              class="flex-shrink-0 flex items-center gap-1 px-2 py-1 rounded-md bg-raised border border-edge text-[10px] font-semibold text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95"
              :disabled="downloadingCorrection"
              @click="handleDownloadCorrection(model)"
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4" />
                <polyline points="7 10 12 15 17 10" />
                <line x1="12" y1="15" x2="12" y2="3" />
              </svg>
              Get
            </button>
          </div>
        </div>
      </div>

      <BaseCard v-if="correctionModelError" tone="flame" padding="sm" class="mt-2">
        <span class="text-[11px] text-flame">{{ correctionModelError }}</span>
      </BaseCard>
    </template>

    <SettingsRow label="Voice editing commands" :helper="voiceCommandsHelper">
      <BaseToggle
        :model-value="!!settings?.voiceCommandsEnabled"
        :disabled="!voiceCommandsAvailable"
        @update:model-value="updateVoiceCommandsEnabled($event)"
      />
    </SettingsRow>

    <template v-if="settings?.voiceCommandsEnabled && voiceCommandsAvailable">
      <div class="mt-2">
        <div class="flex items-center justify-between mb-1.5">
          <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
            Commands instructions
          </span>
          <button
            v-if="voiceCommandsPromptIsCustom"
            type="button"
            class="text-[10px] font-semibold text-ink-faint hover:text-ink transition-colors duration-150"
            @click="restoreDefaultVoiceCommandsPrompt"
          >
            Restore default
          </button>
        </div>
        <BaseCard tone="dashed">
          <textarea
            v-model="voiceCommandsPromptText"
            rows="10"
            placeholder="Loading default…"
            class="w-full min-w-0 rounded-md bg-raised border border-edge text-ink text-[11px] px-2 py-1.5 placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 focus:shadow-[0_0_0_3px_rgba(232,175,71,0.08)] resize-y font-mono"
          />
          <div class="mt-1 text-[9px] text-ink-faint leading-snug">
            Appended to the correction system prompt. Edit to add domain-specific commands or
            tighten the false-positive rules.
          </div>
        </BaseCard>
      </div>
    </template>
  </SettingsSection>
</template>
