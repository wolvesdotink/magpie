<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useSettings } from '@/composables/useSettings';
import { useModelDownloader } from '@/composables/useModelDownloader';
import { useConfirmAction } from '@/composables/useConfirmAction';
import SettingsSection from '@/components/base/SettingsSection.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import Badge from '@/components/base/Badge.vue';
import ProgressBar from '@/components/base/ProgressBar.vue';
import RatingDots from '@/components/base/RatingDots.vue';
import { getSystemMemory, type ModelInfo } from '@/lib/commands';

const { settings, models, currentModel, updateSelectedModel, updateMemorySaver } = useSettings();
const downloader = useModelDownloader('whisper');
const confirmDelete = useConfirmAction();

const emit = defineEmits<{
  modelChanged: [];
}>();

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

const GB = 1024 ** 3;

// Total physical RAM, fetched once. 0 means "unknown" → we skip RAM warnings.
const systemMemoryBytes = ref(0);
onMounted(async () => {
  try {
    systemMemoryBytes.value = await getSystemMemory();
  } catch (e) {
    console.error('Failed to read system memory:', e);
  }
});

// Resident memory ≈ the GGML weights (≈ file size) plus a modest allowance
// for whisper's KV/compute and Metal scratch buffers. Deliberately rough —
// always shown with a "≈".
function ramEstimateBytes(sizeBytes: number): number {
  return Math.round(sizeBytes * 1.15);
}

// 8 GB-class Macs are where the footprint actually pinches. The 8.5 GB cutoff
// catches machines that report a hair under 8 GiB.
const lowRam = computed(() => systemMemoryBytes.value > 0 && systemMemoryBytes.value <= 8.5 * GB);

// Warn when the active model is heavy relative to the machine: an absolute
// ~0.9 GB+ on low-RAM Macs, or >25% of RAM on larger ones.
const showHeavyWarning = computed(() => {
  const m = currentModel.value;
  if (!m || systemMemoryBytes.value <= 0) return false;
  const est = ramEstimateBytes(m.sizeBytes);
  return lowRam.value ? est >= 0.9 * GB : est > 0.25 * systemMemoryBytes.value;
});

const heavyWarning = computed(() => {
  const m = currentModel.value;
  if (!m) return '';
  const est = ramEstimateBytes(m.sizeBytes);
  const pct = systemMemoryBytes.value > 0 ? Math.round((est / systemMemoryBytes.value) * 100) : 0;
  const ofRam =
    systemMemoryBytes.value > 0
      ? ` (~${pct}% of your ${formatBytes(systemMemoryBytes.value)})`
      : '';
  return (
    `${m.displayName} uses about ${formatBytes(est)} of memory${ofRam}. ` +
    'A smaller model (Small or Distil Small) cuts that sharply, or turn on ' +
    'Memory Saver below to free it when idle.'
  );
});

function isDownloaded(model: ModelInfo): boolean {
  return downloader.isDownloaded(model.filename);
}

function isActive(model: ModelInfo): boolean {
  return settings.value?.selectedModel === model.id;
}

const sortedModels = computed(() => [...models.value].sort((a, b) => a.sizeBytes - b.sizeBytes));
const downloadedModels = computed(() => sortedModels.value.filter((m) => isDownloaded(m)));
const availableModels = computed(() => sortedModels.value.filter((m) => !isDownloaded(m)));

async function handleSelectModel(model: ModelInfo) {
  if (!isDownloaded(model)) return;
  await downloader.select(model.id);
  if (!downloader.error.value) {
    await updateSelectedModel(model.id);
    emit('modelChanged');
  }
}

async function handleDownload(model: ModelInfo) {
  await downloader.download(model.id, async () => {
    await updateSelectedModel(model.id);
    emit('modelChanged');
  });
}

async function handleDelete(model: ModelInfo) {
  if (!confirmDelete.confirm(model.id)) return;
  await downloader.remove(model.id);
  if (!downloader.error.value && isActive(model)) emit('modelChanged');
}
</script>

<template>
  <SettingsSection label="Model">
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
        <path
          d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z"
        />
        <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
        <line x1="12" y1="22.08" x2="12" y2="12" />
      </svg>
    </template>

    <BaseCard v-if="currentModel" tone="gold" padding="lg" class="mb-3">
      <div class="flex items-center justify-between mb-1.5">
        <div class="flex items-center gap-2">
          <div class="w-1.5 h-1.5 rounded-full bg-leaf shadow-[0_0_4px_rgba(95,183,96,0.5)]" />
          <span class="text-[12px] font-semibold text-ink">
            {{ currentModel.displayName }}
          </span>
        </div>
        <span class="text-[10px] text-ink-faint font-medium tabular-nums">
          ≈{{ formatBytes(ramEstimateBytes(currentModel.sizeBytes)) }} in memory
        </span>
      </div>
      <div class="flex gap-4">
        <div class="flex items-center gap-1.5">
          <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
            Speed
          </span>
          <RatingDots :value="currentModel.speedRating" reverse size="sm" />
        </div>
        <div class="flex items-center gap-1.5">
          <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
            Accuracy
          </span>
          <RatingDots :value="currentModel.accuracyRating" size="sm" />
        </div>
      </div>
    </BaseCard>
    <BaseCard v-else tone="flame" padding="lg" class="mb-3">
      <span class="text-[12px] text-flame font-medium">No model selected</span>
    </BaseCard>

    <BaseCard v-if="showHeavyWarning" tone="gold" padding="sm" class="mb-3">
      <div class="flex items-start gap-2">
        <svg
          class="flex-shrink-0 mt-[1px]"
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
            d="M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z"
          />
          <line x1="12" y1="9" x2="12" y2="13" />
          <line x1="12" y1="17" x2="12.01" y2="17" />
        </svg>
        <span class="text-[10px] text-gold leading-snug">{{ heavyWarning }}</span>
      </div>
    </BaseCard>

    <SettingsRow
      label="Memory Saver"
      helper="Unload the model when idle and reload it on your next dictation — big memory savings, with a brief load the first time you dictate after a pause."
    >
      <BaseToggle
        :model-value="!!settings?.memorySaver"
        @update:model-value="updateMemorySaver($event)"
      />
    </SettingsRow>

    <div v-if="downloadedModels.length > 0" class="mb-3 mt-3">
      <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">Downloaded</span>
      <div class="flex flex-col gap-1.5 mt-1.5">
        <div
          v-for="model in downloadedModels"
          :key="model.id"
          class="group flex items-center gap-2 px-2.5 py-2 rounded-lg border transition-all duration-150"
          :class="{
            'bg-gold/[0.03] border-gold/15': isActive(model),
            'bg-panel border-edge hover:border-edge-strong hover:bg-raised': !isActive(model),
          }"
        >
          <button
            class="flex-1 flex items-center gap-2.5 min-w-0 text-left"
            @click="handleSelectModel(model)"
          >
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
                ≈{{ formatBytes(ramEstimateBytes(model.sizeBytes)) }} RAM
                <template v-if="model.englishOnly"> · English</template>
                <template v-else> · Multilingual</template>
              </span>
            </div>
          </button>
          <button
            v-if="!isActive(model)"
            class="flex-shrink-0 p-1 rounded-md opacity-0 group-hover:opacity-100 transition-all duration-150"
            :class="
              confirmDelete.isArmed(model.id)
                ? 'bg-flame/15 text-flame opacity-100'
                : 'text-ink-faint hover:text-flame hover:bg-flame/10'
            "
            @click.stop="handleDelete(model)"
          >
            <svg
              v-if="!confirmDelete.isArmed(model.id)"
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

    <div v-if="downloader.downloading.value" class="mb-3">
      <div class="flex items-center justify-between mb-1.5">
        <span class="text-[11px] text-ink-muted font-medium">Downloading…</span>
        <div class="flex items-center gap-2">
          <span class="text-[11px] text-ink-faint tabular-nums">
            {{ downloader.progress.value.toFixed(0) }}%
          </span>
          <button
            type="button"
            aria-label="Cancel download"
            title="Cancel download"
            class="flex items-center justify-center w-[18px] h-[18px] rounded-full bg-raised border border-edge text-ink-faint transition-colors duration-150 hover:bg-panel hover:text-ink hover:border-edge-strong active:scale-95"
            @click="downloader.cancel"
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
      <ProgressBar :value="downloader.progress.value" />
    </div>

    <div v-if="availableModels.length > 0">
      <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
        Available to download
      </span>
      <div class="flex flex-col gap-1.5 mt-1.5">
        <div
          v-for="model in availableModels"
          :key="model.id"
          class="flex items-center gap-2 px-2.5 py-2 rounded-lg border bg-panel border-edge transition-all duration-150"
        >
          <div class="flex-1 flex flex-col min-w-0">
            <div class="flex items-center gap-2">
              <span class="text-[12px] font-semibold text-ink-muted truncate">
                {{ model.displayName }}
              </span>
              <Badge :tone="model.englishOnly ? 'neutral' : 'gold'" class="flex-shrink-0">
                {{ model.englishOnly ? 'EN' : 'Multi' }}
              </Badge>
            </div>
            <div class="flex items-center gap-3 mt-0.5">
              <span class="text-[10px] text-ink-faint tabular-nums">
                {{ formatBytes(model.sizeBytes) }} · ≈{{
                  formatBytes(ramEstimateBytes(model.sizeBytes))
                }}
                RAM
              </span>
              <div class="flex items-center gap-1">
                <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
                  Spd
                </span>
                <RatingDots :value="model.speedRating" reverse intensity="soft" />
              </div>
              <div class="flex items-center gap-1">
                <span class="text-[8px] uppercase tracking-[0.06em] font-semibold text-ink-faint">
                  Acc
                </span>
                <RatingDots :value="model.accuracyRating" intensity="soft" />
              </div>
            </div>
          </div>
          <button
            class="flex-shrink-0 flex items-center gap-1 px-2 py-1 rounded-md bg-raised border border-edge text-[10px] font-semibold text-ink-muted hover:bg-hover hover:text-ink hover:border-edge-strong transition-all duration-150 active:scale-95"
            :disabled="downloader.downloading.value"
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

    <BaseCard v-if="downloader.error.value" tone="flame" padding="sm" class="mt-2">
      <span class="text-[11px] text-flame">{{ downloader.error.value }}</span>
    </BaseCard>
  </SettingsSection>
</template>
