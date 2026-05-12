import { ref, onMounted, onUnmounted } from 'vue';
import { checkPermissions } from '@/lib/commands';

interface PollerOptions {
  /** Poll interval in ms. Default 2000. */
  intervalMs?: number;
  /** Called when permissions update. */
  onChange?: (perms: {
    accessibility: boolean;
    microphone: boolean;
    inputMonitoring: boolean;
  }) => void;
}

/**
 * Polls the OS permission state every `intervalMs` and on window focus.
 * Maintains three reactive refs that consumers can read. Stop polling with
 * `stop()`; the composable also stops on unmount.
 *
 * Used by PermissionsGuide (3 permissions) and setup/PermissionsStep
 * (2 permissions — ignores `inputMonitoring`).
 */
export function usePermissionPoller(opts: PollerOptions = {}) {
  const intervalMs = opts.intervalMs ?? 2000;
  const hasAccessibility = ref(false);
  const hasMicrophone = ref(false);
  const hasInputMonitoring = ref(false);

  let timer: ReturnType<typeof setInterval> | null = null;

  async function refresh() {
    try {
      const perms = await checkPermissions();
      hasAccessibility.value = perms.accessibility;
      hasMicrophone.value = perms.microphone;
      hasInputMonitoring.value = perms.inputMonitoring;
      opts.onChange?.(perms);
    } catch (e) {
      console.error('Permission check failed:', e);
    }
  }

  function start() {
    stop();
    timer = setInterval(refresh, intervalMs);
  }

  function stop() {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  function onFocus() {
    refresh();
  }

  onMounted(() => {
    refresh();
    window.addEventListener('focus', onFocus);
  });

  onUnmounted(() => {
    stop();
    window.removeEventListener('focus', onFocus);
  });

  return {
    hasAccessibility,
    hasMicrophone,
    hasInputMonitoring,
    refresh,
    start,
    stop,
  };
}
