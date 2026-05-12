<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  openAccessibilitySettings,
  requestMicrophonePermission,
  openMicrophoneSettings,
  requestInputMonitoringPermission,
  openInputMonitoringSettings,
  restartFnKeyMonitor,
  restartApp,
} from '@/lib/commands';
import { usePermissionPoller } from '@/composables/usePermissionPoller';
import PermissionCard from '@/components/shared/PermissionCard.vue';
import BaseButton from '@/components/base/BaseButton.vue';

const emit = defineEmits<{
  granted: [];
}>();

const checking = ref(false);
const requestingMic = ref(false);
const requestingInputMon = ref(false);
const micDenied = ref(false);

// When Input Monitoring transitions OFF → ON the Fn key monitor must be
// restarted — the CGEventTap created during startup was rejected and
// needs to be recreated now that permission is granted.
let lastInputMonitoring = false;
async function maybeRestartFnMonitor(next: boolean) {
  if (next && !lastInputMonitoring) {
    try {
      await restartFnKeyMonitor();
    } catch (e) {
      console.error('Failed to restart Fn key monitor:', e);
    }
  }
  lastInputMonitoring = next;
}

const { hasAccessibility, hasMicrophone, hasInputMonitoring, refresh, start, stop } =
  usePermissionPoller({
    onChange: async (perms) => {
      await maybeRestartFnMonitor(perms.inputMonitoring);
      if (perms.accessibility && perms.microphone && perms.inputMonitoring) {
        stop();
        emit('granted');
      }
    },
  });

const allGranted = computed(
  () => hasAccessibility.value && hasMicrophone.value && hasInputMonitoring.value,
);

// Kick off polling if anything is missing.
refresh().then(() => {
  lastInputMonitoring = hasInputMonitoring.value;
  if (allGranted.value) {
    emit('granted');
  } else {
    start();
  }
});

async function openSettings() {
  await openAccessibilitySettings();
  start();
}

async function handleRequestInputMonitoring() {
  requestingInputMon.value = true;
  try {
    const granted = await requestInputMonitoringPermission();
    hasInputMonitoring.value = granted;
    if (granted) {
      await maybeRestartFnMonitor(true);
    } else {
      await openInputMonitoringSettings();
      start();
    }
    if (allGranted.value) {
      stop();
      emit('granted');
    }
  } catch (e) {
    console.error('Input Monitoring request failed:', e);
  }
  requestingInputMon.value = false;
}

async function handleOpenInputMonitoringSettings() {
  await openInputMonitoringSettings();
  start();
}

async function handleGrantMicrophone() {
  requestingMic.value = true;
  try {
    const granted = await requestMicrophonePermission();
    hasMicrophone.value = granted;
    micDenied.value = !granted;
    if (allGranted.value) {
      stop();
      emit('granted');
    }
  } catch (e) {
    console.error('Microphone permission request failed:', e);
  }
  requestingMic.value = false;
}

async function handleOpenMicSettings() {
  await openMicrophoneSettings();
}

async function recheckPermissions() {
  checking.value = true;
  await refresh();
  checking.value = false;
}
</script>

<template>
  <div class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden">
    <div class="h-px bg-gradient-to-r from-transparent via-gold/20 to-transparent" />

    <div class="flex flex-col items-center px-6 pt-8 pb-6 gap-4 flex-1">
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

      <div class="text-center">
        <h2 class="text-[15px] font-bold tracking-tight text-ink">Permissions Required</h2>
        <p class="text-[10px] text-ink-muted leading-relaxed mt-1.5 max-w-[240px]">
          Magpie needs the following permissions to function. Please grant any missing ones below.
        </p>
      </div>

      <div class="w-full flex flex-col gap-3">
        <PermissionCard label="Microphone" :granted="hasMicrophone">
          <template #icon>
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
          </template>
          <template #actions>
            <BaseButton
              v-if="!micDenied"
              variant="primary"
              size="md"
              full-width
              :disabled="requestingMic"
              @click="handleGrantMicrophone"
            >
              {{ requestingMic ? 'Waiting…' : 'Grant Microphone Access' }}
            </BaseButton>
            <div v-else class="flex flex-col gap-1.5">
              <p class="text-[9px] text-flame leading-snug">
                Microphone access was denied. Please enable it in System Settings.
              </p>
              <BaseButton size="md" full-width @click="handleOpenMicSettings">
                Open Microphone Settings
              </BaseButton>
            </div>
          </template>
        </PermissionCard>

        <PermissionCard label="Accessibility" :granted="hasAccessibility">
          <template #icon>
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
          </template>
          <template #actions>
            <div class="flex flex-col gap-1.5">
              <BaseButton variant="primary" size="md" full-width @click="openSettings">
                Open Accessibility Settings
              </BaseButton>
              <BaseButton size="md" full-width @click="restartApp">
                Already enabled? Restart Magpie
              </BaseButton>
            </div>
          </template>
        </PermissionCard>

        <PermissionCard label="Input Monitoring" :granted="hasInputMonitoring">
          <template #icon>
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
              <rect x="2" y="6" width="20" height="12" rx="2" />
              <line x1="6" y1="10" x2="6" y2="10" />
              <line x1="10" y1="10" x2="10" y2="10" />
              <line x1="14" y1="10" x2="14" y2="10" />
              <line x1="18" y1="10" x2="18" y2="10" />
              <line x1="7" y1="14" x2="17" y2="14" />
            </svg>
          </template>
          <template #actions>
            <div class="flex flex-col gap-1.5">
              <p class="text-[9px] text-ink-muted leading-snug">
                Needed to detect the Fn key. On macOS this is a separate pane from Accessibility.
              </p>
              <BaseButton
                variant="primary"
                size="md"
                full-width
                :disabled="requestingInputMon"
                @click="handleRequestInputMonitoring"
              >
                {{ requestingInputMon ? 'Waiting…' : 'Grant Input Monitoring' }}
              </BaseButton>
              <BaseButton size="md" full-width @click="handleOpenInputMonitoringSettings">
                Open Input Monitoring Settings
              </BaseButton>
            </div>
          </template>
        </PermissionCard>
      </div>

      <div class="w-full flex flex-col gap-2 mt-auto">
        <BaseButton size="lg" full-width :disabled="checking" @click="recheckPermissions">
          {{ checking ? 'Checking…' : 'Check Again' }}
        </BaseButton>
      </div>
    </div>
  </div>
</template>
