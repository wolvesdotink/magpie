import { ref, onMounted, onUnmounted } from 'vue';
import {
  getDownloadedModels,
  getDownloadedCorrectionModels,
  downloadModel,
  downloadCorrectionModel,
  selectModel,
  selectCorrectionModel,
  deleteModelFile,
  deleteCorrectionModelFile,
  cancelDownload,
} from '@/lib/commands';
import {
  onModelDownloadProgress,
  onModelDownloadComplete,
  onModelDownloadCancelled,
} from '@/lib/events';
import type { UnlistenFn } from '@tauri-apps/api/event';

export type ModelKind = 'whisper' | 'correction';

interface BindKindResult {
  list: () => Promise<string[]>;
  download: (id: string) => Promise<unknown>;
  select: (id: string) => Promise<unknown>;
  remove: (id: string) => Promise<unknown>;
}

function bindKind(kind: ModelKind): BindKindResult {
  if (kind === 'correction') {
    return {
      list: getDownloadedCorrectionModels,
      download: downloadCorrectionModel,
      select: selectCorrectionModel,
      remove: deleteCorrectionModelFile,
    };
  }
  return {
    list: getDownloadedModels,
    download: downloadModel,
    select: selectModel,
    remove: deleteModelFile,
  };
}

/**
 * Centralised state machine for whisper / correction model downloads.
 * Both ModelSection and TranscriptionSection use it (one instance per
 * kind). Listens for the three download events on Tauri's shared channel
 * and routes them only when the modelId matches our in-flight download.
 */
export function useModelDownloader(kind: ModelKind) {
  const cmds = bindKind(kind);

  const downloadedFiles = ref<string[]>([]);
  const downloading = ref(false);
  const progress = ref(0);
  const downloadingId = ref<string | null>(null);
  const error = ref<string | null>(null);

  const unlisteners: UnlistenFn[] = [];

  async function refresh() {
    downloadedFiles.value = await cmds.list();
  }

  function isDownloaded(filename: string): boolean {
    return downloadedFiles.value.includes(filename);
  }

  async function download(id: string, onComplete?: () => void | Promise<void>) {
    error.value = null;
    downloading.value = true;
    progress.value = 0;
    downloadingId.value = id;
    try {
      await cmds.download(id);
      // Auto-select after successful download — callers usually want this.
      await cmds.select(id);
      await onComplete?.();
    } catch (e) {
      if (!String(e).toLowerCase().includes('cancel')) {
        error.value = `Download failed: ${e}`;
      }
    } finally {
      downloading.value = false;
      downloadingId.value = null;
      await refresh();
    }
  }

  async function cancel() {
    if (!downloadingId.value) return;
    try {
      await cancelDownload(downloadingId.value);
    } catch (e) {
      console.error('Cancel failed:', e);
    }
  }

  async function select(id: string) {
    error.value = null;
    try {
      await cmds.select(id);
    } catch (e) {
      error.value = `Failed to load: ${e}`;
    }
  }

  async function remove(id: string) {
    try {
      await cmds.remove(id);
      await refresh();
    } catch (e) {
      error.value = `Delete failed: ${e}`;
    }
  }

  onMounted(async () => {
    await refresh();

    unlisteners.push(
      await onModelDownloadProgress((p) => {
        if (downloadingId.value === p.modelId) {
          progress.value = p.percent;
        }
      }),
    );
    unlisteners.push(
      await onModelDownloadComplete(async () => {
        if (!downloadingId.value) return;
        downloading.value = false;
        downloadingId.value = null;
        await refresh();
      }),
    );
    unlisteners.push(
      await onModelDownloadCancelled((data) => {
        if (downloadingId.value === data.modelId) {
          downloading.value = false;
          downloadingId.value = null;
          progress.value = 0;
          error.value = null;
        }
      }),
    );
  });

  onUnmounted(() => {
    unlisteners.forEach((u) => u());
  });

  return {
    downloadedFiles,
    downloading,
    progress,
    downloadingId,
    error,
    refresh,
    isDownloaded,
    download,
    cancel,
    select,
    remove,
  };
}
