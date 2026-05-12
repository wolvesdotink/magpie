<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import {
  getAvailableModels,
  getDownloadedModels,
  getSettings,
  downloadModel,
  cancelDownload,
  selectModel,
  type ModelInfo,
} from '@/lib/commands';
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from '@/lib/events';
import type { UnlistenFn } from '@tauri-apps/api/event';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import Badge from '@/components/base/Badge.vue';
import ProgressBar from '@/components/base/ProgressBar.vue';
import RatingDots from '@/components/base/RatingDots.vue';

type ModelTab = 'english' | 'multilingual';

const props = withDefaults(
  defineProps<{
    /** Default tab. App.vue uses 'english', setup wizard uses 'multilingual'. */
    initialTab?: ModelTab;
    /** When true, switch to multilingual on mount if the user's saved language ≠ 'en'. */
    autoDetectTab?: boolean;
    /** When true, wrap the picker in canvas chrome (top edge, full-height). */
    withChrome?: boolean;
    /** Pre-select a model on mount (used by setup wizard: 'small'). */
    initialSelectedId?: string;
    readyLabel?: string;
    downloadLabel?: string;
  }>(),
  {
    initialTab: 'english',
    autoDetectTab: false,
    withChrome: false,
    initialSelectedId: '',
    readyLabel: 'Use Selected Model',
    downloadLabel: 'Download & Use',
  },
);

const emit = defineEmits<{
  done: [];
}>();

const models = ref<ModelInfo[]>([]);
const downloadedFiles = ref<string[]>([]);
const activeTab = ref<ModelTab>(props.initialTab);
const selectedModelId = ref<string | null>(props.initialSelectedId || null);
const downloading = ref(false);
const downloadProgress = ref(0);
const downloadingModelId = ref<string | null>(null);
const error = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function isDownloaded(model: ModelInfo): boolean {
  return downloadedFiles.value.includes(model.filename);
}

function totalDownloadBytes(model: ModelInfo): number {
  return model.sizeBytes + (model.encoderSizeBytes ?? 0);
}

const HIDDEN_FROM_PICKER = new Set(['medium.en']);

const displayedModels = computed(() =>
  models.value
    .filter((m) => !HIDDEN_FROM_PICKER.has(m.id))
    .filter((m) => (activeTab.value === 'english' ? m.englishOnly : !m.englishOnly))
    .sort((a, b) => {
      const tabTag = activeTab.value;
      const aRec = a.recommendedFor === tabTag ? 0 : 1;
      const bRec = b.recommendedFor === tabTag ? 0 : 1;
      if (aRec !== bRec) return aRec - bRec;
      return totalDownloadBytes(a) - totalDownloadBytes(b);
    }),
);

function isRecommended(model: ModelInfo): boolean {
  return model.recommendedFor === activeTab.value;
}

async function handleSelect(model: ModelInfo) {
  selectedModelId.value = model.id;
  if (isDownloaded(model)) {
    try {
      await selectModel(model.id);
    } catch (e) {
      error.value = `Failed to load model: ${e}`;
    }
  }
}

async function handlePrimary() {
  if (!selectedModelId.value) return;
  const model = models.value.find((m) => m.id === selectedModelId.value);
  if (!model) return;

  if (isDownloaded(model)) {
    try {
      await selectModel(model.id);
      emit('done');
    } catch (e) {
      error.value = `Failed to load model: ${e}`;
    }
    return;
  }

  error.value = null;
  downloading.value = true;
  downloadProgress.value = 0;
  downloadingModelId.value = selectedModelId.value;
  try {
    await downloadModel(selectedModelId.value);
    emit('done');
  } catch (e) {
    if (!String(e).toLowerCase().includes('cancel')) {
      error.value = `Download failed: ${e}`;
    }
    downloading.value = false;
    downloadingModelId.value = null;
  }
}

async function handleCancel() {
  if (!downloadingModelId.value) return;
  try {
    await cancelDownload(downloadingModelId.value);
  } catch (e) {
    console.error('Cancel failed:', e);
  }
}

onMounted(async () => {
  try {
    const [availableModels, downloaded] = await Promise.all([
      getAvailableModels(),
      getDownloadedModels(),
    ]);
    models.value = availableModels;
    downloadedFiles.value = downloaded;

    if (props.autoDetectTab) {
      const settings = await getSettings();
      if (settings.language && settings.language !== 'en') {
        activeTab.value = 'multilingual';
      }
    }
  } catch (e) {
    console.error('Failed to load models:', e);
  }

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      if (downloadingModelId.value && downloadingModelId.value !== progress.modelId) return;
      downloadProgress.value = progress.percent;
    }),
  );
  unlisteners.push(
    await onModelDownloadComplete(async () => {
      downloading.value = false;
      downloadingModelId.value = null;
      try {
        downloadedFiles.value = await getDownloadedModels();
      } catch (e) {
        console.error('Failed to refresh downloaded models:', e);
      }
    }),
  );
  unlisteners.push(
    await onModelDownloadCancelled((data) => {
      if (downloadingModelId.value && downloadingModelId.value !== data.modelId) return;
      downloading.value = false;
      downloadingModelId.value = null;
      downloadProgress.value = 0;
      error.value = null;
    }),
  );
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
});

const primaryLabel = computed(() => {
  const m = selectedModelId.value ? models.value.find((m) => m.id === selectedModelId.value) : null;
  if (m && isDownloaded(m)) return props.readyLabel;
  return props.downloadLabel;
});
</script>

<template>
  <div
    :class="
      withChrome
        ? 'flex flex-col h-full bg-canvas rounded-xl overflow-hidden'
        : 'flex flex-col flex-1 min-h-0'
    "
  >
    <div
      v-if="withChrome"
      class="h-px bg-gradient-to-r from-transparent via-gold/20 to-transparent"
    />

    <div class="flex flex-col flex-1 p-5 pt-6 min-h-0">
      <h2 class="text-[15px] font-bold tracking-tight text-ink">Choose a Model</h2>
      <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 mb-4">
        Select a speech recognition model. Larger models are more accurate but slower.
      </p>

      <div class="flex gap-1 p-1 rounded-lg bg-raised border border-edge mb-4">
        <button
          class="flex-1 py-1.5 rounded-md text-[12px] font-semibold transition-all duration-200"
          :class="
            activeTab === 'english'
              ? 'bg-canvas text-ink shadow-soft'
              : 'text-ink-faint hover:text-ink-muted'
          "
          @click="activeTab = 'english'"
        >
          English
        </button>
        <button
          class="flex-1 py-1.5 rounded-md text-[12px] font-semibold transition-all duration-200"
          :class="
            activeTab === 'multilingual'
              ? 'bg-canvas text-ink shadow-soft'
              : 'text-ink-faint hover:text-ink-muted'
          "
          @click="activeTab = 'multilingual'"
        >
          Multilingual
        </button>
      </div>

      <div class="flex-1 overflow-y-auto flex flex-col gap-2 mb-4 -mx-1 px-1">
        <button
          v-for="model in displayedModels"
          :key="model.id"
          class="relative flex flex-col gap-1.5 p-3 rounded-lg border text-left transition-all duration-200 active:scale-[0.99]"
          :class="{
            'bg-gold/[0.05] border-gold/30 shadow-glow-gold': selectedModelId === model.id,
            'bg-panel border-edge shadow-soft hover:bg-raised hover:border-edge-strong':
              selectedModelId !== model.id,
          }"
          @click="handleSelect(model)"
        >
          <div class="flex items-center justify-between gap-2">
            <div class="flex items-center gap-2 min-w-0">
              <span class="text-[13px] font-semibold text-ink truncate">
                {{ model.displayName }}
              </span>
              <Badge v-if="isRecommended(model)" tone="gold" size="sm" class="flex-shrink-0">
                Recommended
              </Badge>
            </div>
            <div class="flex items-center gap-2 flex-shrink-0">
              <span
                v-if="isDownloaded(model)"
                class="flex items-center gap-1 text-[10px] font-bold text-leaf uppercase tracking-wider"
              >
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="3"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <polyline points="20 6 9 17 4 12" />
                </svg>
                Ready
              </span>
              <span class="text-[11px] text-ink-faint font-medium tabular-nums">
                {{ formatBytes(totalDownloadBytes(model)) }}
              </span>
            </div>
          </div>

          <p class="text-[11px] text-ink-muted leading-snug">
            {{ model.description }}
          </p>

          <div class="flex gap-4 mt-0.5">
            <div class="flex items-center gap-1.5">
              <span class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint">
                Speed
              </span>
              <RatingDots :value="model.speedRating" reverse size="sm" />
            </div>
            <div class="flex items-center gap-1.5">
              <span class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint">
                Accuracy
              </span>
              <RatingDots :value="model.accuracyRating" size="sm" />
            </div>
          </div>
        </button>
      </div>

      <BaseCard v-if="error" tone="flame" class="mb-3">
        <span class="text-[10px] text-flame">{{ error }}</span>
      </BaseCard>

      <div
        v-if="downloading && downloadingModelId === selectedModelId"
        class="flex items-center gap-3"
      >
        <ProgressBar class="flex-1" :value="downloadProgress" height="normal" />
        <span class="text-[10px] text-ink-muted font-medium tabular-nums min-w-[36px] text-right">
          {{ downloadProgress.toFixed(0) }}%
        </span>
        <button
          type="button"
          aria-label="Cancel download"
          title="Cancel download"
          class="flex items-center justify-center w-[18px] h-[18px] rounded-full bg-raised border border-edge text-ink-faint transition-colors duration-150 hover:bg-panel hover:text-ink hover:border-edge-strong active:scale-95"
          @click="handleCancel"
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

      <BaseButton
        v-else
        variant="primary"
        size="lg"
        full-width
        :disabled="!selectedModelId || downloading"
        @click="handlePrimary"
      >
        {{ primaryLabel }}
      </BaseButton>
    </div>
  </div>
</template>
