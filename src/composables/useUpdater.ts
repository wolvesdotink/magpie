/**
 * useUpdater — Tauri auto-update state machine for Vue 3.
 *
 * Lifecycle:
 *   idle → checking → available → downloading → ready
 *                  ↘ idle (no update)        ↘ error (any failure)
 *
 * The composable also listens for `menu://check-for-updates` from the tray
 * menu so a user clicking "Check for Updates…" triggers `checkNow()`.
 *
 * Dev-mode behavior:
 *   The updater plugin needs a signed bundle with a valid pubkey to do
 *   anything. In `bun tauri dev` and in plain Vite previews, every call
 *   throws (or the pubkey placeholder fails to parse). We catch those
 *   throws and stay in `idle`/`error` silently — there's nothing useful
 *   the user can do about it.
 *
 * The endpoint, public key, and version checks are all configured in
 * src-tauri/tauri.conf.json under `plugins.updater`. This composable does
 * NOT know the URL — that's baked into the binary at build time.
 */
import { onMounted, onUnmounted, ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";

type UpdaterStatus =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export type UpdaterState = {
  status: UpdaterStatus;
  /** Version string of the available/installed update (e.g. "0.2.0"). */
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
  status: "idle",
  newVersion: null,
  notes: null,
  downloaded: 0,
  totalBytes: 0,
  error: null,
};

/** Wait this long after mount before the first silent check (ms). */
const BOOT_QUIET_MS = 4000;

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

  /** Holds the Update handle returned by `check()` so install/restart can use it. */
  let updateHandle: Awaited<
    ReturnType<typeof import("@tauri-apps/plugin-updater").check>
  > | null = null;

  let bootTimer: number | null = null;
  let unlistenMenu: UnlistenFn | null = null;

  async function checkNow(): Promise<void> {
    dismissed.value = false;
    state.value = { ...state.value, status: "checking", error: null };
    try {
      // Lazy import — keeps the module out of the dev/browser bundle path
      // on first paint and lets the catch below cover the "plugin not
      // present" case cleanly.
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (!update) {
        updateHandle = null;
        state.value = { ...initialState, status: "idle" };
        return;
      }
      updateHandle = update;
      state.value = {
        status: "available",
        newVersion: update.version ?? null,
        notes: update.body ?? null,
        downloaded: 0,
        totalBytes: 0,
        error: null,
      };
    } catch (e) {
      // Most common reason in production: no network. Most common in dev:
      // plugin not active or pubkey not yet generated.
      updateHandle = null;
      state.value = {
        ...initialState,
        status: "error",
        error: (e as Error).message ?? String(e),
      };
    }
  }

  async function install(): Promise<void> {
    if (!updateHandle) return;
    state.value = {
      ...state.value,
      status: "downloading",
      downloaded: 0,
      totalBytes: 0,
    };
    try {
      await updateHandle.downloadAndInstall((event) => {
        // The plugin emits one of three event shapes per the Tauri 2 docs:
        //   { event: "Started",  data: { contentLength } }
        //   { event: "Progress", data: { chunkLength    } }
        //   { event: "Finished" }
        //
        // We deliberately do NOT transition to "ready" on "Finished".
        // Finished fires the moment the byte stream ends — BEFORE signature
        // verification and BEFORE the install step that extracts the .tar.gz
        // and replaces the running .app bundle. Either of those can fail; we
        // use the promise resolution below as the single source of truth.
        if (event.event === "Started") {
          const total =
            (event.data as { contentLength?: number }).contentLength ?? 0;
          state.value = { ...state.value, totalBytes: total };
        } else if (event.event === "Progress") {
          const chunk =
            (event.data as { chunkLength?: number }).chunkLength ?? 0;
          state.value = {
            ...state.value,
            downloaded: state.value.downloaded + chunk,
          };
        }
      });
      state.value = { ...state.value, status: "ready" };
    } catch (e) {
      console.error("[updater] install failed:", e);
      state.value = {
        ...state.value,
        status: "error",
        error: (e as Error).message ?? String(e),
      };
    }
  }

  async function restart(): Promise<void> {
    try {
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      state.value = {
        ...state.value,
        status: "error",
        error: (e as Error).message ?? String(e),
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
        const { listen } = await import("@tauri-apps/api/event");
        unlistenMenu = await listen("menu://check-for-updates", () => {
          void checkNow();
        });
      } catch (e) {
        // Outside Tauri runtime (e.g. plain Vite preview) — listening is a no-op.
        console.debug("[updater] menu listener not registered:", e);
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
