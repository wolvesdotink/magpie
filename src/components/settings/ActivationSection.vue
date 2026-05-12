<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue';
import { useSettings } from '@/composables/useSettings';

const { settings, updateActivationMode, updateCustomShortcut } = useSettings();

const activationModes = [
  {
    id: 'holdFn' as const,
    label: 'Hold Fn',
    desc: 'Hold to record, release to stop',
    icon: 'M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h8',
  },
  {
    id: 'tapFn' as const,
    label: 'Tap Fn',
    desc: 'Press Fn to start, press Fn again to stop (also fires on Fn shortcuts)',
    icon: 'M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h5',
  },
  {
    id: 'doubleTapFn' as const,
    label: 'Double-tap Fn',
    desc: 'Tap twice to start, once to stop',
    icon: 'M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm3 10h3m5 0h3',
  },
  {
    id: 'shortcut' as const,
    label: 'Keyboard shortcut',
    desc: 'Use a custom key combination',
    icon: 'M5 3h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2zm4 8l3 3 5-6',
  },
];

// ── Global shortcut capture ────────────────────────────────────────

const DEFAULT_SHORTCUT_DISPLAY = '⌘⇧Space';

const capturing = ref(false);
const shortcutError = ref<string | null>(null);

/** Map a DOM KeyboardEvent.key/code into Tauri's shortcut-string key segment. */
function keyToTauri(e: KeyboardEvent): string | null {
  // Modifier-only events have e.key === "Meta"/"Control"/"Alt"/"Shift" — caller filters these.
  const key = e.key;

  // Whitespace + special keys
  if (key === ' ') return 'Space';
  if (key === 'Escape') return 'Escape';
  if (key === 'Tab') return 'Tab';
  if (key === 'Enter') return 'Enter';
  if (key === 'Backspace') return 'Backspace';
  if (key === 'ArrowUp') return 'Up';
  if (key === 'ArrowDown') return 'Down';
  if (key === 'ArrowLeft') return 'Left';
  if (key === 'ArrowRight') return 'Right';
  if (key === 'PageUp') return 'PageUp';
  if (key === 'PageDown') return 'PageDown';
  if (key === 'Home') return 'Home';
  if (key === 'End') return 'End';

  // Function keys (F1–F24) come through as-is, but normalize casing.
  if (/^F\d{1,2}$/.test(key)) return key;

  // Letters: uppercase. Digits: as-is. Punctuation: pass through.
  if (key.length === 1) return key.toUpperCase();

  // Fall back to e.code for less-common keys
  if (e.code && e.code.length > 0) return e.code;

  return null;
}

/** Build a Tauri shortcut string from a keydown event. Returns null if it
 *  doesn't include a non-modifier key. */
function buildShortcutString(e: KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.metaKey || e.ctrlKey) mods.push('CmdOrCtrl');
  if (e.altKey) mods.push('Alt');
  if (e.shiftKey) mods.push('Shift');

  const key = keyToTauri(e);
  if (!key) return null;
  return [...mods, key].join('+');
}

/** Pretty-print a Tauri shortcut string with macOS glyphs (⌘⇧⌃⌥). */
function formatShortcut(s: string | null | undefined): string {
  if (!s) return DEFAULT_SHORTCUT_DISPLAY;
  return s
    .split('+')
    .map((part) => {
      switch (part) {
        case 'CmdOrCtrl':
        case 'Cmd':
        case 'Command':
        case 'Meta':
          return '⌘';
        case 'Ctrl':
        case 'Control':
          return '⌃';
        case 'Alt':
        case 'Option':
          return '⌥';
        case 'Shift':
          return '⇧';
        default:
          return part;
      }
    })
    .join('');
}

const displayedShortcut = computed(() => formatShortcut(settings.value?.customShortcut ?? null));

let captureHandler: ((e: KeyboardEvent) => void) | null = null;

function stopCapture() {
  capturing.value = false;
  if (captureHandler) {
    window.removeEventListener('keydown', captureHandler, true);
    captureHandler = null;
  }
}

function startCapture() {
  if (capturing.value) {
    stopCapture();
    return;
  }
  shortcutError.value = null;
  capturing.value = true;

  captureHandler = (e: KeyboardEvent) => {
    // Capture-phase listener with stopPropagation prevents the keystroke
    // from landing in any focused input or triggering app shortcuts.
    e.preventDefault();
    e.stopPropagation();

    // Modifier-only presses: keep listening until the user adds a real key.
    if (e.key === 'Meta' || e.key === 'Control' || e.key === 'Alt' || e.key === 'Shift') {
      return;
    }

    // Escape cancels capture without changing the shortcut.
    if (e.key === 'Escape') {
      stopCapture();
      return;
    }

    const built = buildShortcutString(e);
    if (!built) {
      shortcutError.value = 'Could not interpret that key.';
      return;
    }

    // Require at least one modifier so the user can't bind a single
    // letter (which would steal keystrokes globally).
    const hasModifier = /(CmdOrCtrl|Alt|Shift)/.test(built);
    if (!hasModifier) {
      shortcutError.value = 'Must include a modifier (⌘, ⌃, ⌥ or ⇧).';
      return;
    }

    stopCapture();
    updateCustomShortcut(built)
      .then(() => {
        shortcutError.value = null;
      })
      .catch((err) => {
        shortcutError.value = String(err);
      });
  };

  window.addEventListener('keydown', captureHandler, true);
}

function resetShortcut() {
  shortcutError.value = null;
  updateCustomShortcut(null).catch((err) => {
    shortcutError.value = String(err);
  });
}

onUnmounted(stopCapture);
</script>

<template>
  <section class="settings-section">
    <div class="section-header">
      <div class="section-icon">
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <rect x="2" y="4" width="20" height="16" rx="2" />
          <path
            d="M6 8h.001M10 8h.001M14 8h.001M18 8h.001M8 12h.001M12 12h.001M16 12h.001M8 16h8"
          />
        </svg>
      </div>
      <span class="section-label">Activation</span>
    </div>

    <div class="flex flex-col gap-1.5">
      <button
        v-for="mode in activationModes"
        :key="mode.id"
        class="flex items-center gap-2.5 p-2.5 rounded-lg border text-left transition-all duration-150"
        :class="
          settings?.activationMode === mode.id
            ? 'bg-gold/[0.04] border-gold/20'
            : 'bg-panel border-edge hover:border-edge-strong hover:bg-raised'
        "
        @click="updateActivationMode(mode.id)"
      >
        <div
          class="w-3.5 h-3.5 rounded-full border-[1.5px] flex items-center justify-center flex-shrink-0 transition-all"
          :class="
            settings?.activationMode === mode.id ? 'border-gold bg-gold/10' : 'border-edge-strong'
          "
        >
          <div
            v-if="settings?.activationMode === mode.id"
            class="w-1.5 h-1.5 rounded-full bg-gold"
          />
        </div>
        <div class="flex flex-col min-w-0">
          <span
            class="text-[12px] font-semibold"
            :class="settings?.activationMode === mode.id ? 'text-ink' : 'text-ink-muted'"
          >
            {{ mode.label }}
          </span>
          <span class="text-[10px] text-ink-faint leading-snug">
            {{ mode.desc }}
          </span>
        </div>
      </button>
    </div>

    <!-- ── Custom hotkey capture (Shortcut mode only) ── -->
    <div
      v-if="settings?.activationMode === 'shortcut'"
      class="mt-2 p-2.5 rounded-lg bg-panel border border-edge flex flex-col gap-2"
    >
      <div class="flex items-center justify-between gap-3">
        <div class="flex flex-col min-w-0">
          <span class="text-[12px] font-semibold text-ink"> Global hotkey </span>
          <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
            Click to capture. Must include a modifier (⌘, ⌃, ⌥ or ⇧). Press Esc to cancel.
          </span>
        </div>
        <button
          class="px-2.5 py-1 rounded-md bg-raised border min-w-[110px] text-[11px] font-semibold text-center transition"
          :class="
            capturing ? 'border-gold/40 text-gold' : 'border-edge text-ink hover:border-edge-strong'
          "
          @click="startCapture"
        >
          {{ capturing ? 'Press a key…' : displayedShortcut }}
        </button>
      </div>
      <button
        v-if="settings?.customShortcut"
        class="self-end text-[10px] text-ink-faint hover:text-ink transition-colors"
        @click="resetShortcut"
      >
        Reset to default ({{ DEFAULT_SHORTCUT_DISPLAY }})
      </button>
      <div v-if="shortcutError" class="text-[10px] text-flame">
        {{ shortcutError }}
      </div>
    </div>
  </section>
</template>
