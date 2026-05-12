<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { getSettings, updateSettings, type UserSettings } from '@/lib/commands';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';

const emit = defineEmits<{
  finish: [];
}>();

const settings = ref<UserSettings | null>(null);

function toggleRemoveFillers(value: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, removeFillers: value };
}

function toggleSelfCorrection(value: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, selfCorrection: value };
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

    <div class="text-center">
      <h2 class="text-[15px] font-bold tracking-tight text-ink">Improve Your Transcripts</h2>
      <p class="text-[10px] text-ink-muted leading-relaxed mt-2 max-w-[240px]">
        Clean up your transcriptions automatically. You can change these anytime in Settings.
      </p>
    </div>

    <div class="w-full flex flex-col gap-2">
      <BaseCard padding="lg">
        <div class="flex items-center justify-between gap-3">
          <div class="flex flex-col min-w-0 flex-1">
            <span class="text-[12px] font-semibold text-ink">Remove filler words</span>
            <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
              Strips "um", "uh", "hmm" and similar
            </span>
          </div>
          <BaseToggle
            :model-value="!!settings?.removeFillers"
            @update:model-value="toggleRemoveFillers"
          />
        </div>
      </BaseCard>

      <BaseCard padding="lg">
        <div class="flex items-center justify-between gap-3">
          <div class="flex flex-col min-w-0 flex-1">
            <span class="text-[12px] font-semibold text-ink">Self-correction cleanup</span>
            <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
              Detect and remove corrections like "no wait" or restated phrases
            </span>
          </div>
          <BaseToggle
            :model-value="!!settings?.selfCorrection"
            @update:model-value="toggleSelfCorrection"
          />
        </div>
      </BaseCard>

      <BaseCard v-if="settings?.selfCorrection" tone="gold">
        <span class="text-[10px] text-gold leading-snug">
          Requires a correction model. You can download one in Settings after setup.
        </span>
      </BaseCard>
    </div>

    <BaseButton variant="primary" size="lg" full-width class="mt-auto" @click="handleFinish">
      Finish Setup
    </BaseButton>
  </div>
</template>
