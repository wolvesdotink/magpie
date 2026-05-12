<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { getSettings, updateSettings, type UserSettings } from '@/lib/commands';

const emit = defineEmits<{
  finish: [];
}>();

const settings = ref<UserSettings | null>(null);

function toggleRemoveFillers() {
  if (!settings.value) return;
  settings.value = { ...settings.value, removeFillers: !settings.value.removeFillers };
}

function toggleSelfCorrection() {
  if (!settings.value) return;
  settings.value = { ...settings.value, selfCorrection: !settings.value.selfCorrection };
}

async function handleFinish() {
  try {
    const current = await getSettings();
    await updateSettings({
      ...current,
      removeFillers: settings.value?.removeFillers ?? current.removeFillers,
      selfCorrection: settings.value?.selfCorrection ?? current.selfCorrection,
      setupComplete: true,
    });
  } catch (e) {
    console.error('Failed to save settings:', e);
  }
  emit('finish');
}

onMounted(async () => {
  try {
    settings.value = await getSettings();
  } catch (e) {
    console.error('Settings fetch failed:', e);
  }
});
</script>

<template>
  <div class="flex flex-col items-center px-6 pt-8 pb-6 gap-5 flex-1">
    <!-- Icon -->
    <div
      class="flex items-center justify-center w-14 h-14 rounded-2xl bg-gradient-to-br from-gold/15 to-gold/5 text-gold shadow-glow-gold"
    >
      <svg
        width="26"
        height="26"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <path d="M12 20h9" />
        <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
      </svg>
    </div>

    <!-- Title -->
    <div class="text-center">
      <h2 class="text-[15px] font-bold tracking-tight text-ink">Improve Your Transcripts</h2>
      <p class="text-[10px] text-ink-muted leading-relaxed mt-2 max-w-[240px]">
        Clean up your transcriptions automatically. You can change these anytime in Settings.
      </p>
    </div>

    <!-- Toggles -->
    <div class="w-full flex flex-col gap-2">
      <!-- Filler word removal -->
      <div class="flex items-center justify-between p-3 rounded-lg bg-panel border border-edge">
        <div class="flex flex-col min-w-0 mr-3">
          <span class="text-[12px] font-semibold text-ink"> Remove filler words </span>
          <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
            Strips "um", "uh", "hmm" and similar
          </span>
        </div>
        <button
          class="toggle-switch flex-shrink-0"
          :class="settings?.removeFillers ? 'toggle-on' : 'toggle-off'"
          @click="toggleRemoveFillers"
        >
          <div class="toggle-thumb" />
        </button>
      </div>

      <!-- Self-correction cleanup -->
      <div class="flex items-center justify-between p-3 rounded-lg bg-panel border border-edge">
        <div class="flex flex-col min-w-0 mr-3">
          <span class="text-[12px] font-semibold text-ink"> Self-correction cleanup </span>
          <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
            Detect and remove corrections like "no wait" or restated phrases
          </span>
        </div>
        <button
          class="toggle-switch flex-shrink-0"
          :class="settings?.selfCorrection ? 'toggle-on' : 'toggle-off'"
          @click="toggleSelfCorrection"
        >
          <div class="toggle-thumb" />
        </button>
      </div>

      <!-- Self-correction hint -->
      <div
        v-if="settings?.selfCorrection"
        class="p-2.5 rounded-lg bg-gold/[0.06] border border-gold/15"
      >
        <span class="text-[10px] text-gold leading-snug">
          Requires a correction model. You can download one in Settings after setup.
        </span>
      </div>
    </div>

    <!-- CTA -->
    <button
      class="w-full py-2.5 rounded-lg text-[13px] font-semibold transition-all duration-200 active:scale-[0.97] bg-gradient-to-b from-gold to-gold-hover text-gold-ink hover:from-gold-hover hover:to-gold-deep shadow-press hover:shadow-lifted mt-auto"
      @click="handleFinish"
    >
      Finish Setup
    </button>
  </div>
</template>

<style scoped>
.toggle-switch {
  position: relative;
  width: 32px;
  height: 18px;
  border-radius: 9px;
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  cursor: pointer;
}

.toggle-on {
  background: var(--color-gold);
  box-shadow: 0 0 8px rgba(232, 175, 71, 0.2);
}

.toggle-off {
  background: var(--color-edge-strong);
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: white;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.3);
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
}

.toggle-on .toggle-thumb {
  left: 16px;
}

.toggle-off .toggle-thumb {
  left: 2px;
}
</style>
