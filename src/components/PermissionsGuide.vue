<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import {
  checkPermissions,
  openAccessibilitySettings,
  requestMicrophonePermission,
  openMicrophoneSettings,
  requestInputMonitoringPermission,
  openInputMonitoringSettings,
  restartFnKeyMonitor,
  restartApp,
} from "@/lib/commands";

const emit = defineEmits<{
  granted: [];
}>();

const checking = ref(false);
const requestingMic = ref(false);
const requestingInputMon = ref(false);
const hasAccessibility = ref(false);
const hasMicrophone = ref(false);
const hasInputMonitoring = ref(false);
const micDenied = ref(false);

const allGranted = computed(
  () =>
    hasAccessibility.value &&
    hasMicrophone.value &&
    hasInputMonitoring.value,
);

let permissionPollTimer: ReturnType<typeof setInterval> | null = null;

function stopPermissionPolling() {
  if (permissionPollTimer !== null) {
    clearInterval(permissionPollTimer);
    permissionPollTimer = null;
  }
}

// When Input Monitoring transitions OFF → ON the Fn key monitor must be
// restarted — the CGEventTap created during startup was rejected and
// needs to be recreated now that permission is granted.
let lastInputMonitoring = false;
async function maybeRestartFnMonitor(next: boolean) {
  if (next && !lastInputMonitoring) {
    try {
      await restartFnKeyMonitor();
    } catch (e) {
      console.error("Failed to restart Fn key monitor:", e);
    }
  }
  lastInputMonitoring = next;
}

function startPermissionPolling() {
  stopPermissionPolling();
  permissionPollTimer = setInterval(async () => {
    try {
      const perms = await checkPermissions();
      hasAccessibility.value = perms.accessibility;
      hasMicrophone.value = perms.microphone;
      await maybeRestartFnMonitor(perms.inputMonitoring);
      hasInputMonitoring.value = perms.inputMonitoring;
      if (allGranted.value) {
        stopPermissionPolling();
        emit("granted");
      }
    } catch (e) {
      console.error("Permission poll failed:", e);
    }
  }, 2000);
}

onMounted(async () => {
  try {
    const perms = await checkPermissions();
    hasAccessibility.value = perms.accessibility;
    hasMicrophone.value = perms.microphone;
    hasInputMonitoring.value = perms.inputMonitoring;
    lastInputMonitoring = perms.inputMonitoring;
    if (allGranted.value) {
      emit("granted");
    } else {
      startPermissionPolling();
    }
  } catch (e) {
    console.error("Permission check failed:", e);
  }

  window.addEventListener("focus", onWindowFocus);
});

onUnmounted(() => {
  stopPermissionPolling();
  window.removeEventListener("focus", onWindowFocus);
});

async function openSettings() {
  await openAccessibilitySettings();
  startPermissionPolling();
}

async function handleRequestInputMonitoring() {
  requestingInputMon.value = true;
  try {
    // This call triggers the TCC prompt on first run AND adds the app to
    // System Settings → Privacy & Security → Input Monitoring so the user
    // can toggle it on. On subsequent calls it just returns the current
    // state, so we open the Settings pane to let the user finish granting.
    const granted = await requestInputMonitoringPermission();
    hasInputMonitoring.value = granted;
    if (granted) {
      await maybeRestartFnMonitor(true);
    } else {
      await openInputMonitoringSettings();
      startPermissionPolling();
    }
    if (allGranted.value) {
      stopPermissionPolling();
      emit("granted");
    }
  } catch (e) {
    console.error("Input Monitoring request failed:", e);
  }
  requestingInputMon.value = false;
}

async function handleOpenInputMonitoringSettings() {
  await openInputMonitoringSettings();
  startPermissionPolling();
}

async function handleGrantMicrophone() {
  requestingMic.value = true;
  try {
    const granted = await requestMicrophonePermission();
    hasMicrophone.value = granted;
    micDenied.value = !granted;
    if (allGranted.value) {
      stopPermissionPolling();
      emit("granted");
    }
  } catch (e) {
    console.error("Microphone permission request failed:", e);
  }
  requestingMic.value = false;
}

async function handleOpenMicSettings() {
  await openMicrophoneSettings();
}

// Re-check immediately when the window regains focus (e.g. user returns from System Settings)
function onWindowFocus() {
  if (!allGranted.value) {
    recheckPermissions();
  }
}

async function recheckPermissions() {
  checking.value = true;
  try {
    const perms = await checkPermissions();
    hasAccessibility.value = perms.accessibility;
    hasMicrophone.value = perms.microphone;
    await maybeRestartFnMonitor(perms.inputMonitoring);
    hasInputMonitoring.value = perms.inputMonitoring;
    if (allGranted.value) {
      stopPermissionPolling();
      emit("granted");
    }
  } catch (e) {
    console.error("Permission check failed:", e);
  }
  checking.value = false;
}
</script>

<template>
  <div class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden">
    <!-- Top edge -->
    <div class="h-px bg-gradient-to-r from-transparent via-gold/20 to-transparent" />

    <div class="flex flex-col items-center px-6 pt-8 pb-6 gap-4 flex-1">
      <!-- ── Shield Icon ── -->
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

      <!-- ── Title ── -->
      <div class="text-center">
        <h2 class="text-[15px] font-bold tracking-tight text-ink">
          Permissions Required
        </h2>
        <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 max-w-[240px]">
          Magpie needs the following permissions to function. Please grant any
          missing ones below.
        </p>
      </div>

      <!-- ── Permission cards ── -->
      <div class="w-full flex flex-col gap-3">
        <!-- Microphone -->
        <div
          class="w-full p-3.5 rounded-lg border transition-colors duration-200"
          :class="
            hasMicrophone
              ? 'bg-leaf/[0.06] border-leaf/25'
              : 'bg-panel border-edge'
          "
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2.5">
              <div
                class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
                :class="
                  hasMicrophone
                    ? 'bg-leaf/15 text-leaf'
                    : 'bg-raised text-ink-faint'
                "
              >
                <svg
                  width="14" height="14" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round"
                >
                  <path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
                  <path d="M19 10v2a7 7 0 0 1-14 0v-2" />
                  <line x1="12" y1="19" x2="12" y2="22" />
                </svg>
              </div>
              <span class="text-[12px] font-semibold text-ink">Microphone</span>
            </div>
            <span
              v-if="hasMicrophone"
              class="flex items-center gap-1 text-[9px] font-bold text-leaf
                     uppercase tracking-wider flex-shrink-0"
            >
              <svg
                width="10" height="10" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="3"
                stroke-linecap="round" stroke-linejoin="round"
              ><polyline points="20 6 9 17 4 12" /></svg>
              Granted
            </span>
          </div>
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
                Microphone access was denied. Please enable it in System Settings.
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

        <!-- Accessibility -->
        <div
          class="w-full p-3.5 rounded-lg border transition-colors duration-200"
          :class="
            hasAccessibility
              ? 'bg-leaf/[0.06] border-leaf/25'
              : 'bg-panel border-edge'
          "
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2.5">
              <div
                class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
                :class="
                  hasAccessibility
                    ? 'bg-leaf/15 text-leaf'
                    : 'bg-raised text-ink-faint'
                "
              >
                <svg
                  width="14" height="14" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round"
                >
                  <path d="M18 8h1a4 4 0 0 1 0 8h-1" />
                  <path d="M5 8H4a4 4 0 0 0 0 8h1" />
                  <line x1="8" y1="6" x2="8" y2="2" />
                  <line x1="16" y1="6" x2="16" y2="2" />
                  <rect x="5" y="6" width="14" height="12" rx="2" />
                </svg>
              </div>
              <span class="text-[12px] font-semibold text-ink">Accessibility</span>
            </div>
            <span
              v-if="hasAccessibility"
              class="flex items-center gap-1 text-[9px] font-bold text-leaf
                     uppercase tracking-wider flex-shrink-0"
            >
              <svg
                width="10" height="10" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="3"
                stroke-linecap="round" stroke-linejoin="round"
              ><polyline points="20 6 9 17 4 12" /></svg>
              Granted
            </span>
          </div>
          <div v-if="!hasAccessibility" class="mt-2 flex flex-col gap-1.5">
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-gradient-to-b from-gold to-gold-hover text-gold-ink
                     hover:from-gold-hover hover:to-gold-deep
                     shadow-press hover:shadow-lifted"
              @click="openSettings"
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

        <!-- Input Monitoring (required for Fn key detection via CGEventTap) -->
        <div
          class="w-full p-3.5 rounded-lg border transition-colors duration-200"
          :class="
            hasInputMonitoring
              ? 'bg-leaf/[0.06] border-leaf/25'
              : 'bg-panel border-edge'
          "
        >
          <div class="flex items-center justify-between gap-3">
            <div class="flex items-center gap-2.5">
              <div
                class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
                :class="
                  hasInputMonitoring
                    ? 'bg-leaf/15 text-leaf'
                    : 'bg-raised text-ink-faint'
                "
              >
                <!-- keyboard icon -->
                <svg
                  width="14" height="14" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2"
                  stroke-linecap="round" stroke-linejoin="round"
                >
                  <rect x="2" y="6" width="20" height="12" rx="2" />
                  <line x1="6" y1="10" x2="6" y2="10" />
                  <line x1="10" y1="10" x2="10" y2="10" />
                  <line x1="14" y1="10" x2="14" y2="10" />
                  <line x1="18" y1="10" x2="18" y2="10" />
                  <line x1="7" y1="14" x2="17" y2="14" />
                </svg>
              </div>
              <span class="text-[12px] font-semibold text-ink">Input Monitoring</span>
            </div>
            <span
              v-if="hasInputMonitoring"
              class="flex items-center gap-1 text-[9px] font-bold text-leaf
                     uppercase tracking-wider flex-shrink-0"
            >
              <svg
                width="10" height="10" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="3"
                stroke-linecap="round" stroke-linejoin="round"
              ><polyline points="20 6 9 17 4 12" /></svg>
              Granted
            </span>
          </div>
          <div v-if="!hasInputMonitoring" class="mt-2 flex flex-col gap-1.5">
            <p class="text-[9px] text-ink-muted leading-snug">
              Needed to detect the Fn key. On macOS this is a separate pane
              from Accessibility.
            </p>
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-gradient-to-b from-gold to-gold-hover text-gold-ink
                     hover:from-gold-hover hover:to-gold-deep
                     shadow-press hover:shadow-lifted"
              :disabled="requestingInputMon"
              @click="handleRequestInputMonitoring"
            >
              {{ requestingInputMon ? "Waiting…" : "Grant Input Monitoring" }}
            </button>
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold
                     transition-all duration-200 active:scale-[0.97]
                     bg-panel border border-edge text-ink-muted shadow-soft
                     hover:bg-raised hover:border-edge-strong"
              @click="handleOpenInputMonitoringSettings"
            >
              Open Input Monitoring Settings
            </button>
          </div>
        </div>
      </div>

      <!-- ── Actions ── -->
      <div class="w-full flex flex-col gap-2 mt-auto">
        <button
          class="w-full py-2.5 rounded-lg text-[13px] font-semibold
                 transition-all duration-200 active:scale-[0.97]
                 bg-panel border border-edge text-ink-muted shadow-soft
                 hover:bg-raised hover:border-edge-strong hover:shadow-lifted"
          :disabled="checking"
          @click="recheckPermissions"
        >
          {{ checking ? "Checking\u2026" : "Check Again" }}
        </button>
      </div>
    </div>
  </div>
</template>
