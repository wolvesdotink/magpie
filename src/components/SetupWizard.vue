<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { checkPermissions } from '@/lib/commands';
import WelcomeStep from '@/components/setup/WelcomeStep.vue';
import PermissionsStep from '@/components/setup/PermissionsStep.vue';
import ModelSelector from '@/components/shared/ModelSelector.vue';
import TranscriptionStep from '@/components/setup/TranscriptionStep.vue';

const emit = defineEmits<{
  complete: [];
}>();

// ── Step state ────────────────────────────────────────────────────
//
// The wizard is a thin state machine: a `currentStep` ref plus a
// computed `visibleSteps` list that drops the permissions step when
// both permissions are already granted at launch. Each step is its
// own component that owns its data fetches and emits `next` when the
// user is ready to advance.

type StepId = 'welcome' | 'permissions' | 'model' | 'transcription';

const currentStep = ref<StepId>('welcome');

/// `true` until the initial perm check completes, then locked to the
/// observed value. Drives whether the permissions step appears in the
/// nav order — a user who already granted both perms skips straight to
/// the model picker.
const needsPermissions = ref(true);

const visibleSteps = computed<StepId[]>(() => {
  const steps: StepId[] = ['welcome'];
  if (needsPermissions.value) steps.push('permissions');
  steps.push('model', 'transcription');
  return steps;
});

const currentStepIndex = computed(() => visibleSteps.value.indexOf(currentStep.value));

function nextStep() {
  const idx = currentStepIndex.value;
  if (idx < visibleSteps.value.length - 1) {
    currentStep.value = visibleSteps.value[idx + 1];
  }
  // Once the user clears the permissions step (PermissionsStep auto-emits
  // `next` when both are granted), drop the dot from the progress strip
  // so the indicator collapses from 4 dots to 3. visibleSteps recomputes
  // synchronously and currentStepIndex stays correct because the new step
  // we just navigated to keeps its position.
  if (currentStep.value !== 'permissions' && needsPermissions.value) {
    needsPermissions.value = false;
  }
}

onMounted(async () => {
  // One-shot permissions check so we know whether to include the
  // permissions step. PermissionsStep itself does its own re-check on
  // mount and runs the polling loop while it's active.
  try {
    const perms = await checkPermissions();
    needsPermissions.value = !perms.accessibility || !perms.microphone;
  } catch (e) {
    console.error('Setup wizard init permissions check failed:', e);
  }
});
</script>

<template>
  <div class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden">
    <!-- Top edge -->
    <div class="h-px bg-gradient-to-r from-transparent via-gold/20 to-transparent" />

    <WelcomeStep v-if="currentStep === 'welcome'" @next="nextStep" />
    <PermissionsStep v-else-if="currentStep === 'permissions'" @next="nextStep" />
    <ModelSelector
      v-else-if="currentStep === 'model'"
      initial-tab="multilingual"
      initial-selected-id="small"
      ready-label="Continue"
      download-label="Download & Continue"
      @done="nextStep"
    />
    <TranscriptionStep v-else-if="currentStep === 'transcription'" @finish="emit('complete')" />

    <!-- ════════════════ PROGRESS DOTS ════════════════ -->
    <div class="flex justify-center gap-2 pb-4 pt-2">
      <span
        v-for="(step, idx) in visibleSteps"
        :key="step"
        class="w-[6px] h-[6px] rounded-full transition-all duration-300"
        :class="
          idx === currentStepIndex
            ? 'bg-gold scale-110'
            : idx < currentStepIndex
              ? 'bg-gold/40'
              : 'bg-edge'
        "
      />
    </div>
  </div>
</template>
