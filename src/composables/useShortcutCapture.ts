import { ref, onUnmounted } from 'vue';
import { buildShortcutString, hasModifier } from '@/lib/keyboard';

export interface ShortcutCaptureResult {
  shortcut: string;
}

/**
 * Capture a global keyboard shortcut. Calls `startCapture()` to install a
 * capture-phase keydown listener; on a valid press (any non-modifier key
 * with at least one modifier) it stops and resolves the promise. Escape
 * cancels capture without resolving.
 */
export function useShortcutCapture() {
  const capturing = ref(false);
  const error = ref<string | null>(null);

  let handler: ((e: KeyboardEvent) => void) | null = null;
  let pendingResolve: ((value: string | null) => void) | null = null;

  function stop() {
    capturing.value = false;
    if (handler) {
      window.removeEventListener('keydown', handler, true);
      handler = null;
    }
    if (pendingResolve) {
      pendingResolve(null);
      pendingResolve = null;
    }
  }

  function start(): Promise<string | null> {
    if (capturing.value) {
      stop();
    }
    error.value = null;
    capturing.value = true;

    return new Promise((resolve) => {
      pendingResolve = resolve;

      handler = (e: KeyboardEvent) => {
        // Capture-phase listener with stopPropagation prevents the keystroke
        // from landing in any focused input or triggering app shortcuts.
        e.preventDefault();
        e.stopPropagation();

        if (e.key === 'Meta' || e.key === 'Control' || e.key === 'Alt' || e.key === 'Shift') {
          return;
        }

        if (e.key === 'Escape') {
          stop();
          return;
        }

        const built = buildShortcutString(e);
        if (!built) {
          error.value = 'Could not interpret that key.';
          return;
        }

        if (!hasModifier(built)) {
          error.value = 'Must include a modifier (⌘, ⌃, ⌥ or ⇧).';
          return;
        }

        capturing.value = false;
        if (handler) {
          window.removeEventListener('keydown', handler, true);
          handler = null;
        }
        const resolveFn = pendingResolve;
        pendingResolve = null;
        if (resolveFn) resolveFn(built);
      };

      window.addEventListener('keydown', handler, true);
    });
  }

  onUnmounted(stop);

  return { capturing, error, start, stop };
}
