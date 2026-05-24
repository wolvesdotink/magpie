/**
 * useUpdater — channel-aware in-app updater state machine for Vue 3.
 *
 * Talks to our Rust-side `magpie_updater_*` commands (NOT the
 * `@tauri-apps/plugin-updater` JS API), because the channel choice —
 * stable vs. beta — has to happen in Rust per call: the plugin's global
 * Builder doesn't accept endpoints, only `tauri.conf.json` does. Our
 * commands always call `app.updater_builder().endpoints(...)` with the
 * URL matching the user's `UserSettings.update_channel`.
 *
 * Lifecycle:
 *   idle → checking → available → downloading → ready
 *                  ↘ idle (no update)        ↘ error (any failure)
 *
 * Listens for `menu://check-for-updates` from the tray menu so a user
 * clicking "Check for Updates…" triggers `checkNow()`.
 *
 * Dev-mode behavior:
 *   The Rust commands need a real signed bundle + valid pubkey to do
 *   anything useful. In `pnpm tauri dev` and plain Vite previews every
 *   call errors; we surface that as `status: 'error'` rather than
 *   crashing.
 */
import { onMounted, onUnmounted, ref } from 'vue';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { magpieUpdaterCheck, magpieUpdaterInstall } from '@/lib/commands';

type UpdaterStatus = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'error';

export type UpdaterState = {
  status: UpdaterStatus;
  /** Version string of the available/installed update (e.g. "0.2.0" or "0.2.0-beta.3"). */
  newVersion: string | null;
  /** Release notes / changelog markdown, if the manifest provided one. */
  notes: string | null;
  /** Bytes downloaded so far (during `downloading`). */
  downloaded: number;
  /** Total bytes to download (during `downloading`). 0 until first chunk. */
  totalBytes: number;
  /** Last error message; only meaningful when status === "error". */
  error: string | null;
};

const initialState: UpdaterState = {
  status: 'idle',
  newVersion: null,
  notes: null,
  downloaded: 0,
  totalBytes: 0,
  error: null,
};

/** Wait this long after mount before the first silent check (ms). */
const BOOT_QUIET_MS = 4000;

type ProgressEvent = {
  chunkLength: number;
  contentLength: number | null;
};

type UseUpdaterOptions = {
  /** Run a silent check shortly after mount. Default true. */
  bootCheck?: boolean;
  /** Listen for the `menu://check-for-updates` tray event. Default true. */
  listenMenu?: boolean;
};

export function useUpdater(options: UseUpdaterOptions = {}) {
  const { bootCheck = true, listenMenu = true } = options;

  const state = ref<UpdaterState>({ ...initialState });
  const dismissed = ref(false);

  let bootTimer: number | null = null;
  let unlistenMenu: UnlistenFn | null = null;
  let unlistenProgress: UnlistenFn | null = null;
  let unlistenFinish: UnlistenFn | null = null;

  async function checkNow(): Promise<void> {
    dismissed.value = false;
    state.value = { ...state.value, status: 'checking', error: null };
    try {
      const result = await magpieUpdaterCheck();
      if (!result) {
        state.value = { ...initialState, status: 'idle' };
        return;
      }
      state.value = {
        status: 'available',
        newVersion: result.version,
        notes: result.body,
        downloaded: 0,
        totalBytes: 0,
        error: null,
      };
    } catch (e) {
      // Most common reason in production: no network. Most common in dev:
      // signed bundle / pubkey not present.
      state.value = {
        ...initialState,
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      };
    }
  }

  async function install(): Promise<void> {
    state.value = {
      ...state.value,
      status: 'downloading',
      downloaded: 0,
      totalBytes: 0,
    };
    try {
      const { listen } = await import('@tauri-apps/api/event');
      // Subscribe to progress + finish events for the duration of this
      // install. The Rust `magpie_updater_install` command emits per-chunk
      // progress and a single `finished` event when the byte stream ends
      // (before signature verification + extract — see useUpdater notes
      // below; we don't transition to 'ready' on 'finished' for that
      // reason).
      unlistenProgress = await listen<ProgressEvent>('magpie://updater-progress', ({ payload }) => {
        const chunk = payload.chunkLength ?? 0;
        const total = payload.contentLength ?? 0;
        state.value = {
          ...state.value,
          downloaded: state.value.downloaded + chunk,
          // First progress event carries the total; preserve any
          // previously-known total if a later event reports null.
          totalBytes: total > 0 ? total : state.value.totalBytes,
        };
      });
      unlistenFinish = await listen('magpie://updater-finished', () => {
        // No state transition here: 'finished' fires the moment the byte
        // stream ends, BEFORE signature verification + extract. We use the
        // command promise's resolution as the single source of truth for
        // success vs. failure.
      });

      await magpieUpdaterInstall();
      state.value = { ...state.value, status: 'ready' };
    } catch (e) {
      console.error('[updater] install failed:', e);
      state.value = {
        ...state.value,
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      };
    } finally {
      unlistenProgress?.();
      unlistenProgress = null;
      unlistenFinish?.();
      unlistenFinish = null;
    }
  }

  async function restart(): Promise<void> {
    try {
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (e) {
      state.value = {
        ...state.value,
        status: 'error',
        error: e instanceof Error ? e.message : String(e),
      };
    }
  }

  function dismiss(): void {
    dismissed.value = true;
  }

  onMounted(async () => {
    if (bootCheck) {
      bootTimer = window.setTimeout(() => {
        void checkNow();
      }, BOOT_QUIET_MS);
    }

    if (listenMenu) {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        unlistenMenu = await listen('menu://check-for-updates', () => {
          void checkNow();
        });
      } catch (e) {
        // Outside Tauri runtime (e.g. plain Vite preview) — listening is a no-op.
        console.debug('[updater] menu listener not registered:', e);
      }
    }
  });

  onUnmounted(() => {
    if (bootTimer !== null) {
      window.clearTimeout(bootTimer);
      bootTimer = null;
    }
    if (unlistenMenu) {
      unlistenMenu();
      unlistenMenu = null;
    }
    unlistenProgress?.();
    unlistenProgress = null;
    unlistenFinish?.();
    unlistenFinish = null;
  });

  return {
    state,
    dismissed,
    checkNow,
    install,
    restart,
    dismiss,
  };
}
