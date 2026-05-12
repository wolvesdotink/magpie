<script setup lang="ts">
import { ref, computed } from 'vue';
import {
  openAccessibilitySettings,
  requestMicrophonePermission,
  openMicrophoneSettings,
  restartFnKeyMonitor,
  restartApp,
} from '@/lib/commands';
import { usePermissionPoller } from '@/composables/usePermissionPoller';
import PermissionCard from '@/components/shared/PermissionCard.vue';
import BaseButton from '@/components/base/BaseButton.vue';

const emit = defineEmits<{
  next: [];
}>();

const checkingPermissions = ref(false);
const requestingMic = ref(false);
const micDenied = ref(false);

const { hasAccessibility, hasMicrophone, refresh, start, stop } = usePermissionPoller({
  onChange: (perms) => {
    if (perms.accessibility && perms.microphone) {
      stop();
      restartFnKeyMonitor();
      emit('next');
    }
  },
});

const bothPermissionsGranted = computed(() => hasAccessibility.value && hasMicrophone.value);

async function handleOpenAccessibilitySettings() {
  await openAccessibilitySettings();
  start();
}

async function handleCheckPermissions() {
  checkingPermissions.value = true;
  await refresh();
  checkingPermissions.value = false;
}

async function handleGrantMicrophone() {
  requestingMic.value = true;
  try {
    const granted = await requestMicrophonePermission();
    hasMicrophone.value = granted;
    micDenied.value = !granted;
    if (bothPermissionsGranted.value) {
      stop();
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

// Kick off polling if anything is missing after the initial refresh.
refresh().then(() => {
  if (!bothPermissionsGranted.value) start();
});
</script>

<template>
  <div class="flex flex-col items-center px-6 pt-8 pb-6 gap-4 flex-1 overflow-y-auto">
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
        Magpie needs two permissions to work: microphone access for recording and accessibility for
        detecting the Fn key.
      </p>
    </div>

    <div class="w-full flex flex-col gap-3">
      <PermissionCard
        label="Microphone"
        description="Record your voice for transcription"
        :granted="hasMicrophone"
      >
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

      <PermissionCard
        label="Accessibility"
        description="Detect Fn key and paste transcribed text"
        :granted="hasAccessibility"
      >
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
          <div class="flex flex-col gap-3">
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
            <BaseButton
              variant="primary"
              size="md"
              full-width
              @click="handleOpenAccessibilitySettings"
            >
              Open Accessibility Settings
            </BaseButton>
            <BaseButton size="md" full-width @click="restartApp">
              Already enabled? Restart Magpie
            </BaseButton>
          </div>
        </template>
      </PermissionCard>
    </div>

    <div class="w-full flex flex-col gap-2">
      <BaseButton
        v-if="bothPermissionsGranted"
        variant="primary"
        size="lg"
        full-width
        @click="$emit('next')"
      >
        Continue
      </BaseButton>
      <BaseButton
        v-else
        size="lg"
        full-width
        :disabled="checkingPermissions"
        @click="handleCheckPermissions"
      >
        {{ checkingPermissions ? 'Checking…' : 'Check Again' }}
      </BaseButton>
    </div>
  </div>
</template>
