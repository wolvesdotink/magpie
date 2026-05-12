<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { WebviewWindow, getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
  checkPermissions,
  getDownloadedModels,
  getSettings,
  updateSettings,
  restartFnKeyMonitor,
  getFnKeyMonitorStatus,
} from '@/lib/commands';
import { onPermissionsStatus } from '@/lib/events';
import { useAppState } from '@/composables/useAppState';
import SetupWizard from '@/components/SetupWizard.vue';
import ModelPicker from '@/components/ModelPicker.vue';
import TrayPopover from '@/components/TrayPopover.vue';
import PermissionsGuide from '@/components/PermissionsGuide.vue';

type View = 'loading' | 'permissions' | 'setup' | 'model-picker' | 'main';

const currentView = ref<View>('loading');
const { hasModel } = useAppState();

// Listen for runtime permissions-status events (e.g. CGEventTap failure
// after a restart_fn_key_monitor call).  If accessibility drops, redirect
// to the permissions guide so the user can re-grant.
let unlistenPerms: (() => void) | null = null;

/**
 * Health-check the Fn monitor. If `is_active()` reports dead, try to
 * restart. Shared by the initial onMounted flow and the window-focus
 * listener so that a user noticing "Fn isn't working" and clicking the
 * tray icon triggers an immediate recovery attempt instead of having
 * to relaunch.
 */
async function checkFnMonitorHealth(): Promise<void> {
  if (currentView.value !== 'main') return;
  try {
    const active = await getFnKeyMonitorStatus();
    if (active) return;
    console.warn('Fn key monitor inactive — attempting restart');
    const restarted = await restartFnKeyMonitor();
    if (!restarted) {
      console.warn('Fn key monitor restart failed — showing permissions guide');
      currentView.value = 'permissions';
    }
  } catch (e) {
    console.error('Fn monitor health check failed:', e);
  }
}

onMounted(async () => {
  // Set up the permissions-status listener
  unlistenPerms = await onPermissionsStatus((status) => {
    // Any of the three permissions going false while the user is in the
    // main view means the Fn flow is broken — redirect to the guide.
    if ((!status.accessibility || !status.inputMonitoring) && currentView.value === 'main') {
      console.warn('Required permission lost — showing permissions guide', status);
      currentView.value = 'permissions';
    }
  });

  // Re-check Fn monitor health when the window regains focus. This catches
  // the case where the user notices Fn isn't working and clicks the tray
  // icon to open the popover. Pairs with the Rust-side watchdog that
  // restarts the monitor periodically if the tap-disabled auto-recover
  // path didn't fire.
  window.addEventListener('focus', checkFnMonitorHealth);

  try {
    const [settings, downloaded, perms] = await Promise.all([
      getSettings(),
      getDownloadedModels(),
      checkPermissions(),
    ]);

    if (!settings.setupComplete) {
      // Backward compat: existing user with a model already downloaded
      if (downloaded.length > 0 && hasModel.value) {
        // Silently mark setup as complete and continue
        await updateSettings({ ...settings, setupComplete: true });
        currentView.value = 'main';
      } else {
        // First launch — show the setup wizard
        currentView.value = 'setup';
      }
      return;
    }

    // Setup is complete — handle returning-user edge cases.
    // Input Monitoring is required for Fn key detection (CGEventTap).
    // Check all three permissions so we correctly catch returning users
    // upgrading from a build that didn't ask for Input Monitoring.
    if (!perms.accessibility || !perms.microphone || !perms.inputMonitoring) {
      currentView.value = 'permissions';
      return;
    }

    if (downloaded.length === 0 || !hasModel.value) {
      currentView.value = 'model-picker';
      return;
    }

    currentView.value = 'main';

    // Verify the Fn key monitor is actually running. `is_active()` now
    // accounts for the needs_restart flag set by the tap callback when
    // macOS silently disables the tap, so a `false` here means events
    // genuinely aren't flowing.
    await checkFnMonitorHealth();
  } catch (e) {
    console.error('Startup check failed:', e);
    currentView.value = 'setup';
  }
});

onUnmounted(() => {
  unlistenPerms?.();
  window.removeEventListener('focus', checkFnMonitorHealth);
});

async function onSetupComplete() {
  currentView.value = 'main';
  await getCurrentWebviewWindow().hide();
}

async function onPermissionsGranted() {
  // Restart Fn key monitor now that accessibility permission is granted
  await restartFnKeyMonitor();
  // If user already has a model loaded, go straight to main
  if (hasModel.value) {
    currentView.value = 'main';
  } else {
    currentView.value = 'model-picker';
  }
}

function onModelReady() {
  currentView.value = 'main';
}

async function openSettings() {
  const settingsWindow = await WebviewWindow.getByLabel('settings');
  if (settingsWindow) {
    await settingsWindow.show();
    await settingsWindow.setFocus();
  }
}
</script>

<template>
  <div class="w-full h-full overflow-hidden">
    <!-- Loading -->
    <div
      v-if="currentView === 'loading'"
      class="flex flex-col items-center justify-center h-full gap-4 bg-canvas rounded-xl"
    >
      <div class="relative">
        <div
          class="flex items-center justify-center w-12 h-12 rounded-xl bg-gradient-to-br from-gold via-gold to-gold-deep text-gold-ink font-extrabold text-[16px] shadow-glow-gold"
        >
          M
        </div>
        <div class="absolute -inset-2 rounded-2xl border-2 border-gold/25 animate-breathe" />
      </div>
      <p class="text-[11px] text-ink-muted font-medium tracking-tight">Starting Magpie…</p>
    </div>

    <SetupWizard v-else-if="currentView === 'setup'" @complete="onSetupComplete" />

    <PermissionsGuide v-else-if="currentView === 'permissions'" @granted="onPermissionsGranted" />

    <ModelPicker v-else-if="currentView === 'model-picker'" @model-ready="onModelReady" />

    <TrayPopover v-else-if="currentView === 'main'" @open-settings="openSettings" />
  </div>
</template>
