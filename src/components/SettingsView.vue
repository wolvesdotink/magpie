<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import { useSettings } from "@/composables/useSettings";
import { LANGUAGES } from "@/lib/languages";
import {
  getDownloadedModels,
  getDownloadedCorrectionModels,
  downloadModel,
  downloadCorrectionModel,
  cancelDownload,
  selectModel,
  selectCorrectionModel,
  deleteModelFile,
  deleteCorrectionModelFile,
  getVocabulary,
  addVocabularyEntry,
  removeVocabularyEntry,
  clearVocabulary,
  type ModelInfo,
  type CorrectionModelInfo,
  type VocabularyEntry,
} from "@/lib/commands";
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from "@/lib/events";
import type { UnlistenFn } from "@tauri-apps/api/event";
import UpdatesSection from "@/components/UpdatesSection.vue";

withDefaults(
  defineProps<{
    standalone?: boolean;
  }>(),
  { standalone: false },
);

const emit = defineEmits<{
  back: [];
  modelChanged: [];
}>();

const {
  settings,
  models,
  correctionModels,
  currentLanguageCode,
  isEnglishOnlyModel,
  currentModel,
  launchAtLoginStatus,
  openLoginItemsSettings,
  updateLanguage,
  updateActivationMode,
  updateCustomShortcut,
  updateAutoStart,
  updateRemoveFillers,
  updateSelfCorrection,
  updateVocabularyLearning,
  updateStreamingPreview,
  updateSelectedCorrectionModel,
  updateSelectedModel,
} = useSettings();

// ── Model management state ──
const downloadedFiles = ref<string[]>([]);
const downloading = ref(false);
const downloadProgress = ref(0);
const downloadingModelId = ref<string | null>(null);
const deletingModelId = ref<string | null>(null);
const modelError = ref<string | null>(null);

// ── Correction model management state ──
const downloadedCorrectionFiles = ref<string[]>([]);
const downloadingCorrection = ref(false);
const correctionDownloadProgress = ref(0);
const downloadingCorrectionModelId = ref<string | null>(null);
const correctionModelError = ref<string | null>(null);
const confirmDeleteCorrection = ref<string | null>(null);

// ── Vocabulary state ──
const vocabularyEntries = ref<VocabularyEntry[]>([]);
const showAddVocab = ref(false);
const vocabWrong = ref("");
const vocabCorrect = ref("");
const confirmClearVocab = ref(false);

// ── UI state ──
const languageOpen = ref(false);
const languageRef = ref<HTMLElement | null>(null);
const confirmDelete = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

const explicitLanguages = LANGUAGES.slice(1);

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

const sortedModels = computed(() =>
  [...models.value].sort((a, b) => a.sizeBytes - b.sizeBytes),
);

const downloadedModels = computed(() =>
  sortedModels.value.filter((m) => isDownloaded(m)),
);

const availableModels = computed(() =>
  sortedModels.value.filter((m) => !isDownloaded(m)),
);

async function handleSelectModel(model: ModelInfo) {
  if (!isDownloaded(model)) return;
  modelError.value = null;
  try {
    await selectModel(model.id);
    await updateSelectedModel(model.id);
    emit("modelChanged");
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
    emit("modelChanged");
  } catch (e) {
    // Suppress cancel-as-rejection — the cancelled listener resets state.
    if (!String(e).toLowerCase().includes("cancel")) {
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
    console.error("Cancel failed:", e);
  }
}

async function handleDelete(model: ModelInfo) {
  if (confirmDelete.value !== model.id) {
    confirmDelete.value = model.id;
    return;
  }
  deletingModelId.value = model.id;
  confirmDelete.value = null;
  try {
    await deleteModelFile(model.id);
    downloadedFiles.value = await getDownloadedModels();
    // If we deleted the active model, clear selection
    if (isActive(model)) {
      emit("modelChanged");
    }
  } catch (e) {
    modelError.value = `Delete failed: ${e}`;
  } finally {
    deletingModelId.value = null;
  }
}

// ── Correction model helpers ──

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
    // Auto-select after download
    await selectCorrectionModel(model.id);
    await updateSelectedCorrectionModel(model.id);
  } catch (e) {
    if (!String(e).toLowerCase().includes("cancel")) {
      correctionModelError.value = `Download failed: ${e}`;
    }
  } finally {
    downloadingCorrection.value = false;
    downloadingCorrectionModelId.value = null;
    // Always refresh the list — the file may have been cleaned up on failure
    downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();
  }
}

async function handleCancelCorrectionDownload() {
  if (!downloadingCorrectionModelId.value) return;
  try {
    await cancelDownload(downloadingCorrectionModelId.value);
  } catch (e) {
    console.error("Cancel failed:", e);
  }
}

async function handleDeleteCorrection(model: CorrectionModelInfo) {
  if (confirmDeleteCorrection.value !== model.id) {
    confirmDeleteCorrection.value = model.id;
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

// ── Vocabulary helpers ──

async function loadVocabulary() {
  try {
    vocabularyEntries.value = await getVocabulary();
  } catch (e) {
    console.error("Failed to load vocabulary:", e);
  }
}

async function handleAddVocab() {
  const wrong = vocabWrong.value.trim();
  const correct = vocabCorrect.value.trim();
  if (!wrong || !correct || wrong === correct) return;
  try {
    await addVocabularyEntry(wrong, correct);
    vocabWrong.value = "";
    vocabCorrect.value = "";
    showAddVocab.value = false;
    await loadVocabulary();
  } catch (e) {
    console.error("Failed to add vocabulary entry:", e);
  }
}

async function handleRemoveVocab(wrong: string) {
  try {
    await removeVocabularyEntry(wrong);
    await loadVocabulary();
  } catch (e) {
    console.error("Failed to remove vocabulary entry:", e);
  }
}

async function handleClearVocab() {
  if (!confirmClearVocab.value) {
    confirmClearVocab.value = true;
    setTimeout(() => {
      confirmClearVocab.value = false;
    }, 3000);
    return;
  }
  confirmClearVocab.value = false;
  try {
    await clearVocabulary();
    await loadVocabulary();
  } catch (e) {
    console.error("Failed to clear vocabulary:", e);
  }
}

function selectLanguage(code: string) {
  updateLanguage(code);
  languageOpen.value = false;
}

function onClickOutsideLang(e: MouseEvent) {
  if (languageRef.value && !languageRef.value.contains(e.target as Node)) {
    languageOpen.value = false;
  }
}

// Clear confirm-delete when clicking elsewhere
watch(confirmDelete, (val) => {
  if (val) {
    setTimeout(() => {
      confirmDelete.value = null;
    }, 3000);
  }
});

watch(confirmDeleteCorrection, (val) => {
  if (val) {
    setTimeout(() => {
      confirmDeleteCorrection.value = null;
    }, 3000);
  }
});

onMounted(async () => {
  downloadedFiles.value = await getDownloadedModels();
  downloadedCorrectionFiles.value = await getDownloadedCorrectionModels();
  await loadVocabulary();

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      // Route progress to the correct download tracker
      if (downloadingCorrectionModelId.value === progress.modelId) {
        correctionDownloadProgress.value = progress.percent;
      } else {
        downloadProgress.value = progress.percent;
      }
    }),
  );
  unlisteners.push(
    await onModelDownloadComplete(async () => {
      downloading.value = false;
      downloadingModelId.value = null;
      downloadedFiles.value = await getDownloadedModels();
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
      } else {
        downloading.value = false;
        downloadingModelId.value = null;
        downloadProgress.value = 0;
        modelError.value = null;
      }
    }),
  );

  document.addEventListener("mousedown", onClickOutsideLang);
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
  document.removeEventListener("mousedown", onClickOutsideLang);
  stopCapture();
});

const activationModes = [
  {
    id: "holdFn" as const,
    label: "Hold Fn",
    desc: "Hold to record, release to stop",
    icon: "M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h8",
  },
  {
    id: "tapFn" as const,
    label: "Tap Fn",
    desc: "Press Fn to start, press Fn again to stop (also fires on Fn shortcuts)",
    icon: "M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h5",
  },
  {
    id: "doubleTapFn" as const,
    label: "Double-tap Fn",
    desc: "Tap twice to start, once to stop",
    icon: "M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h3m5 0h3",
  },
  {
    id: "shortcut" as const,
    label: "Keyboard shortcut",
    desc: "Use a custom key combination",
    icon: "M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm4 8l3 3 5-6",
  },
];

// ── Global shortcut capture ────────────────────────────────────────

const DEFAULT_SHORTCUT_DISPLAY = "⌘⇧Space";

const capturing = ref(false);
const shortcutError = ref<string | null>(null);

/** Map a DOM KeyboardEvent.key/code into Tauri's shortcut-string key segment. */
function keyToTauri(e: KeyboardEvent): string | null {
  // Modifier-only events have e.key === "Meta"/"Control"/"Alt"/"Shift" — caller filters these.
  const key = e.key;

  // Whitespace + special keys
  if (key === " ") return "Space";
  if (key === "Escape") return "Escape";
  if (key === "Tab") return "Tab";
  if (key === "Enter") return "Enter";
  if (key === "Backspace") return "Backspace";
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  if (key === "PageUp") return "PageUp";
  if (key === "PageDown") return "PageDown";
  if (key === "Home") return "Home";
  if (key === "End") return "End";

  // Function keys (F1–F24) come through as-is, but normalize casing.
  if (/^F\d{1,2}$/.test(key)) return key;

  // Letters: uppercase. Digits: as-is. Punctuation: pass through.
  if (key.length === 1) return key.toUpperCase();

  // Fall back to e.code for less-common keys
  if (e.code && e.code.length > 0) return e.code;

  return null;
}

/** Build a Tauri shortcut string from a keydown event. Returns null if it
 *  doesn't include a non-modifier key. */
function buildShortcutString(e: KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push("CmdOrCtrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");

  const key = keyToTauri(e);
  if (!key) return null;
  return [...mods, key].join("+");
}

/** Pretty-print a Tauri shortcut string with macOS glyphs (⌘⇧⌃⌥). */
function formatShortcut(s: string | null | undefined): string {
  if (!s) return DEFAULT_SHORTCUT_DISPLAY;
  return s
    .split("+")
    .map((part) => {
      switch (part) {
        case "CmdOrCtrl":
        case "Cmd":
        case "Command":
        case "Meta":
          return "⌘";
        case "Ctrl":
        case "Control":
          return "⌃";
        case "Alt":
        case "Option":
          return "⌥";
        case "Shift":
          return "⇧";
        default:
          return part;
      }
    })
    .join("");
}

const displayedShortcut = computed(() =>
  formatShortcut(settings.value?.customShortcut ?? null),
);

let captureHandler: ((e: KeyboardEvent) => void) | null = null;

function stopCapture() {
  capturing.value = false;
  if (captureHandler) {
    window.removeEventListener("keydown", captureHandler, true);
    captureHandler = null;
  }
}

function startCapture() {
  if (capturing.value) {
    stopCapture();
    return;
  }
  shortcutError.value = null;
  capturing.value = true;

  captureHandler = (e: KeyboardEvent) => {
    // Capture-phase listener with stopPropagation prevents the keystroke
    // from landing in any focused input or triggering app shortcuts.
    e.preventDefault();
    e.stopPropagation();

    // Modifier-only presses: keep listening until the user adds a real key.
    if (
      e.key === "Meta" ||
      e.key === "Control" ||
      e.key === "Alt" ||
      e.key === "Shift"
    ) {
      return;
    }

    // Escape cancels capture without changing the shortcut.
    if (e.key === "Escape") {
      stopCapture();
      return;
    }

    const built = buildShortcutString(e);
    if (!built) {
      shortcutError.value = "Could not interpret that key.";
      return;
    }

    // Require at least one modifier so the user can't bind a single
    // letter (which would steal keystrokes globally).
    const hasModifier = /(CmdOrCtrl|Alt|Shift)/.test(built);
    if (!hasModifier) {
      shortcutError.value = "Must include a modifier (⌘, ⌃, ⌥ or ⇧).";
      return;
    }

    stopCapture();
    updateCustomShortcut(built)
      .then(() => {
        shortcutError.value = null;
      })
      .catch((err) => {
        shortcutError.value = String(err);
      });
  };

  window.addEventListener("keydown", captureHandler, true);
}

function resetShortcut() {
  shortcutError.value = null;
  updateCustomShortcut(null).catch((err) => {
    shortcutError.value = String(err);
  });
}

const currentLanguageLabel = computed(() => {
  if (currentLanguageCode.value === "auto") return "Auto-detect";
  const lang = LANGUAGES.find((l) => l.code === currentLanguageCode.value);
  return lang ? lang.nativeName : currentLanguageCode.value.toUpperCase();
});
</script>

<template>
  <div
    class="flex flex-col overflow-hidden"
    :class="standalone ? 'flex-1 min-h-0' : 'h-full bg-canvas rounded-xl relative surface-grain'"
  >
    <!-- Decorative top edge (embedded only) -->
    <div
      v-if="!standalone"
      class="h-[1.5px] bg-gradient-to-r from-transparent via-ink-faint/20 to-transparent"
    />

    <!-- ── Header (embedded only) ── -->
    <div v-if="!standalone" class="flex items-center gap-3 px-5 pt-5 pb-3">
      <button
        class="flex items-center justify-center w-7 h-7 rounded-lg
               bg-raised border border-edge
               text-ink-faint hover:text-ink hover:border-edge-strong hover:bg-hover
               transition-all duration-150 active:scale-95"
        @click="$emit('back')"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
      </button>
      <h1 class="text-[15px] font-bold tracking-tight text-ink">Settings</h1>
    </div>

    <!-- ── Scrollable Content ── -->
    <div class="flex-1 overflow-y-auto min-h-0 px-5 pt-4 pb-5">
      <div class="flex flex-col gap-5">
        <!-- ═══════════════ MODEL SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 0ms">
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
                <path d="M21 16V8a2 2 0 00-1-1.73l-7-4a2 2 0 00-2 0l-7 4A2 2 0 003 8v8a2 2 0 001 1.73l7 4a2 2 0 002 0l7-4A2 2 0 0021 16z" />
                <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
                <line x1="12" y1="22.08" x2="12" y2="12" />
              </svg>
            </div>
            <span class="section-label">Model</span>
          </div>

          <!-- Active model card -->
          <div
            v-if="currentModel"
            class="p-3 rounded-lg bg-gold/[0.04] border border-gold/20 mb-3"
          >
            <div class="flex items-center justify-between mb-1.5">
              <div class="flex items-center gap-2">
                <div
                  class="w-1.5 h-1.5 rounded-full bg-leaf shadow-[0_0_4px_rgba(95,183,96,0.5)]"
                />
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
          <div
            v-else
            class="p-3 rounded-lg bg-flame/[0.04] border border-flame/15 mb-3"
          >
            <span class="text-[12px] text-flame font-medium">
              No model selected
            </span>
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
                    <div
                      v-if="isActive(model)"
                      class="w-1.5 h-1.5 rounded-full bg-gold"
                    />
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
                  class="flex-shrink-0 p-1 rounded-md opacity-0 group-hover:opacity-100
                         transition-all duration-150"
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
                  <span
                    v-else
                    class="text-[9px] font-bold uppercase tracking-wider px-0.5"
                  >
                    Delete?
                  </span>
                </button>
              </div>
            </div>
          </div>

          <!-- Download progress -->
          <div
            v-if="downloading"
            class="mb-3"
          >
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-[11px] text-ink-muted font-medium">
                Downloading…
              </span>
              <div class="flex items-center gap-2">
                <span class="text-[11px] text-ink-faint tabular-nums">
                  {{ downloadProgress.toFixed(0) }}%
                </span>
                <button
                  type="button"
                  aria-label="Cancel download"
                  title="Cancel download"
                  class="flex items-center justify-center w-[18px] h-[18px] rounded-full
                         bg-raised border border-edge text-ink-faint
                         transition-colors duration-150
                         hover:bg-panel hover:text-ink hover:border-edge-strong
                         active:scale-95"
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
                class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full
                       transition-[width] duration-300 ease-out"
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
                    <span
                      v-if="model.englishOnly"
                      class="model-badge"
                    >
                      EN
                    </span>
                    <span v-else class="model-badge model-badge-multi">
                      Multi
                    </span>
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
                  class="flex-shrink-0 flex items-center gap-1 px-2 py-1 rounded-md
                         bg-raised border border-edge text-[10px] font-semibold text-ink-muted
                         hover:bg-hover hover:text-ink hover:border-edge-strong
                         transition-all duration-150 active:scale-95"
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
          <div
            v-if="modelError"
            class="mt-2 p-2 rounded-md bg-flame/10 border border-flame/20"
          >
            <span class="text-[11px] text-flame">{{ modelError }}</span>
          </div>
        </section>

        <!-- ═══════════════ LANGUAGE SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 40ms">
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
                <circle cx="12" cy="12" r="10" />
                <path d="M2 12h20" />
                <path d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z" />
              </svg>
            </div>
            <span class="section-label">Language</span>
          </div>

          <!-- English-only model hint -->
          <div
            v-if="isEnglishOnlyModel"
            class="p-2 rounded-md bg-gold/[0.06] border border-gold/15 mb-2.5"
          >
            <span class="text-[10px] text-gold leading-snug">
              Switch to a multilingual model to unlock other languages.
            </span>
          </div>

          <div ref="languageRef" class="relative">
            <button
              class="w-full flex items-center justify-between p-2.5 rounded-lg
                     bg-panel border transition-all duration-150"
              :class="
                languageOpen
                  ? 'border-gold/30 shadow-glow-gold'
                  : 'border-edge hover:border-edge-strong'
              "
              @click="languageOpen = !languageOpen"
            >
              <div class="flex items-center gap-2">
                <span class="text-[12px] font-semibold text-ink">
                  {{ currentLanguageLabel }}
                </span>
              </div>
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="text-ink-faint transition-transform duration-200"
                :class="{ 'rotate-180': languageOpen }"
              >
                <polyline points="6 9 12 15 18 9" />
              </svg>
            </button>

            <!-- Language dropdown -->
            <Transition
              enter-active-class="transition duration-150 ease-out"
              enter-from-class="opacity-0 -translate-y-1 scale-[0.98]"
              enter-to-class="opacity-100 translate-y-0 scale-100"
              leave-active-class="transition duration-100 ease-in"
              leave-from-class="opacity-100 translate-y-0 scale-100"
              leave-to-class="opacity-0 -translate-y-1 scale-[0.98]"
            >
              <div
                v-if="languageOpen"
                class="absolute top-full left-0 right-0 mt-1.5
                       bg-panel border border-edge rounded-lg shadow-elevated
                       overflow-hidden z-50"
              >
                <div class="max-h-[200px] overflow-y-auto py-1">
                  <!-- Auto-detect -->
                  <button
                    class="lang-option"
                    :class="
                      currentLanguageCode === 'auto'
                        ? 'bg-gold/[0.06] text-ink'
                        : 'text-ink-muted hover:bg-raised hover:text-ink'
                    "
                    @click="selectLanguage('auto')"
                  >
                    <span class="text-[11px] font-medium">Auto-detect</span>
                    <svg
                      v-if="currentLanguageCode === 'auto'"
                      width="10"
                      height="10"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="3"
                      class="text-gold"
                    >
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  </button>
                  <div class="h-px bg-edge mx-2 my-0.5" />
                  <button
                    v-for="lang in explicitLanguages"
                    :key="lang.code"
                    class="lang-option"
                    :class="[
                      currentLanguageCode === lang.code
                        ? 'bg-gold/[0.06] text-ink'
                        : 'hover:bg-raised',
                      isEnglishOnlyModel && lang.code !== 'en'
                        ? 'opacity-30 pointer-events-none'
                        : 'text-ink-muted hover:text-ink',
                    ]"
                    @click="selectLanguage(lang.code)"
                  >
                    <div class="flex items-baseline gap-1.5 min-w-0">
                      <span class="text-[11px] font-medium truncate">
                        {{ lang.nativeName }}
                      </span>
                      <span
                        v-if="lang.nativeName !== lang.name"
                        class="text-[9px] text-ink-faint truncate"
                      >
                        {{ lang.name }}
                      </span>
                    </div>
                    <svg
                      v-if="currentLanguageCode === lang.code"
                      width="10"
                      height="10"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="3"
                      class="text-gold flex-shrink-0"
                    >
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  </button>
                </div>
              </div>
            </Transition>
          </div>
        </section>

        <!-- ═══════════════ INPUT SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 80ms">
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
                <rect x="2" y="4" width="20" height="16" rx="2" />
                <path d="M6 8h.001M10 8h.001M14 8h.001M18 8h.001M8 12h.001M12 12h.001M16 12h.001M8 16h8" />
              </svg>
            </div>
            <span class="section-label">Activation</span>
          </div>

          <div class="flex flex-col gap-1.5">
            <button
              v-for="mode in activationModes"
              :key="mode.id"
              class="flex items-center gap-2.5 p-2.5 rounded-lg border
                     text-left transition-all duration-150"
              :class="
                settings?.activationMode === mode.id
                  ? 'bg-gold/[0.04] border-gold/20'
                  : 'bg-panel border-edge hover:border-edge-strong hover:bg-raised'
              "
              @click="updateActivationMode(mode.id)"
            >
              <div
                class="w-3.5 h-3.5 rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 transition-all"
                :class="
                  settings?.activationMode === mode.id
                    ? 'border-gold bg-gold/10'
                    : 'border-edge-strong'
                "
              >
                <div
                  v-if="settings?.activationMode === mode.id"
                  class="w-1.5 h-1.5 rounded-full bg-gold"
                />
              </div>
              <div class="flex flex-col min-w-0">
                <span
                  class="text-[12px] font-semibold"
                  :class="
                    settings?.activationMode === mode.id
                      ? 'text-ink'
                      : 'text-ink-muted'
                  "
                >
                  {{ mode.label }}
                </span>
                <span class="text-[10px] text-ink-faint leading-snug">
                  {{ mode.desc }}
                </span>
              </div>
            </button>
          </div>

          <!-- ── Custom hotkey capture (Shortcut mode only) ── -->
          <div
            v-if="settings?.activationMode === 'shortcut'"
            class="mt-2 p-2.5 rounded-lg bg-panel border border-edge flex flex-col gap-2"
          >
            <div class="flex items-center justify-between gap-3">
              <div class="flex flex-col min-w-0">
                <span class="text-[12px] font-semibold text-ink">
                  Global hotkey
                </span>
                <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
                  Click to capture. Must include a modifier
                  (⌘, ⌃, ⌥ or ⇧). Press Esc to cancel.
                </span>
              </div>
              <button
                class="px-2.5 py-1 rounded-md bg-raised border min-w-[110px]
                       text-[11px] font-semibold text-center transition"
                :class="
                  capturing
                    ? 'border-gold/40 text-gold'
                    : 'border-edge text-ink hover:border-edge-strong'
                "
                @click="startCapture"
              >
                {{ capturing ? "Press a key…" : displayedShortcut }}
              </button>
            </div>
            <button
              v-if="settings?.customShortcut"
              class="self-end text-[10px] text-ink-faint hover:text-ink transition-colors"
              @click="resetShortcut"
            >
              Reset to default ({{ DEFAULT_SHORTCUT_DISPLAY }})
            </button>
            <div v-if="shortcutError" class="text-[10px] text-flame">
              {{ shortcutError }}
            </div>
          </div>
        </section>

        <!-- ═══════════════ TRANSCRIPTION SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 120ms">
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
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
              </svg>
            </div>
            <span class="section-label">Transcription</span>
          </div>

          <!-- Filler word removal -->
          <div
            class="flex items-center justify-between p-2.5 rounded-lg
                   bg-panel border border-edge"
          >
            <div class="flex flex-col min-w-0 mr-3">
              <span class="text-[12px] font-semibold text-ink">
                Remove filler words
              </span>
              <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
                Strips "um", "uh", "hmm" and similar
              </span>
            </div>
            <button
              class="toggle-switch flex-shrink-0"
              :class="settings?.removeFillers ? 'toggle-on' : 'toggle-off'"
              @click="updateRemoveFillers(!settings?.removeFillers)"
            >
              <div class="toggle-thumb" />
            </button>
          </div>

          <!-- Live partial-caption preview while recording -->
          <div
            class="flex items-center justify-between p-2.5 rounded-lg
                   bg-panel border border-edge mt-2"
          >
            <div class="flex flex-col min-w-0 mr-3">
              <span class="text-[12px] font-semibold text-ink">
                Live preview while recording
              </span>
              <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
                Show partial captions in the overlay as you dictate (extra CPU
                load; final transcript on stop is unaffected)
              </span>
            </div>
            <button
              class="toggle-switch flex-shrink-0"
              :class="settings?.streamingPreview ? 'toggle-on' : 'toggle-off'"
              @click="updateStreamingPreview(!settings?.streamingPreview)"
            >
              <div class="toggle-thumb" />
            </button>
          </div>

          <!-- Self-correction cleanup -->
          <div
            class="flex items-center justify-between p-2.5 rounded-lg
                   bg-panel border border-edge mt-2"
          >
            <div class="flex flex-col min-w-0 mr-3">
              <span class="text-[12px] font-semibold text-ink">
                Self-correction cleanup
              </span>
              <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
                Detect and remove corrections like "no wait" or restated phrases
              </span>
            </div>
            <button
              class="toggle-switch flex-shrink-0"
              :class="settings?.selfCorrection ? 'toggle-on' : 'toggle-off'"
              @click="updateSelfCorrection(!settings?.selfCorrection)"
            >
              <div class="toggle-thumb" />
            </button>
          </div>

          <!-- Correction model picker (shown when self-correction is enabled) -->
          <template v-if="settings?.selfCorrection">
            <!-- No model hint -->
            <div
              v-if="downloadedCorrectionModels.length === 0 && !downloadingCorrection"
              class="mt-2 p-2.5 rounded-lg bg-gold/[0.06] border border-gold/15"
            >
              <span class="text-[10px] text-gold leading-snug">
                Download a correction model below to enable self-correction cleanup.
              </span>
            </div>

            <!-- Active correction model -->
            <div
              v-if="downloadedCorrectionModels.some((m) => isCorrectionActive(m))"
              class="mt-2 p-2.5 rounded-lg bg-gold/[0.04] border border-gold/20"
            >
              <div class="flex items-center gap-2">
                <div
                  class="w-1.5 h-1.5 rounded-full bg-leaf shadow-[0_0_4px_rgba(95,183,96,0.5)]"
                />
                <span class="text-[11px] font-semibold text-ink">
                  {{ downloadedCorrectionModels.find((m) => isCorrectionActive(m))?.displayName }}
                </span>
                <span class="text-[9px] text-ink-faint tabular-nums ml-auto">
                  {{ formatBytes(downloadedCorrectionModels.find((m) => isCorrectionActive(m))?.sizeBytes ?? 0) }}
                </span>
              </div>
            </div>

            <!-- Downloaded correction models -->
            <div v-if="downloadedCorrectionModels.length > 0" class="mt-2">
              <span class="subsection-label">Correction models</span>
              <div class="flex flex-col gap-1.5 mt-1.5">
                <div
                  v-for="model in downloadedCorrectionModels"
                  :key="model.id"
                  class="model-row group"
                  :class="{
                    'bg-gold/[0.03] border-gold/15': isCorrectionActive(model),
                    'bg-panel border-edge hover:border-edge-strong hover:bg-raised': !isCorrectionActive(model),
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
                      <div
                        v-if="isCorrectionActive(model)"
                        class="w-1.5 h-1.5 rounded-full bg-gold"
                      />
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
                    class="flex-shrink-0 p-1 rounded-md opacity-0 group-hover:opacity-100
                           transition-all duration-150"
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
                      <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
                    </svg>
                    <span
                      v-else
                      class="text-[9px] font-bold uppercase tracking-wider px-0.5"
                    >
                      Delete?
                    </span>
                  </button>
                </div>
              </div>
            </div>

            <!-- Correction download progress -->
            <div
              v-if="downloadingCorrection"
              class="mt-2"
            >
              <div class="flex items-center justify-between mb-1.5">
                <span class="text-[11px] text-ink-muted font-medium">
                  Downloading…
                </span>
                <div class="flex items-center gap-2">
                  <span class="text-[11px] text-ink-faint tabular-nums">
                    {{ correctionDownloadProgress.toFixed(0) }}%
                  </span>
                  <button
                    type="button"
                    aria-label="Cancel download"
                    title="Cancel download"
                    class="flex items-center justify-center w-[18px] h-[18px] rounded-full
                           bg-raised border border-edge text-ink-faint
                           transition-colors duration-150
                           hover:bg-panel hover:text-ink hover:border-edge-strong
                           active:scale-95"
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
                  class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full
                         transition-[width] duration-300 ease-out"
                  :style="{ width: `${correctionDownloadProgress}%` }"
                />
              </div>
            </div>

            <!-- Available correction models to download -->
            <div v-if="availableCorrectionModels.length > 0" class="mt-2">
              <span class="subsection-label">Available to download</span>
              <div class="flex flex-col gap-1.5 mt-1.5">
                <div
                  v-for="model in availableCorrectionModels"
                  :key="model.id"
                  class="model-row bg-panel border-edge"
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
                        <span class="rating-label">Spd</span>
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
                        <span class="rating-label">Qual</span>
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
                    class="flex-shrink-0 flex items-center gap-1 px-2 py-1 rounded-md
                           bg-raised border border-edge text-[10px] font-semibold text-ink-muted
                           hover:bg-hover hover:text-ink hover:border-edge-strong
                           transition-all duration-150 active:scale-95"
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

            <!-- Correction model error -->
            <div
              v-if="correctionModelError"
              class="mt-2 p-2 rounded-md bg-flame/10 border border-flame/20"
            >
              <span class="text-[11px] text-flame">{{ correctionModelError }}</span>
            </div>
          </template>
        </section>

        <!-- ═══════════════ VOCABULARY SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 160ms">
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
                <path d="M4 19.5A2.5 2.5 0 016.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z" />
              </svg>
            </div>
            <span class="section-label">Vocabulary</span>
            <span
              v-if="vocabularyEntries.length > 0"
              class="ml-auto text-[9px] text-ink-faint tabular-nums"
            >
              {{ vocabularyEntries.length }} {{ vocabularyEntries.length === 1 ? 'word' : 'words' }}
            </span>
          </div>

          <!-- Learn from corrections toggle -->
          <div
            class="flex items-center justify-between p-2.5 rounded-lg
                   bg-panel border border-edge"
          >
            <div class="flex flex-col min-w-0 mr-3">
              <span class="text-[12px] font-semibold text-ink">
                Learn from corrections
              </span>
              <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
                Automatically learn words when you correct transcriptions
              </span>
            </div>
            <button
              class="toggle-switch flex-shrink-0"
              :class="settings?.vocabularyLearning ? 'toggle-on' : 'toggle-off'"
              @click="updateVocabularyLearning(!settings?.vocabularyLearning)"
            >
              <div class="toggle-thumb" />
            </button>
          </div>

          <!-- Vocabulary entries list -->
          <div v-if="vocabularyEntries.length > 0" class="mt-2">
            <span class="subsection-label">Learned words</span>
            <div class="flex flex-col gap-1 mt-1.5">
              <div
                v-for="entry in vocabularyEntries"
                :key="entry.wrong"
                class="group flex items-center gap-2 px-2.5 py-1.5 rounded-lg
                       bg-panel border border-edge"
              >
                <span class="text-[11px] text-ink-muted truncate">{{ entry.wrong }}</span>
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  class="flex-shrink-0 text-ink-faint"
                >
                  <line x1="5" y1="12" x2="19" y2="12" />
                  <polyline points="12 5 19 12 12 19" />
                </svg>
                <span class="text-[11px] font-semibold text-ink truncate">{{ entry.correct }}</span>
                <span
                  class="ml-auto flex-shrink-0 px-1 py-0.5 rounded text-[8px] font-bold uppercase tracking-wider"
                  :class="entry.source === 'auto'
                    ? 'bg-gold/10 text-gold/80 border border-gold/15'
                    : 'bg-raised text-ink-faint border border-edge'"
                >
                  {{ entry.source === 'auto' ? 'auto' : 'manual' }}
                </span>
                <button
                  class="flex-shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100
                         text-ink-faint hover:text-flame transition-all duration-150"
                  @click="handleRemoveVocab(entry.wrong)"
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
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </div>
            </div>
          </div>

          <!-- Add word form -->
          <div class="mt-2">
            <button
              v-if="!showAddVocab"
              class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg w-full
                     bg-panel border border-edge border-dashed
                     text-[11px] text-ink-faint font-medium
                     hover:bg-raised hover:text-ink-muted hover:border-edge-strong
                     transition-all duration-150"
              @click="showAddVocab = true"
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
                <line x1="12" y1="5" x2="12" y2="19" />
                <line x1="5" y1="12" x2="19" y2="12" />
              </svg>
              Add word
            </button>

            <div
              v-else
              class="flex flex-col gap-2 p-2.5 rounded-lg bg-panel border border-edge"
            >
              <div class="flex gap-2 items-center">
                <input
                  v-model="vocabWrong"
                  type="text"
                  placeholder="Wrong word"
                  class="flex-1 min-w-0 px-2 py-1 text-[11px] rounded-md
                         bg-raised border border-edge text-ink
                         placeholder:text-ink-faint/50
                         focus:outline-none focus:border-gold/40
                         transition-colors duration-150"
                  @keydown.enter="handleAddVocab"
                />
                <svg
                  width="10"
                  height="10"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                  class="flex-shrink-0 text-ink-faint"
                >
                  <line x1="5" y1="12" x2="19" y2="12" />
                  <polyline points="12 5 19 12 12 19" />
                </svg>
                <input
                  v-model="vocabCorrect"
                  type="text"
                  placeholder="Correct word"
                  class="flex-1 min-w-0 px-2 py-1 text-[11px] rounded-md
                         bg-raised border border-edge text-ink
                         placeholder:text-ink-faint/50
                         focus:outline-none focus:border-gold/40
                         transition-colors duration-150"
                  @keydown.enter="handleAddVocab"
                />
              </div>
              <div class="flex gap-1.5 justify-end">
                <button
                  class="px-2 py-0.5 rounded-md text-[10px] font-semibold
                         text-ink-faint hover:text-ink
                         transition-colors duration-150"
                  @click="showAddVocab = false; vocabWrong = ''; vocabCorrect = ''"
                >
                  Cancel
                </button>
                <button
                  class="px-2.5 py-0.5 rounded-md text-[10px] font-semibold
                         bg-gold text-canvas hover:bg-gold-hover
                         transition-all duration-150 active:scale-95
                         disabled:opacity-40 disabled:cursor-not-allowed"
                  :disabled="!vocabWrong.trim() || !vocabCorrect.trim() || vocabWrong.trim() === vocabCorrect.trim()"
                  @click="handleAddVocab"
                >
                  Save
                </button>
              </div>
            </div>
          </div>

          <!-- Clear all button -->
          <button
            v-if="vocabularyEntries.length > 0"
            class="mt-2 flex items-center justify-center gap-1.5 w-full px-2.5 py-1.5
                   rounded-lg text-[10px] font-semibold transition-all duration-150"
            :class="confirmClearVocab
              ? 'bg-flame/15 border border-flame/30 text-flame'
              : 'bg-panel border border-edge text-ink-faint hover:text-flame hover:border-flame/20 hover:bg-flame/5'"
            @click="handleClearVocab"
          >
            {{ confirmClearVocab ? 'Click again to clear all' : 'Clear all words' }}
          </button>
        </section>

        <!-- ═══════════════ GENERAL SECTION ═══════════════ -->
        <section class="settings-section" style="animation-delay: 200ms">
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
                <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
              </svg>
            </div>
            <span class="section-label">General</span>
          </div>

          <div
            class="flex items-center justify-between p-2.5 rounded-lg
                   bg-panel border border-edge"
          >
            <div class="flex flex-col min-w-0 mr-3">
              <span class="text-[12px] font-semibold text-ink">
                Launch at login
              </span>
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
            class="text-[10px] text-amber-400 mt-1.5 px-1 text-left
                   hover:underline cursor-pointer"
            @click="openLoginItemsSettings()"
          >
            Magpie needs approval in System Settings → Login Items.
            Click to open.
          </button>
        </section>

        <!-- ═══════════════ UPDATES SECTION ═══════════════ -->
        <UpdatesSection />

        <!-- Bottom spacer for safe scrolling -->
        <div class="h-2" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-section {
  animation: section-enter 0.3s ease-out both;
}

@keyframes section-enter {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.section-header {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 8px;
}

.section-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border-radius: 5px;
  background: var(--color-raised);
  border: 1px solid var(--color-edge);
  color: var(--color-ink-faint);
}

.section-label {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--color-ink-faint);
}

.subsection-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--color-ink-faint);
  letter-spacing: 0.02em;
}

.rating-label {
  font-size: 8px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  font-weight: 600;
  color: var(--color-ink-faint);
}

.model-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid;
  transition: all 0.15s ease;
}

.model-badge {
  flex-shrink: 0;
  padding: 1px 4px;
  border-radius: 3px;
  font-size: 8px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  background: var(--color-raised);
  border: 1px solid var(--color-edge);
  color: var(--color-ink-faint);
}

.model-badge-multi {
  background: var(--color-gold);
  background: rgba(232, 175, 71, 0.08);
  border-color: rgba(232, 175, 71, 0.2);
  color: var(--color-gold);
}

.lang-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 5px 10px;
  text-align: left;
  transition: all 0.1s ease;
}

/* ── Toggle Switch ── */
.toggle-switch {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 9px;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
}

.toggle-on {
  background: var(--color-gold);
  box-shadow: 0 0 8px rgba(232, 175, 71, 0.2);
}

.toggle-off {
  background: var(--color-edge-strong);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.toggle-on .toggle-thumb {
  left: 16px;
}

.toggle-off .toggle-thumb {
  left: 2px;
}
</style>
