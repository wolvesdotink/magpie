<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { useAppState } from "@/composables/useAppState";
import { toggleRecording } from "@/lib/commands";
import { onVocabularyLearned } from "@/lib/events";
import LanguageSelector from "@/components/LanguageSelector.vue";
import type { UnlistenFn } from "@tauri-apps/api/event";

defineEmits<{
  openSettings: [];
}>();

const { recording, processing, hasModel, lastTranscription, error } =
  useAppState();

// ── Vocabulary toast ──
const vocabToast = ref<{ wrong: string; correct: string } | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;
const unlisteners: UnlistenFn[] = [];

onMounted(async () => {
  unlisteners.push(
    await onVocabularyLearned((data) => {
      vocabToast.value = { wrong: data.wrong, correct: data.correct };
      if (toastTimer) clearTimeout(toastTimer);
      toastTimer = setTimeout(() => {
        vocabToast.value = null;
      }, 3000);
    }),
  );
});

onUnmounted(() => {
  unlisteners.forEach((u) => u());
  if (toastTimer) clearTimeout(toastTimer);
});

async function handleToggle() {
  try {
    await toggleRecording();
  } catch (e) {
    console.error("Toggle recording failed:", e);
  }
}
</script>

<template>
  <div class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden shadow-elevated relative surface-grain">
    <!-- Decorative top edge highlight -->
    <div class="h-[1.5px] bg-gradient-to-r from-transparent via-gold/40 to-transparent" />

    <div class="flex flex-col flex-1 px-5 pt-5 pb-4 gap-4 min-h-0">
      <!-- ── Header ── -->
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2">
          <div
            class="flex items-center justify-center w-7 h-7 rounded-lg
                   bg-gradient-to-br from-gold to-gold-deep
                   text-gold-ink font-extrabold text-[13px] tracking-tight
                   shadow-press"
          >
            M
          </div>
          <span class="font-bold text-[15px] tracking-tight text-ink">
            Magpie
          </span>
        </div>
        <div class="flex items-center gap-2">
          <button
            class="flex items-center justify-center w-6 h-6 rounded-md
                   text-ink-faint hover:text-ink-muted hover:bg-raised
                   transition-all duration-150 active:scale-90"
            title="Settings"
            @click="$emit('openSettings')"
          >
            <svg
              width="13"
              height="13"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            >
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
            </svg>
          </button>
          <div
            class="w-2 h-2 rounded-full transition-all duration-300"
            :class="{
              'bg-leaf shadow-[0_0_6px_rgba(95,183,96,0.5)]':
                !recording && !processing,
              'bg-flame shadow-[0_0_6px_rgba(232,90,79,0.5)] animate-pulse':
                recording,
              'bg-gold shadow-[0_0_6px_rgba(232,175,71,0.5)] animate-pulse':
                processing && !recording,
            }"
          />
        </div>
      </div>

      <!-- ── Status Panel ── -->
      <div
        class="relative p-3.5 rounded-lg border transition-all duration-300"
        :class="{
          'bg-flame/[0.06] border-flame/30 shadow-glow-flame': recording,
          'bg-panel border-edge shadow-soft': !recording,
        }"
      >
        <!-- Recording -->
        <div v-if="recording" class="flex items-center gap-2.5">
          <div class="relative flex-shrink-0 w-2.5 h-2.5">
            <div class="absolute inset-0 rounded-full bg-flame" />
            <div class="absolute inset-0 rounded-full bg-flame animate-ping" />
          </div>
          <span class="text-[13px] font-semibold text-flame">
            Recording… release Fn to stop
          </span>
        </div>

        <!-- Processing -->
        <div v-else-if="processing" class="flex items-center gap-2.5">
          <div
            class="w-3.5 h-3.5 rounded-full border-2 border-edge
                   border-t-gold animate-spin flex-shrink-0"
          />
          <span class="text-[13px] font-semibold text-gold">
            Transcribing…
          </span>
        </div>

        <!-- No model -->
        <div v-else-if="!hasModel" class="flex items-center gap-2.5">
          <div class="w-1.5 h-1.5 rounded-full bg-ink-faint flex-shrink-0" />
          <span class="text-[13px] text-ink-faint">No model loaded</span>
        </div>

        <!-- Ready -->
        <div v-else class="flex items-center gap-2.5">
          <div class="w-1.5 h-1.5 rounded-full bg-leaf flex-shrink-0" />
          <span class="text-[13px] text-ink-muted">
            Ready — hold
            <kbd
              class="inline-flex items-center px-1.5 py-0.5 mx-0.5
                     text-[10px] font-mono font-semibold leading-none
                     bg-raised rounded border border-edge shadow-soft text-ink-faint"
            >
              Fn
            </kbd>
            to dictate
          </span>
        </div>
      </div>

      <!-- ── Record Button ── -->
      <button
        class="flex items-center justify-center gap-2 w-full py-3
               rounded-lg text-[13px] font-semibold
               transition-all duration-200 active:scale-[0.97]"
        :class="
          recording
            ? 'bg-gradient-to-b from-flame to-flame-hover text-white shadow-glow-flame animate-glow'
            : 'bg-gradient-to-b from-raised to-hover border border-edge-strong text-ink shadow-press hover:shadow-lifted hover:from-hover hover:to-subtle'
        "
        :disabled="processing || !hasModel"
        @click="handleToggle"
      >
        <!-- Microphone icon -->
        <svg
          v-if="!recording"
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="currentColor"
          class="opacity-70"
        >
          <path
            d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"
          />
          <path
            d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5H5c0 3.53 2.61 6.43 6 6.92V21h2v-3.08c3.39-.49 6-3.39 6-6.92h-2z"
          />
        </svg>
        <!-- Stop icon -->
        <svg
          v-else
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="currentColor"
        >
          <rect x="6" y="6" width="12" height="12" rx="2" />
        </svg>
        {{ recording ? "Stop" : "Record" }}
      </button>

      <!-- ── Last Transcription ── -->
      <div
        v-if="lastTranscription"
        class="flex-1 flex flex-col gap-1.5 min-h-0"
      >
        <span
          class="text-[10px] uppercase tracking-[0.08em] font-semibold text-ink-faint"
        >
          Last transcription
        </span>
        <div
          class="flex-1 p-3.5 text-[13px] leading-relaxed text-ink-muted
                 bg-panel rounded-lg border border-edge shadow-well overflow-y-auto"
        >
          {{ lastTranscription }}
        </div>
      </div>

      <!-- ── Empty State ── -->
      <div
        v-else-if="!error"
        class="flex-1 flex flex-col items-center justify-center gap-2.5
               bg-panel/50 rounded-lg border border-dashed border-edge/60 shadow-well"
      >
        <svg
          width="28"
          height="28"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.2"
          class="text-ink-faint/40"
        >
          <path
            d="M12 14c1.66 0 3-1.34 3-3V5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3z"
            stroke-linecap="round"
          />
          <path d="M17 11c0 2.76-2.24 5-5 5s-5-2.24-5-5" stroke-linecap="round" />
          <line x1="12" y1="19" x2="12" y2="22" stroke-linecap="round" />
          <line x1="9" y1="22" x2="15" y2="22" stroke-linecap="round" />
        </svg>
        <span class="text-[11px] text-ink-faint/50 font-medium">
          Transcriptions appear here
        </span>
      </div>

      <!-- ── Error ── -->
      <div
        v-if="error"
        class="p-2.5 rounded-lg bg-flame/10 border border-flame/20"
      >
        <span class="text-[10px] text-flame">{{ error }}</span>
      </div>
    </div>

    <!-- ── Vocabulary Learned Toast ── -->
    <Transition name="toast">
      <div
        v-if="vocabToast"
        class="absolute top-3 left-1/2 -translate-x-1/2 z-50
               flex items-center gap-2 px-3 py-1.5 rounded-lg
               bg-gold/15 border border-gold/30 shadow-soft backdrop-blur-sm"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="text-gold flex-shrink-0"
        >
          <path d="M4 19.5A2.5 2.5 0 016.5 17H20" />
          <path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z" />
        </svg>
        <span class="text-[10px] text-gold font-semibold">
          Learned: {{ vocabToast.wrong }}
        </span>
        <svg
          width="8"
          height="8"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="3"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="text-gold/60 flex-shrink-0"
        >
          <line x1="5" y1="12" x2="19" y2="12" />
          <polyline points="12 5 19 12 12 19" />
        </svg>
        <span class="text-[10px] text-gold font-bold">
          {{ vocabToast.correct }}
        </span>
      </div>
    </Transition>

    <!-- ── Footer ── -->
    <div class="px-5 py-3 border-t border-edge bg-raised/40">
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-1.5 text-[11px] text-ink-faint">
          <kbd
            class="inline-flex items-center px-1 py-px text-[9px] font-mono font-semibold
                   leading-none bg-raised rounded border border-edge shadow-soft"
          >
            Fn
          </kbd>
          <span>hold to dictate</span>
          <span class="mx-0.5 text-edge-strong">·</span>
          <kbd
            class="inline-flex items-center px-1 py-px text-[9px] font-mono font-semibold
                   leading-none bg-raised rounded border border-edge shadow-soft"
          >
            ⌘⇧Space
          </kbd>
        </div>
        <LanguageSelector />
      </div>
    </div>
  </div>
</template>

<style scoped>
.toast-enter-active {
  transition: all 0.3s ease-out;
}
.toast-leave-active {
  transition: all 0.2s ease-in;
}
.toast-enter-from {
  opacity: 0;
  transform: translate(-50%, -8px);
}
.toast-leave-to {
  opacity: 0;
  transform: translate(-50%, -4px);
}
</style>
