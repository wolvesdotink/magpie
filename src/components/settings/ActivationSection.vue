<script setup lang="ts">
import { computed } from 'vue';
import { useSettings } from '@/composables/useSettings';
import { useShortcutCapture } from '@/composables/useShortcutCapture';
import { formatShortcut } from '@/lib/keyboard';
import SettingsSection from '@/components/base/SettingsSection.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import RadioGroup from '@/components/base/RadioGroup.vue';

const { settings, updateActivationMode, updateCustomShortcut } = useSettings();

type ActivationMode = 'holdFn' | 'tapFn' | 'doubleTapFn' | 'shortcut';

const activationOptions: { value: ActivationMode; label: string; desc: string }[] = [
  { value: 'holdFn', label: 'Hold Fn', desc: 'Hold to record, release to stop' },
  {
    value: 'tapFn',
    label: 'Tap Fn',
    desc: 'Press Fn to start, press Fn again to stop (also fires on Fn shortcuts)',
  },
  { value: 'doubleTapFn', label: 'Double-tap Fn', desc: 'Tap twice to start, once to stop' },
  { value: 'shortcut', label: 'Keyboard shortcut', desc: 'Use a custom key combination' },
];

const activationMode = computed<ActivationMode>({
  get: () => (settings.value?.activationMode as ActivationMode) ?? 'holdFn',
  set: (value) => updateActivationMode(value),
});

const DEFAULT_SHORTCUT_DISPLAY = '⌘⇧Space';

const {
  capturing,
  error: shortcutError,
  start: startCaptureRaw,
  stop: stopCapture,
} = useShortcutCapture();

const displayedShortcut = computed(() =>
  formatShortcut(settings.value?.customShortcut ?? null, DEFAULT_SHORTCUT_DISPLAY),
);

async function startCapture() {
  if (capturing.value) {
    stopCapture();
    return;
  }
  const result = await startCaptureRaw();
  if (result === null) return;
  try {
    await updateCustomShortcut(result);
  } catch (err) {
    shortcutError.value = String(err);
  }
}

function resetShortcut() {
  shortcutError.value = null;
  updateCustomShortcut(null).catch((err) => {
    shortcutError.value = String(err);
  });
}
</script>

<template>
  <SettingsSection label="Activation">
    <template #icon>
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
        <path d="M6 8h.001M10 8h.001M14 8h.001M18 8h.001M8 12h.001M12 12h.001M16 12h.001M8 16h8" />
      </svg>
    </template>

    <RadioGroup v-model="activationMode" :options="activationOptions" />

    <BaseCard v-if="settings?.activationMode === 'shortcut'" class="mt-2 flex flex-col gap-2">
      <div class="flex items-center justify-between gap-3">
        <div class="flex flex-col min-w-0">
          <span class="text-[12px] font-semibold text-ink">Global hotkey</span>
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
    </BaseCard>
  </SettingsSection>
</template>
