<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useSettings } from '@/composables/useSettings';
import {
  getDownloadedModels,
  downloadModel,
  selectModel,
  deleteModelFile,
  cancelDownload,
  type ModelInfo,
} from '@/lib/commands';
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from '@/lib/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

const { settings, models, currentModel, updateSelectedModel } = useSettings();

const emit = defineEmits<{
  modelChanged: [];
}>();

// ── Download state ──
const downloadedFiles = ref<string[]>([]);
const downloading = ref(false);
const downloadProgress = ref(0);
const downloadingModelId = ref<string | null>(null);
const deletingModelId = ref<string | null>(null);
const modelError = ref<string | null>(null);
const confirmDelete = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function isDownloaded(model: ModelInfo): boolean {
  return downloadedFiles.value.includes(model.filename);
}

function isActive(model: ModelInfo): boolean {
  return settings.value?.selectedModel === model.id;
}

const sortedModels = computed(() => [...models.value].sort((a, b) => a.sizeBytes - b.sizeBytes));

const downloadedModels = computed(() => sortedModels.value.filter((m) => isDownloaded(m)));

const availableModels = computed(() => sortedModels.value.filter((m) => !isDownloaded(m)));

async function handleSelectModel(model: ModelInfo) {
  if (!isDownloaded(model)) return;
  modelError.value = null;
  try {
    await selectModel(model.id);
    await updateSelectedModel(model.id);
    emit('modelChanged');
  } catch (e) {
    modelError.value = `Failed to load: ${e}`;
  }
}

async function handleDownload(model: ModelInfo) {
  modelError.value = null;
  downloading.value = true;
  downloadProgress.value = 0;
  downloadingModelId.value = model.id;
  try {
    await downloadModel(model.id);
    downloadedFiles.value = await getDownloadedModels();
    // Auto-select after download
    await selectModel(model.id);
    await updateSelectedModel(model.id);
    emit('modelChanged');
  } catch (e) {
    // Suppress cancel-as-rejection — the cancelled listener resets state.
    if (!String(e).toLowerCase().includes('cancel')) {
      modelError.value = `Download failed: ${e}`;
    }
  } finally {
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

async function handleDelete(model: ModelInfo) {
  if (confirmDelete.value !== model.id) {
    confirmDelete.value = model.id;
    setTimeout(() => {
      if (confirmDelete.value === model.id) confirmDelete.value = null;
    }, 3000);
    return;
  }
  deletingModelId.value = model.id;
  confirmDelete.value = null;
  try {
    await deleteModelFile(model.id);
    downloadedFiles.value = await getDownloadedModels();
    if (isActive(model)) emit('modelChanged');
  } catch (e) {
    modelError.value = `Delete failed: ${e}`;
  } finally {
    deletingModelId.value = null;
  }
}

onMounted(async () => {
  downloadedFiles.value = await getDownloadedModels();

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      if (downloadingModelId.value === progress.modelId) {
        downloadProgress.value = progress.percent;
      }
    }),
  );
  unlisteners.push(
    await onModelDownloadComplete(async () => {
      if (!downloadingModelId.value) return;
      downloading.value = false;
      downloadingModelId.value = null;
      downloadedFiles.value = await getDownloadedModels();
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
          <path
            d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"
          />
          <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
          <line x1="12" y1="22.08" x2="12" y2="12" />
        </svg>
      </div>
      <span class="section-label">Model</span>
    </div>

    <!-- Active model card -->
    <div v-if="currentModel" class="p-3 rounded-lg bg-gold/[0.04] border border-gold/20 mb-3">
      <div class="flex items-center justify-between mb-1.5">
        <div class="flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full bg-leaf shadow-[0_0_4px_rgba(95,183,96,0.5)]" />
          <span class="text-[12px] font-semibold text-ink">
            {{ currentModel.displayName }}
          </span>
        </div>
        <span class="text-[10px] text-ink-faint font-medium tabular-nums">
          {{ formatBytes(currentModel.sizeBytes) }}
        </span>
      </div>
      <div class="flex gap-4">
        <div class="flex items-center gap-1.5">
          <span class="rating-label">Speed</span>
          <div class="flex gap-[3px]">
            <span
              v-for="i in 5"
              :key="'sp-' + i"
              class="w-[4px] h-[4px] rounded-full"
              :class="i <= 6 - currentModel.speedRating ? 'bg-gold' : 'bg-edge'"
            />
          </div>
        </div>
        <div class="flex items-center gap-1.5">
          <span class="rating-label">Accuracy</span>
          <div class="flex gap-[3px]">
            <span
              v-for="i in 5"
              :key="'ac-' + i"
              class="w-[4px] h-[4px] rounded-full"
              :class="i <= currentModel.accuracyRating ? 'bg-gold' : 'bg-edge'"
            />
          </div>
        </div>
      </div>
    </div>
    <div v-else class="p-3 rounded-lg bg-flame/[0.04] border border-flame/15 mb-3">
      <span class="text-[12px] text-flame font-medium"> No model selected </span>
    </div>

    <!-- Downloaded models -->
    <div v-if="downloadedModels.length > 0" class="mb-3">
      <span class="subsection-label">Downloaded</span>
      <div class="flex flex-col gap-1.5 mt-1.5">
        <div
          v-for="model in downloadedModels"
          :key="model.id"
          class="model-row group"
          :class="{
            'bg-gold/[0.03] border-gold/15': isActive(model),
            'bg-panel border-edge hover:border-edge-strong hover:bg-raised': !isActive(model),
          }"
        >
          <button
            class="flex-1 flex items-center gap-2.5 min-w-0 text-left"
            @click="handleSelectModel(model)"
          >
            <!-- Radio indicator -->
            <div
              class="w-3.5 h-3.5 rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 transition-all duration-200"
              :class="
                isActive(model)
                  ? 'border-gold bg-gold/10'
                  : 'border-edge-strong group-hover:border-ink-faint'
              "
            >
              <div v-if="isActive(model)" class="w-1.5 h-1.5 rounded-full bg-gold" />
            </div>
            <div class="flex flex-col min-w-0">
              <span
                class="text-[12px] font-semibold truncate"
                :class="isActive(model) ? 'text-ink' : 'text-ink-muted'"
              >
                {{ model.displayName }}
              </span>
              <span class="text-[10px] text-ink-faint tabular-nums">
                {{ formatBytes(model.sizeBytes) }}
                <template v-if="model.englishOnly"> · English</template>
                <template v-else> · Multilingual</template>
              </span>
            </div>
          </button>
          <!-- Delete button -->
          <button
            v-if="!isActive(model)"
            class="flex-shrink-0 p-1 rounded-md opacity-0 group-hover:opacity-100 transition-all duration-150"
            :class="
              confirmDelete === model.id
                ? 'bg-flame/15 text-flame opacity-100'
                : 'text-ink-faint hover:text-flame hover:bg-flame/10'
            "
            @click.stop="handleDelete(model)"
          >
            <svg
              v-if="confirmDelete !== model.id"
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
              <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
            </svg>
            <span v-else class="text-[9px] font-bold uppercase tracking-wider px-0.5">
              Delete?
            </span>
          </button>
        </div>
      </div>
    </div>

    <!-- Download progress -->
    <div v-if="downloading" class="mb-3">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[11px] text-ink-muted font-medium"> Downloading… </span>
        <div class="flex items-center gap-2">
          <span class="text-[11px] text-ink-faint tabular-nums">
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
      </div>
      <div class="h-1 bg-raised shadow-well rounded-full overflow-hidden">
        <div
          class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full transition-[width] duration-300 ease-out"
          :style="{ width: `${downloadProgress}%` }"
        />
      </div>
    </div>

    <!-- Available to download -->
    <div v-if="availableModels.length > 0">
      <span class="subsection-label">Available to download</span>
      <div class="flex flex-col gap-1.5 mt-1.5">
        <div
          v-for="model in availableModels"
          :key="model.id"
          class="model-row bg-panel border-edge"
        >
          <div class="flex-1 flex flex-col min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-[12px] font-semibold text-ink-muted truncate">
                {{ model.displayName }}
              </span>
              <span v-if="model.englishOnly" class="model-badge"> EN </span>
              <span v-else class="model-badge model-badge-multi"> Multi </span>
            </div>
            <div class="flex items-center gap-3 mt-0.5">
              <span class="text-[10px] text-ink-faint tabular-nums">
                {{ formatBytes(model.sizeBytes) }}
              </span>
              <div class="flex items-center gap-1">
                <span class="rating-label">Spd</span>
                <div class="flex gap-[2px]">
                  <span
                    v-for="i in 5"
                    :key="'dsp-' + model.id + i"
                    class="w-[3px] h-[3px] rounded-full"
                    :class="i <= 6 - model.speedRating ? 'bg-gold/70' : 'bg-edge'"
                  />
                </div>
              </div>
              <div class="flex items-center gap-1">
                <span class="rating-label">Acc</span>
                <div class="flex gap-[2px]">
                  <span
                    v-for="i in 5"
                    :key="'dac-' + model.id + i"
                    class="w-[3px] h-[3px] rounded-full"
                    :class="i <= model.accuracyRating ? 'bg-gold/70' : 'bg-edge'"
                  />
                </div>
              </div>
            </div>
          </div>
          <button
            class="flex-shrink-0 flex items-center gap-1 px-2 py-1 rounded-md bg-raised border border-edge text-[10px] font-semibold text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95"
            :disabled="downloading"
            @click="handleDownload(model)"
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

    <!-- Model error -->
    <div v-if="modelError" class="mt-2 p-2 rounded-md bg-flame/10 border border-flame/20">
      <span class="text-[11px] text-flame">{{ modelError }}</span>
    </div>
  </section>
</template>
