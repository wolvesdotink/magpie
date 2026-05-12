<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import {
  checkPermissions,
  openAccessibilitySettings,
  requestMicrophonePermission,
  openMicrophoneSettings,
  restartFnKeyMonitor,
  restartApp,
} from '@/lib/commands';

const emit = defineEmits<{
  next: [];
}>();

const hasAccessibility = ref(false);
const hasMicrophone = ref(false);
const bothPermissionsGranted = computed(() => hasAccessibility.value && hasMicrophone.value);

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
        restartFnKeyMonitor();
        emit('next');
      }
    } catch (e) {
      console.error('Permission poll failed:', e);
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
      restartFnKeyMonitor();
      emit('next');
    }
  } catch (e) {
    console.error('Permission check failed:', e);
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
      restartFnKeyMonitor();
      emit('next');
    }
  } catch (e) {
    console.error('Microphone permission request failed:', e);
  }
  requestingMic.value = false;
}

async function handleOpenMicSettings() {
  await openMicrophoneSettings();
}

// Re-check immediately when the window regains focus (e.g. user returns
// from System Settings).
function onWindowFocus() {
  if (!bothPermissionsGranted.value) handleCheckPermissions();
}

onMounted(async () => {
  // Seed state from the current TCC values, then start polling if anything
  // is missing. The polling auto-advances via emit('next') once both perms
  // are granted, so the user doesn't need to click anything after enabling
  // them in System Settings.
  try {
    const perms = await checkPermissions();
    hasAccessibility.value = perms.accessibility;
    hasMicrophone.value = perms.microphone;
  } catch (e) {
    console.error('Initial permission check failed:', e);
  }
  if (!bothPermissionsGranted.value) startPermissionPolling();
  window.addEventListener('focus', onWindowFocus);
});

onUnmounted(() => {
  stopPermissionPolling();
  window.removeEventListener('focus', onWindowFocus);
});
</script>

<template>
  <div class="flex flex-col items-center px-6 pt-8 pb-6 gap-4 flex-1 overflow-y-auto">
    <!-- Shield Icon -->
    <div
      class="flex items-center justify-center w-14 h-14 rounded-2xl bg-gradient-to-br from-gold/15 to-gold/5 text-gold shadow-glow-gold"
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
        <path d="M9 12l2 2 4-4" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </div>

    <!-- Title -->
    <div class="text-center">
      <h2 class="text-[15px] font-bold tracking-tight text-ink">Permissions Required</h2>
      <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 max-w-[240px]">
        Magpie needs two permissions to work: microphone access for recording and accessibility for
        detecting the Fn key.
      </p>
    </div>

    <!-- Permission cards -->
    <div class="w-full flex flex-col gap-3">
      <!-- ── Microphone card ── -->
      <div
        class="w-full p-3.5 rounded-lg border transition-colors duration-200"
        :class="hasMicrophone ? 'bg-leaf/[0.06] border-leaf/25' : 'bg-panel border-edge'"
      >
        <div class="flex items-center justify-between gap-3 mb-2">
          <div class="flex items-center gap-2.5">
            <div
              class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
              :class="hasMicrophone ? 'bg-leaf/15 text-leaf' : 'bg-raised text-ink-faint'"
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
              <span class="text-[12px] font-semibold text-ink"> Microphone </span>
              <p class="text-[9px] text-ink-faint leading-snug mt-0.5">
                Record your voice for transcription
              </p>
            </div>
          </div>
          <span
            v-if="hasMicrophone"
            class="flex items-center gap-1 text-[9px] font-bold text-leaf uppercase tracking-wider flex-shrink-0"
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
        <div v-if="!hasMicrophone" class="mt-2">
          <button
            v-if="!micDenied"
            class="w-full py-2 rounded-md text-[11px] font-semibold transition-all duration-200 active:scale-[0.97] bg-gradient-to-b from-gold to-gold-hover text-gold-ink hover:from-gold-hover hover:to-gold-deep shadow-press hover:shadow-lifted"
            :disabled="requestingMic"
            @click="handleGrantMicrophone"
          >
            {{ requestingMic ? 'Waiting…' : 'Grant Microphone Access' }}
          </button>
          <div v-else class="flex flex-col gap-1.5">
            <p class="text-[9px] text-flame leading-snug">
              Microphone access was denied. Please enable it in System Settings.
            </p>
            <button
              class="w-full py-2 rounded-md text-[11px] font-semibold transition-all duration-200 active:scale-[0.97] bg-panel border border-edge text-ink-muted shadow-soft hover:bg-raised hover:border-edge-strong"
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
        :class="hasAccessibility ? 'bg-leaf/[0.06] border-leaf/25' : 'bg-panel border-edge'"
      >
        <div class="flex items-center justify-between gap-3 mb-2">
          <div class="flex items-center gap-2.5">
            <div
              class="flex items-center justify-center w-7 h-7 rounded-lg flex-shrink-0"
              :class="hasAccessibility ? 'bg-leaf/15 text-leaf' : 'bg-raised text-ink-faint'"
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
              <span class="text-[12px] font-semibold text-ink"> Accessibility </span>
              <p class="text-[9px] text-ink-faint leading-snug mt-0.5">
                Detect Fn key and paste transcribed text
              </p>
            </div>
          </div>
          <span
            v-if="hasAccessibility"
            class="flex items-center gap-1 text-[9px] font-bold text-leaf uppercase tracking-wider flex-shrink-0"
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
        <div v-if="!hasAccessibility" class="flex flex-col gap-3 mt-2">
          <div class="flex flex-col gap-2 p-2.5 bg-raised/50 rounded-md">
            <div class="flex items-center gap-2">
              <span
                class="flex items-center justify-center w-4 h-4 rounded-full bg-gold/20 text-gold text-[8px] font-bold flex-shrink-0"
                >1</span
              >
              <span class="text-[9px] text-ink-muted leading-snug">
                Click "Open Settings" below
              </span>
            </div>
            <div class="flex items-center gap-2">
              <span
                class="flex items-center justify-center w-4 h-4 rounded-full bg-gold/20 text-gold text-[8px] font-bold flex-shrink-0"
                >2</span
              >
              <span class="text-[9px] text-ink-muted leading-snug">
                Enable <strong class="text-ink font-semibold">Magpie</strong>
              </span>
            </div>
          </div>
          <button
            class="w-full py-2 rounded-md text-[11px] font-semibold transition-all duration-200 active:scale-[0.97] bg-gradient-to-b from-gold to-gold-hover text-gold-ink hover:from-gold-hover hover:to-gold-deep shadow-press hover:shadow-lifted"
            @click="handleOpenAccessibilitySettings"
          >
            Open Accessibility Settings
          </button>
          <button
            class="w-full py-2 rounded-md text-[11px] font-semibold transition-all duration-200 active:scale-[0.97] bg-panel border border-edge text-ink-muted shadow-soft hover:bg-raised hover:border-edge-strong"
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
        class="w-full py-2.5 rounded-lg text-[13px] font-semibold transition-all duration-200 active:scale-[0.97] bg-gradient-to-b from-gold to-gold-hover text-gold-ink hover:from-gold-hover hover:to-gold-deep shadow-press hover:shadow-lifted"
        @click="$emit('next')"
      >
        Continue
      </button>
      <button
        v-else
        class="w-full py-2.5 rounded-lg text-[13px] font-semibold transition-all duration-200 active:scale-[0.97] bg-panel border border-edge text-ink-muted shadow-soft hover:bg-raised hover:border-edge-strong hover:shadow-lifted"
        :disabled="checkingPermissions"
        @click="handleCheckPermissions"
      >
        {{ checkingPermissions ? 'Checking…' : 'Check Again' }}
      </button>
    </div>
  </div>
</template>
