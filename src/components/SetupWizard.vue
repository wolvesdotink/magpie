<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from "vue";
import {
  checkPermissions,
  openAccessibilitySettings,
  requestMicrophonePermission,
  openMicrophoneSettings,
  getAvailableModels,
  getDownloadedModels,
  getSettings,
  updateSettings,
  downloadModel,
  cancelDownload,
  selectModel,
  restartFnKeyMonitor,
  restartApp,
  type ModelInfo,
  type UserSettings,
} from "@/lib/commands";
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from "@/lib/events";
import type { UnlistenFn } from "@tauri-apps/api/event";

const emit = defineEmits<{
  complete: [];
}>();

// ── Step management ──────────────────────────────────────────────

type StepId = "welcome" | "permissions" | "model" | "transcription";

const currentStep = ref<StepId>("welcome");
const needsPermissions = ref(true);
const hasAccessibility = ref(false);
const hasMicrophone = ref(false);
const bothPermissionsGranted = computed(
  () => hasAccessibility.value && hasMicrophone.value,
);

const visibleSteps = computed<StepId[]>(() => {
  const steps: StepId[] = ["welcome"];
  if (needsPermissions.value) steps.push("permissions");
  steps.push("model", "transcription");
  return steps;
});

const currentStepIndex = computed(() =>
  visibleSteps.value.indexOf(currentStep.value),
);

function nextStep() {
  const idx = currentStepIndex.value;
  if (idx < visibleSteps.value.length - 1) {
    currentStep.value = visibleSteps.value[idx + 1];
  }
}

// ── Permissions ──────────────────────────────────────────────────

const checkingPermissions = ref(false);
const requestingMic = ref(false);
const micDenied = ref(false);
let permissionPollTimer: ReturnType<typeof setInterval> | null = null;

function stopPermissionPolling() {
  if (permissionPollTimer !== null) {
    clearInterval(permissionPollTimer);
    permissionPollTimer = null;
  }
}

function startPermissionPolling() {
  stopPermissionPolling();
  permissionPollTimer = setInterval(async () => {
    try {
      const perms = await checkPermissions();
      hasAccessibility.value = perms.accessibility;
      hasMicrophone.value = perms.microphone;
      if (hasAccessibility.value && hasMicrophone.value) {
        stopPermissionPolling();
        needsPermissions.value = false;
        currentStep.value = "model";
        restartFnKeyMonitor();
      }
    } catch (e) {
      console.error("Permission poll failed:", e);
    }
  }, 2000);
}

async function handleOpenAccessibilitySettings() {
  await openAccessibilitySettings();
  startPermissionPolling();
}

async function handleCheckPermissions() {
  checkingPermissions.value = true;
  try {
    const perms = await checkPermissions();
    hasAccessibility.value = perms.accessibility;
    hasMicrophone.value = perms.microphone;
    if (hasAccessibility.value && hasMicrophone.value) {
      stopPermissionPolling();
      needsPermissions.value = false;
      currentStep.value = "model";
      restartFnKeyMonitor();
    }
  } catch (e) {
    console.error("Permission check failed:", e);
  }
  checkingPermissions.value = false;
}

async function handleGrantMicrophone() {
  requestingMic.value = true;
  try {
    const granted = await requestMicrophonePermission();
    hasMicrophone.value = granted;
    micDenied.value = !granted;
    if (hasAccessibility.value && hasMicrophone.value) {
      stopPermissionPolling();
      needsPermissions.value = false;
      currentStep.value = "model";
      restartFnKeyMonitor();
    }
  } catch (e) {
    console.error("Microphone permission request failed:", e);
  }
  requestingMic.value = false;
}

async function handleOpenMicSettings() {
  await openMicrophoneSettings();
}

// Auto-start polling when the permissions step becomes active
watch(currentStep, (step) => {
  if (step === "permissions" && !bothPermissionsGranted.value) {
    startPermissionPolling();
  } else {
    stopPermissionPolling();
  }
});

// Re-check immediately when the window regains focus (e.g. user returns from System Settings)
function onWindowFocus() {
  if (currentStep.value === "permissions" && !bothPermissionsGranted.value) {
    handleCheckPermissions();
  }
}

// ── Model selection ──────────────────────────────────────────────

const models = ref<ModelInfo[]>([]);
const downloadedFiles = ref<string[]>([]);
type ModelTab = "english" | "multilingual";
const activeTab = ref<ModelTab>("multilingual");
const selectedModelId = ref<string | null>("small");
const downloading = ref(false);
const downloadProgress = ref(0);
const downloadingModelId = ref<string | null>(null);
const modelError = ref<string | null>(null);

const unlisteners: UnlistenFn[] = [];

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(0)} KB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

function isDownloaded(model: ModelInfo): boolean {
  return downloadedFiles.value.includes(model.filename);
}

const displayedModels = computed(() =>
  models.value
    .filter((m) =>
      activeTab.value === "english" ? m.englishOnly : !m.englishOnly,
    )
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
      nextStep();
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
    nextStep();
  } catch (e) {
    // Suppress cancel-as-rejection — the cancelled listener resets state.
    if (!String(e).toLowerCase().includes("cancel")) {
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
    console.error("Cancel failed:", e);
  }
}

// ── Transcript settings ──────────────────────────────────────────

const settings = ref<UserSettings | null>(null);

function toggleRemoveFillers() {
  if (!settings.value) return;
  settings.value = {
    ...settings.value,
    removeFillers: !settings.value.removeFillers,
  };
}

function toggleSelfCorrection() {
  if (!settings.value) return;
  settings.value = {
    ...settings.value,
    selfCorrection: !settings.value.selfCorrection,
  };
}

async function handleFinish() {
  try {
    const current = await getSettings();
    await updateSettings({
      ...current,
      removeFillers: settings.value?.removeFillers ?? current.removeFillers,
      selfCorrection: settings.value?.selfCorrection ?? current.selfCorrection,
      setupComplete: true,
    });
  } catch (e) {
    console.error("Failed to save settings:", e);
  }
  emit("complete");
}

// ── Lifecycle ────────────────────────────────────────────────────

onMounted(async () => {
  try {
    const [perms, availableModels, downloaded, userSettings] =
      await Promise.all([
        checkPermissions(),
        getAvailableModels(),
        getDownloadedModels(),
        getSettings(),
      ]);

    hasAccessibility.value = perms.accessibility;
    hasMicrophone.value = perms.microphone;
    needsPermissions.value = !perms.accessibility || !perms.microphone;
    models.value = availableModels;
    downloadedFiles.value = downloaded;
    settings.value = userSettings;
  } catch (e) {
    console.error("Setup wizard init failed:", e);
  }

  window.addEventListener("focus", onWindowFocus);

  unlisteners.push(
    await onModelDownloadProgress((progress) => {
      downloadProgress.value = progress.percent;
    }),
  );

  unlisteners.push(
    await onModelDownloadComplete(() => {
      downloading.value = false;
      downloadingModelId.value = null;
    }),
  );

  unlisteners.push(
    await onModelDownloadCancelled(() => {
      downloading.value = false;
      downloadingModelId.value = null;
      downloadProgress.value = 0;
      modelError.value = null;
    }),
  );
});

onUnmounted(() => {
  stopPermissionPolling();
  window.removeEventListener("focus", onWindowFocus);
  unlisteners.forEach((u) => u());
});
</script>

<template>
  <div class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden">
    <!-- Top edge -->
    <div
      class="h-px bg-gradient-to-r from-transparent via-gold/20 to-transparent"
    />

    <!-- ════════════════ STEP 1: WELCOME ════════════════ -->
    <div
      v-if="currentStep === 'welcome'"
      class="flex flex-col items-center justify-center flex-1 px-6 gap-5"
    >
      <!-- Logo -->
      <div class="relative">
        <div
          class="flex items-center justify-center w-16 h-16 rounded-2xl
                 bg-gradient-to-br from-gold via-gold to-gold-deep
                 text-gold-ink shadow-glow-gold"
        >
          <svg
            class="w-9 h-9"
            viewBox="0 0 1046 1044"
            fill="currentColor"
            xmlns="http://www.w3.org/2000/svg"
            aria-label="Magpie"
          >
            <path d="M229.203 4.93335C221.203 21.0667 206.136 73.7333 201.736 101.333C196.269 135.867 198.003 188 205.869 226.533C222.269 306.667 263.07 383.333 318.936 438.933C364.27 484.133 419.87 515.333 474.936 526.533C482.136 528 488.136 529.333 488.403 529.6C489.07 530.267 448.536 575.2 428.536 596C418.27 606.667 322.536 702.933 215.736 810C108.936 917.067 16.6695 1010.4 10.5361 1017.33C4.53612 1024.27 -0.130546 1030.93 0.00278714 1032C0.536121 1034.67 10.4028 1039.33 21.0695 1042.13C27.6028 1043.73 33.6028 1044.13 47.0695 1043.6C66.6695 1042.93 75.0695 1040.93 91.2028 1033.47C114.269 1022.67 127.203 1011.47 154.936 978.667C183.203 945.067 255.203 863.067 341.603 766C398.003 702.4 410.27 689.333 412.536 689.333C415.47 689.333 414.803 696.4 411.336 701.467C406.936 708 382.803 738.4 332.403 801.333C234.536 923.2 191.469 977.467 192.003 978.133C192.403 978.4 196.136 979.333 200.403 980.133C222.803 984.267 257.069 973.6 282.136 954.667C303.203 938.933 312.003 927.733 364.536 850.933C391.47 811.6 418.003 772.8 423.47 764.933C441.736 738.267 458.003 725.6 482.403 718.8C497.47 714.667 508.803 713.6 552.27 712C574.27 711.2 599.736 709.6 608.936 708.533C707.07 697.067 787.736 650 838.536 574.533C862.803 538.533 876.536 502.133 888.936 442.267C896.003 408.267 902.136 387.067 908.403 374.533C920.936 349.733 936.936 332.267 954.936 323.867C971.336 316.267 1002 308.133 1035.47 302.667C1040.67 301.867 1045.07 300.533 1045.07 299.867C1045.07 298.133 1031.74 289.733 1022.54 285.733C1010.8 280.4 1002 277.6 979.07 271.733C954.403 265.467 951.87 264.4 943.736 256.667C929.336 242.933 916.27 235.467 897.47 230C859.87 218.933 818.27 228.267 781.736 255.6C774.67 260.933 749.47 284.667 720.67 313.2C692.803 340.933 671.203 361.2 670.536 360.533C670.003 359.867 667.603 353.6 665.203 346.667C655.07 316.8 640.403 291.867 621.336 271.867C602.27 251.733 591.603 244.4 506.936 192.933C456.403 162.267 444.403 154.533 412.403 132.267C365.203 99.3333 282.269 38.3999 248.136 11.7333C239.869 5.33328 232.803 0 232.403 0C232.136 0 230.669 2.26668 229.203 4.93335ZM277.336 78.4C303.603 98.1333 341.203 125.067 407.336 171.2C454.936 204.4 465.203 212.8 477.47 228.133C491.87 246.133 507.603 280.8 511.07 302.667L512.136 308.667L506.27 301.333C487.47 277.2 459.336 257.067 414.403 235.333C368.136 213.067 334.536 195.467 320.27 186C282.936 161.2 261.869 132.267 254.269 95.3333C252.003 84.6666 250.669 60 252.403 60C252.669 60 264.003 68.2666 277.336 78.4ZM242.003 179.2C273.736 203.067 306.003 221.867 371.736 254.8C402.136 270 431.336 284.933 436.403 288.133C468.27 307.467 489.87 331.333 499.203 357.6L500.936 362.667L492.67 355.6C470.27 336.667 447.736 326.933 373.736 304.533C333.07 292.133 323.336 288.8 306.67 281.2C268.536 263.733 245.469 235.333 236.669 195.333C234.136 183.333 232.669 173.333 233.603 173.333C234.003 173.333 237.869 176 242.003 179.2ZM890.936 268.667C895.603 270.933 899.736 276.8 899.736 281.333C899.736 286.667 895.736 293.067 890.803 295.333C881.203 299.867 870.403 292.933 870.403 282.4C870.403 275.733 872.136 272.533 877.47 269.333C882.67 266.133 885.336 266 890.936 268.667ZM303.07 319.333C320.803 326.133 343.47 333.067 384.403 344C428.136 355.733 451.203 364.933 473.47 379.6C482.936 385.867 498.27 400.133 501.07 405.2C502.536 408.133 502.536 408.133 498.67 406.133C478.803 395.867 449.87 389.467 398.403 384C359.336 379.867 342.803 376.533 324.67 369.067C305.736 361.333 287.603 346.133 278.536 330.267C275.069 324.133 266.403 304.8 266.403 303.2C266.403 302.8 271.336 305.067 277.336 308.133C283.47 311.2 294.936 316.267 303.07 319.333ZM758.803 384.4C768.403 389.2 771.87 394.533 771.87 404.8C771.87 428 742.803 455.733 696.803 476.267C669.603 488.4 642.936 495.6 610.936 499.333C589.203 501.867 555.736 502.133 555.736 499.6C555.736 495.067 607.603 442.4 629.736 424.667C650.27 408.267 674.67 393.867 693.07 387.467C710.003 381.467 719.07 380 735.87 380.4C749.603 380.8 752.136 381.2 758.803 384.4ZM820.936 438.4C826.27 452.533 828.136 463.333 828.136 482C828.003 526.133 809.203 567.2 773.07 601.867C756.67 617.6 742.803 628 724.936 638.133C674.67 666.533 620.003 679.467 586.136 670.8C564.136 665.067 546.27 651.6 537.603 633.867C532.27 623.2 532.27 618.133 537.47 616C539.07 615.333 550.27 610.933 562.403 606.267C603.736 590 633.603 576.133 663.07 558.933C702.136 536.267 725.07 518.933 753.07 490.8C776.936 466.933 791.07 448.8 800.536 430.133L807.07 417.2L812.003 422.667C814.803 425.867 818.67 432.533 820.936 438.4ZM665.203 521.2C652.136 531.2 592.136 560.667 550.936 577.333C511.47 593.333 462.403 611.2 462.536 609.6C462.536 609.067 476.136 592 492.803 571.467L523.07 534.4L559.07 534C599.603 533.733 617.47 531.6 648.403 523.467C668.936 518.133 669.336 518 665.203 521.2Z" />
          </svg>
        </div>
        <div
          class="absolute -inset-2.5 rounded-[20px] border-2 border-gold/25 animate-breathe"
        />
      </div>

      <!-- Copy -->
      <div class="text-center">
        <h1 class="text-[18px] font-bold tracking-tight text-ink">
          Welcome to Magpie
        </h1>
        <p
          class="text-[11px] text-ink-muted leading-relaxed mt-2 max-w-[240px]"
        >
          Fast, private voice-to-text that runs entirely on your Mac. Let's get
          you set up in a few quick steps.
        </p>
      </div>

      <!-- CTA -->
      <button
        class="w-full max-w-[240px] py-2.5 rounded-lg text-[13px] font-semibold
               transition-all duration-200 active:scale-[0.97]
               bg-gradient-to-b from-gold to-gold-hover text-gold-ink
               hover:from-gold-hover hover:to-gold-deep
               shadow-press hover:shadow-lifted mt-2"
        @click="nextStep"
      >
        Get Started
      </button>
    </div>

    <!-- ════════════════ STEP 2: PERMISSIONS ════════════════ -->
    <div
      v-else-if="currentStep === 'permissions'"
      class="flex flex-col items-center px-6 pt-8 pb-6 gap-4 flex-1 overflow-y-auto"
    >
      <!-- Shield Icon -->
      <div
        class="flex items-center justify-center w-14 h-14 rounded-2xl
               bg-gradient-to-br from-gold/15 to-gold/5 text-gold shadow-glow-gold"
      >
        <svg
          width="28"
          height="28"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
        >
          <path
            d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
          <path
            d="M9 12l2 2 4-4"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </div>

      <!-- Title -->
      <div class="text-center">
        <h2 class="text-[15px] font-bold tracking-tight text-ink">
          Permissions Required
        </h2>
        <p
          class="text-[10px] text-ink-muted leading-relaxed mt-1.5 max-w-[240px]"
        >
          Magpie needs two permissions to work: microphone access for recording
          and accessibility for detecting the Fn key.
        </p>
      </div>

      <!-- Permission cards -->
      <div class="w-full flex flex-col gap-3">
        <!-- ── Microphone card ── -->
        <div
          class="w-full p-3.5 rounded-lg border transition-colors duration-200"
          :class="
            hasMicrophone
              ? 'bg-leaf/[0.06] border-leaf/25'
              : 'bg-panel border-edge'
          "
        >
          <div class="flex items-center justify-between gap-3 mb-2">
            <div class="flex items-center gap-2.5">
              <!-- Mic icon -->
              <div
                class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
                :class="
                  hasMicrophone
                    ? 'bg-leaf/15 text-leaf'
                    : 'bg-raised text-ink-faint'
                "
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                  <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                  <line x1="12" y1="19" x2="12" y2="22" />
                </svg>
              </div>
              <div>
                <span class="text-[12px] font-semibold text-ink">
                  Microphone
                </span>
                <p class="text-[9px] text-ink-faint leading-snug mt-0.5">
                  Record your voice for transcription
                </p>
              </div>
            </div>
            <!-- Status badge -->
            <span
              v-if="hasMicrophone"
              class="flex items-center gap-1 text-[9px] font-bold text-leaf
                     uppercase tracking-wider flex-shrink-0"
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
              Granted
            </span>
          </div>
          <!-- Action buttons for microphone -->
          <div v-if="!hasMicrophone" class="mt-2">
            <button
              v-if="!micDenied"
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-gradient-to-b from-gold to-gold-hover text-gold-ink
                     hover:from-gold-hover hover:to-gold-deep
                     shadow-press hover:shadow-lifted"
              :disabled="requestingMic"
              @click="handleGrantMicrophone"
            >
              {{ requestingMic ? "Waiting\u2026" : "Grant Microphone Access" }}
            </button>
            <div v-else class="flex flex-col gap-1.5">
              <p class="text-[9px] text-flame leading-snug">
                Microphone access was denied. Please enable it in System
                Settings.
              </p>
              <button
                class="w-full py-2 rounded-md text-[11px] font-semibold
                       transition-all duration-200 active:scale-[0.97]
                       bg-panel border border-edge text-ink-muted shadow-soft
                       hover:bg-raised hover:border-edge-strong"
                @click="handleOpenMicSettings"
              >
                Open Microphone Settings
              </button>
            </div>
          </div>
        </div>

        <!-- ── Accessibility card ── -->
        <div
          class="w-full p-3.5 rounded-lg border transition-colors duration-200"
          :class="
            hasAccessibility
              ? 'bg-leaf/[0.06] border-leaf/25'
              : 'bg-panel border-edge'
          "
        >
          <div class="flex items-center justify-between gap-3 mb-2">
            <div class="flex items-center gap-2.5">
              <!-- Accessibility icon -->
              <div
                class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
                :class="
                  hasAccessibility
                    ? 'bg-leaf/15 text-leaf'
                    : 'bg-raised text-ink-faint'
                "
              >
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                >
                  <path d="M18 8h1a4 4 0 0 1 0 8h-1" />
                  <path d="M5 8H4a4 4 0 0 0 0 8h1" />
                  <line x1="8" y1="6" x2="8" y2="2" />
                  <line x1="16" y1="6" x2="16" y2="2" />
                  <rect x="5" y="6" width="14" height="12" rx="2" />
                </svg>
              </div>
              <div>
                <span class="text-[12px] font-semibold text-ink">
                  Accessibility
                </span>
                <p class="text-[9px] text-ink-faint leading-snug mt-0.5">
                  Detect Fn key and paste transcribed text
                </p>
              </div>
            </div>
            <!-- Status badge -->
            <span
              v-if="hasAccessibility"
              class="flex items-center gap-1 text-[9px] font-bold text-leaf
                     uppercase tracking-wider flex-shrink-0"
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
              Granted
            </span>
          </div>
          <!-- Action buttons for accessibility -->
          <div v-if="!hasAccessibility" class="flex flex-col gap-3 mt-2">
            <div
              class="flex flex-col gap-2 p-2.5 bg-raised/50 rounded-md"
            >
              <div class="flex items-center gap-2">
                <span
                  class="flex items-center justify-center w-4 h-4 rounded-full
                         bg-gold/20 text-gold text-[8px] font-bold flex-shrink-0"
                >1</span>
                <span class="text-[9px] text-ink-muted leading-snug">
                  Click "Open Settings" below
                </span>
              </div>
              <div class="flex items-center gap-2">
                <span
                  class="flex items-center justify-center w-4 h-4 rounded-full
                         bg-gold/20 text-gold text-[8px] font-bold flex-shrink-0"
                >2</span>
                <span class="text-[9px] text-ink-muted leading-snug">
                  Enable <strong class="text-ink font-semibold">Magpie</strong>
                </span>
              </div>
            </div>
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-gradient-to-b from-gold to-gold-hover text-gold-ink
                     hover:from-gold-hover hover:to-gold-deep
                     shadow-press hover:shadow-lifted"
              @click="handleOpenAccessibilitySettings"
            >
              Open Accessibility Settings
            </button>
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-panel border border-edge text-ink-muted shadow-soft
                     hover:bg-raised hover:border-edge-strong"
              @click="restartApp"
            >
              Already enabled? Restart Magpie
            </button>
          </div>
        </div>
      </div>

      <!-- Bottom actions -->
      <div class="w-full flex flex-col gap-2">
        <button
          v-if="bothPermissionsGranted"
          class="w-full py-2.5 rounded-lg text-[13px] font-semibold
                 transition-all duration-200 active:scale-[0.97]
                 bg-gradient-to-b from-gold to-gold-hover text-gold-ink
                 hover:from-gold-hover hover:to-gold-deep
                 shadow-press hover:shadow-lifted"
          @click="nextStep"
        >
          Continue
        </button>
        <button
          v-else
          class="w-full py-2.5 rounded-lg text-[13px] font-semibold
                 transition-all duration-200 active:scale-[0.97]
                 bg-panel border border-edge text-ink-muted shadow-soft
                 hover:bg-raised hover:border-edge-strong hover:shadow-lifted"
          :disabled="checkingPermissions"
          @click="handleCheckPermissions"
        >
          {{ checkingPermissions ? "Checking\u2026" : "Check Again" }}
        </button>
      </div>
    </div>

    <!-- ════════════════ STEP 3: MODEL SELECTION ════════════════ -->
    <div
      v-else-if="currentStep === 'model'"
      class="flex flex-col flex-1 p-5 pt-6 min-h-0"
    >
      <!-- Header -->
      <h2 class="text-[15px] font-bold tracking-tight text-ink">
        Choose a Model
      </h2>
      <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 mb-4">
        Select a speech recognition model. Larger models are more accurate but
        slower.
      </p>

      <!-- Tabs -->
      <div class="flex gap-1 p-1 rounded-lg bg-raised border border-edge mb-4">
        <button
          class="flex-1 py-1.5 rounded-md text-[12px] font-semibold
                 transition-all duration-200"
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
          class="flex-1 py-1.5 rounded-md text-[12px] font-semibold
                 transition-all duration-200"
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
          class="relative flex flex-col gap-1.5 p-3 rounded-lg border text-left
                 transition-all duration-200 active:scale-[0.99]"
          :class="{
            'bg-gold/[0.05] border-gold/30 shadow-glow-gold':
              selectedModelId === model.id,
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
              <!-- Recommended badge for multilingual small -->
              <span
                v-if="model.id === 'small'"
                class="px-1.5 py-[1px] rounded text-[8px] font-bold uppercase
                       tracking-wider bg-gold/15 text-gold border border-gold/20"
              >
                Recommended
              </span>
            </div>
            <div class="flex items-center gap-2 flex-shrink-0">
              <span
                v-if="isDownloaded(model)"
                class="flex items-center gap-1 text-[10px] font-bold
                       text-leaf uppercase tracking-wider"
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
              <span
                class="text-[11px] text-ink-faint font-medium tabular-nums"
              >
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
              <span
                class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint"
              >
                Speed
              </span>
              <div class="flex gap-[3px]">
                <span
                  v-for="i in 5"
                  :key="i"
                  class="w-[5px] h-[5px] rounded-full transition-colors duration-200"
                  :class="
                    i <= 6 - model.speedRating ? 'bg-gold' : 'bg-edge'
                  "
                />
              </div>
            </div>
            <div class="flex items-center gap-1.5">
              <span
                class="text-[9px] uppercase tracking-[0.08em] font-semibold text-ink-faint"
              >
                Accuracy
              </span>
              <div class="flex gap-[3px]">
                <span
                  v-for="i in 5"
                  :key="i"
                  class="w-[5px] h-[5px] rounded-full transition-colors duration-200"
                  :class="
                    i <= model.accuracyRating ? 'bg-gold' : 'bg-edge'
                  "
                />
              </div>
            </div>
          </div>
        </button>
      </div>

      <!-- Error -->
      <div
        v-if="modelError"
        class="p-2.5 rounded-lg bg-flame/10 border border-flame/20 mb-3"
      >
        <span class="text-[10px] text-flame">{{ modelError }}</span>
      </div>

      <!-- Download Progress -->
      <div
        v-if="downloading && downloadingModelId === selectedModelId"
        class="flex items-center gap-3"
      >
        <div
          class="flex-1 h-1.5 bg-raised shadow-well rounded-full overflow-hidden"
        >
          <div
            class="h-full bg-gradient-to-r from-gold-deep to-gold rounded-full
                   transition-[width] duration-300 ease-out"
            :style="{ width: `${downloadProgress}%` }"
          />
        </div>
        <span
          class="text-[10px] text-ink-muted font-medium tabular-nums min-w-[36px] text-right"
        >
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

      <!-- Action Button -->
      <button
        v-else
        class="w-full py-2.5 rounded-lg text-[13px] font-semibold
               transition-all duration-200 active:scale-[0.97]
               bg-gradient-to-b from-gold to-gold-hover text-gold-ink
               hover:from-gold-hover hover:to-gold-deep
               shadow-press hover:shadow-lifted"
        :disabled="!selectedModelId || downloading"
        @click="handleDownloadAndContinue"
      >
        {{
          selectedModelId &&
          models.find((m) => m.id === selectedModelId) &&
          isDownloaded(models.find((m) => m.id === selectedModelId)!)
            ? "Continue"
            : "Download & Continue"
        }}
      </button>
    </div>

    <!-- ════════════════ STEP 4: TRANSCRIPT IMPROVEMENTS ════════════════ -->
    <div
      v-else-if="currentStep === 'transcription'"
      class="flex flex-col items-center px-6 pt-8 pb-6 gap-5 flex-1"
    >
      <!-- Icon -->
      <div
        class="flex items-center justify-center w-14 h-14 rounded-2xl
               bg-gradient-to-br from-gold/15 to-gold/5 text-gold shadow-glow-gold"
      >
        <svg
          width="26"
          height="26"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M12 20h9" />
          <path
            d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z"
          />
        </svg>
      </div>

      <!-- Title -->
      <div class="text-center">
        <h2 class="text-[15px] font-bold tracking-tight text-ink">
          Improve Your Transcripts
        </h2>
        <p
          class="text-[10px] text-ink-muted leading-relaxed mt-2 max-w-[240px]"
        >
          Clean up your transcriptions automatically. You can change these
          anytime in Settings.
        </p>
      </div>

      <!-- Toggles -->
      <div class="w-full flex flex-col gap-2">
        <!-- Filler word removal -->
        <div
          class="flex items-center justify-between p-3 rounded-lg
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
            :class="
              settings?.removeFillers ? 'toggle-on' : 'toggle-off'
            "
            @click="toggleRemoveFillers"
          >
            <div class="toggle-thumb" />
          </button>
        </div>

        <!-- Self-correction cleanup -->
        <div
          class="flex items-center justify-between p-3 rounded-lg
                 bg-panel border border-edge"
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
            :class="
              settings?.selfCorrection ? 'toggle-on' : 'toggle-off'
            "
            @click="toggleSelfCorrection"
          >
            <div class="toggle-thumb" />
          </button>
        </div>

        <!-- Self-correction hint -->
        <div
          v-if="settings?.selfCorrection"
          class="p-2.5 rounded-lg bg-gold/[0.06] border border-gold/15"
        >
          <span class="text-[10px] text-gold leading-snug">
            Requires a correction model. You can download one in Settings after
            setup.
          </span>
        </div>
      </div>

      <!-- CTA -->
      <button
        class="w-full py-2.5 rounded-lg text-[13px] font-semibold
               transition-all duration-200 active:scale-[0.97]
               bg-gradient-to-b from-gold to-gold-hover text-gold-ink
               hover:from-gold-hover hover:to-gold-deep
               shadow-press hover:shadow-lifted mt-auto"
        @click="handleFinish"
      >
        Finish Setup
      </button>
    </div>

    <!-- ════════════════ PROGRESS DOTS ════════════════ -->
    <div class="flex justify-center gap-2 pb-4 pt-2">
      <span
        v-for="(step, idx) in visibleSteps"
        :key="step"
        class="w-[6px] h-[6px] rounded-full transition-all duration-300"
        :class="
          idx === currentStepIndex
            ? 'bg-gold scale-110'
            : idx < currentStepIndex
              ? 'bg-gold/40'
              : 'bg-edge'
        "
      />
    </div>
  </div>
</template>

<style scoped>
/* ── Toggle Switch (matches SettingsView.vue) ── */
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
