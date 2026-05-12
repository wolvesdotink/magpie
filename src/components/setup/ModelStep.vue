<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import {
  getAvailableModels,
  getDownloadedModels,
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

const emit = defineEmits<{
  next: [];
}>();

const models = ref<ModelInfo[]>([]);
const downloadedFiles = ref<string[]>([]);
type ModelTab = 'english' | 'multilingual';
const activeTab = ref<ModelTab>('multilingual');
const selectedModelId = ref<string | null>('small');
const downloading = ref(false);
const downloadProgress = ref(0);
const downloadingModelId = ref<string | null>(null);
const modelError = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function isDownloaded(model: ModelInfo): boolean {
  return downloadedFiles.value.includes(model.filename);
}

const displayedModels = computed(() =>
  models.value
    .filter((m) => (activeTab.value === 'english' ? m.englishOnly : !m.englishOnly))
    .sort((a, b) => a.sizeBytes - b.sizeBytes),
);

async function handleSelectModel(model: ModelInfo) {
  selectedModelId.value = model.id;
  if (isDownloaded(model)) {
    try {
      await selectModel(model.id);
    } catch (e) {
      modelError.value = `Failed to load model: ${e}`;
    }
  }
}

async function handleDownloadAndContinue() {
  if (!selectedModelId.value) return;

  const selected = models.value.find((m) => m.id === selectedModelId.value);
  if (selected && isDownloaded(selected)) {
    try {
      await selectModel(selected.id);
      emit('next');
    } catch (e) {
      modelError.value = `Failed to load model: ${e}`;
    }
    return;
  }

  modelError.value = null;
  downloading.value = true;
  downloadProgress.value = 0;
  downloadingModelId.value = selectedModelId.value;

  try {
    await downloadModel(selectedModelId.value);
    emit('next');
  } catch (e) {
    if (!String(e).toLowerCase().includes('cancel')) {
      modelError.value = `Download failed: ${e}`;
    }
    downloading.value = false;
    downloadingModelId.value = null;
  }
}

async function handleCancelDownload() {
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
  } catch (e) {
    console.error('Model fetch failed:', e);
  }

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      if (downloadingModelId.value === progress.modelId) {
        downloadProgress.value = progress.percent;
      }
    }),
  );
  unlisteners.push(
    await onModelDownloadComplete(() => {
      if (!downloadingModelId.value) return;
      downloading.value = false;
      downloadingModelId.value = null;
    }),
  );
  unlisteners.push(
    await onModelDownloadCancelled((data) => {
      if (downloadingModelId.value === data.modelId) {
        downloading.value = false;
        downloadingModelId.value = null;
        downloadProgress.value = 0;
        modelError.value = null;
      }
    }),
  );
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
});
</script>

<template>
  <div class="flex flex-col flex-1 p-5 pt-6 min-h-0">
    <!-- Header -->
    <h2 class="text-[15px] font-bold tracking-tight text-ink">Choose a Model</h2>
    <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 mb-4">
      Select a speech recognition model. Larger models are more accurate but slower.
    </p>

    <!-- Tabs -->
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

    <!-- Model List -->
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
        @click="handleSelectModel(model)"
      >
        <!-- Name + meta row -->
        <div class="flex items-center justify-between gap-2">
          <div class="flex items-center gap-2">
            <span class="text-[13px] font-semibold text-ink">
              {{ model.displayName }}
            </span>
            <span
              v-if="model.id === 'small'"
              class="px-1.5 py-[1px] rounded text-[8px] font-bold uppercase tracking-wider bg-gold/15 text-gold border border-gold/20"
            >
              Recommended
            </span>
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
              {{ formatBytes(model.sizeBytes) }}
            </span>
          </div>
        </div>

        <!-- Description -->
        <p class="text-[11px] text-ink-muted leading-snug">
          {{ model.description }}
        </p>

        <!-- Ratings -->
        <div class="flex gap-4 mt-0.5">
          <div class="flex items-center gap-1.5">
            <span class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint">
              Speed
            </span>
            <div class="flex gap-[3px]">
              <span
                v-for="i in 5"
                :key="i"
                class="w-[5px] h-[5px] rounded-full transition-colors duration-200"
                :class="i <= 6 - model.speedRating ? 'bg-gold' : 'bg-edge'"
              />
            </div>
          </div>
          <div class="flex items-center gap-1.5">
            <span class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint">
              Accuracy
            </span>
            <div class="flex gap-[3px]">
              <span
                v-for="i in 5"
                :key="i"
                class="w-[5px] h-[5px] rounded-full transition-colors duration-200"
                :class="i <= model.accuracyRating ? 'bg-gold' : 'bg-edge'"
              />
            </div>
          </div>
        </div>
      </button>
    </div>

    <!-- Error -->
    <div v-if="modelError" class="p-2.5 rounded-lg bg-flame/10 border border-flame/20 mb-3">
      <span class="text-[10px] text-flame">{{ modelError }}</span>
    </div>

    <!-- Download Progress -->
    <div
      v-if="downloading && downloadingModelId === selectedModelId"
      class="flex items-center gap-3"
    >
      <div class="flex-1 h-1.5 bg-raised shadow-well rounded-full overflow-hidden">
        <div
          class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full transition-[width] duration-300 ease-out"
          :style="{ width: `${downloadProgress}%` }"
        />
      </div>
      <span class="text-[10px] text-ink-muted font-medium tabular-nums min-w-[36px] text-right">
        {{ downloadProgress.toFixed(0) }}%
      </span>
      <button
        type="button"
        aria-label="Cancel download"
        title="Cancel download"
        class="flex items-center justify-center w-[18px] h-[18px] rounded-full bg-raised border border-edge text-ink-faint transition-colors duration-150 hover:bg-panel hover:text-ink hover:border-edge-strong active:scale-95"
        @click="handleCancelDownload"
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

    <!-- Action Button -->
    <button
      v-else
      class="w-full py-2.5 rounded-lg text-[13px] font-semibold transition-all duration-200 active:scale-[0.97] bg-gradient-to-b from-gold to-gold-hover text-gold-ink hover:from-gold-hover hover:to-gold-deep shadow-press hover:shadow-lifted"
      :disabled="!selectedModelId || downloading"
      @click="handleDownloadAndContinue"
    >
      {{
        selectedModelId &&
        models.find((m) => m.id === selectedModelId) &&
        isDownloaded(models.find((m) => m.id === selectedModelId)!)
          ? 'Continue'
          : 'Download & Continue'
      }}
    </button>
  </div>
</template>
