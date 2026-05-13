<script setup lang="ts">
import { ref, watch, computed } from 'vue';
import BaseInput from '@/components/base/BaseInput.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import { useProfiles } from '@/composables/useProfiles';
import { useStyles } from '@/composables/useStyles';
import type { AppProfile, VocabularyEntry } from '@/lib/commands';

const props = defineProps<{
  profile: AppProfile;
}>();

const emit = defineEmits<{
  save: [value: AppProfile];
  cancel: [];
  'edit-style': [styleId: string];
}>();

const draft = ref<AppProfile>(structuredClone(props.profile));
const detectError = ref<string | null>(null);

const { detectFrontmostApp } = useProfiles();
const { styles } = useStyles();

watch(
  () => props.profile,
  (next) => {
    draft.value = structuredClone(next);
  },
);

async function handleDetect() {
  detectError.value = null;
  try {
    const app = await detectFrontmostApp();
    if (!app) {
      detectError.value = 'Could not detect frontmost app. Make sure another app has focus.';
      return;
    }
    draft.value.bundleId = app.bundleId;
    if (!draft.value.displayName) {
      draft.value.displayName = app.name;
    }
  } catch (e) {
    detectError.value = String(e);
  }
}

const sortedStyles = computed(() => {
  const all = [...styles.value];
  return all.sort((a, b) => {
    if (a.builtin !== b.builtin) return a.builtin ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
});

const saveDisabled = computed(
  () =>
    draft.value.bundleId.trim().length === 0 ||
    draft.value.displayName.trim().length === 0 ||
    draft.value.styleId.trim().length === 0,
);

function handleSave() {
  if (saveDisabled.value) return;
  emit('save', draft.value);
}

// ── Profile-scoped vocabulary editing ──
const newVocabWrong = ref('');
const newVocabCorrect = ref('');

function addProfileVocab() {
  const wrong = newVocabWrong.value.trim();
  const correct = newVocabCorrect.value.trim();
  if (!wrong || !correct || wrong === correct) return;
  const now = new Date().toISOString();
  // Update existing entry (case-insensitive match on `wrong`) or push new.
  const existingIdx = draft.value.vocabulary.findIndex(
    (v) => v.wrong.toLowerCase() === wrong.toLowerCase(),
  );
  if (existingIdx >= 0) {
    draft.value.vocabulary[existingIdx] = {
      ...draft.value.vocabulary[existingIdx],
      correct,
      lastUsed: now,
    };
  } else {
    const entry: VocabularyEntry = {
      wrong,
      correct,
      source: 'manual',
      confidence: 1,
      createdAt: now,
      lastUsed: now,
    };
    draft.value.vocabulary.push(entry);
  }
  newVocabWrong.value = '';
  newVocabCorrect.value = '';
}

function removeProfileVocab(wrong: string) {
  draft.value.vocabulary = draft.value.vocabulary.filter(
    (v) => v.wrong.toLowerCase() !== wrong.toLowerCase(),
  );
}

// Vocabulary learning override: tri-state inherit / on / off.
const learningOverrideKind = computed<'inherit' | 'on' | 'off'>(() => {
  const v = draft.value.vocabularyLearningOverride;
  if (v === null || v === undefined) return 'inherit';
  return v ? 'on' : 'off';
});

function setLearningOverride(kind: 'inherit' | 'on' | 'off') {
  draft.value.vocabularyLearningOverride =
    kind === 'inherit' ? null : kind === 'on' ? true : false;
}
</script>

<template>
  <BaseCard padding="lg">
    <div class="flex flex-col gap-3">
      <div class="flex flex-col gap-1">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
          App bundle ID
        </span>
        <div class="flex gap-1.5 items-center">
          <BaseInput
            v-model="draft.bundleId"
            size="md"
            placeholder="com.example.app"
            class="flex-1"
          />
          <BaseButton variant="secondary" size="sm" @click="handleDetect">
            Detect current app
          </BaseButton>
        </div>
        <div v-if="detectError" class="text-[10px] text-flame">{{ detectError }}</div>
      </div>

      <div class="flex flex-col gap-1">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
          Display name
        </span>
        <BaseInput v-model="draft.displayName" size="md" placeholder="My Editor" />
      </div>

      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between">
          <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">Style</span>
          <BaseButton
            v-if="draft.styleId"
            variant="link"
            size="sm"
            @click="emit('edit-style', draft.styleId)"
          >
            Edit style…
          </BaseButton>
        </div>
        <select
          v-model="draft.styleId"
          class="rounded-md bg-raised border border-edge text-ink text-[12px] px-2.5 py-1.5 focus:outline-none focus:border-gold/40"
        >
          <option v-for="s in sortedStyles" :key="s.id" :value="s.id">
            {{ s.name }}{{ s.builtin ? ' (built-in)' : '' }}
          </option>
        </select>
      </div>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Profile vocabulary
        </div>
        <div v-if="draft.vocabulary.length > 0" class="flex flex-col gap-1 mb-1.5">
          <div
            v-for="entry in draft.vocabulary"
            :key="entry.wrong"
            class="group flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-panel border border-edge"
          >
            <span class="text-[11px] text-ink-muted truncate">{{ entry.wrong }}</span>
            <svg
              width="10"
              height="10"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="2.5"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="flex-shrink-0 text-ink-faint"
            >
              <line x1="5" y1="12" x2="19" y2="12" />
              <polyline points="12 5 19 12 12 19" />
            </svg>
            <span class="text-[11px] font-semibold text-ink truncate">
              {{ entry.correct }}
            </span>
            <span
              class="ml-auto flex-shrink-0 px-1 py-0.5 rounded text-[8px] font-bold uppercase tracking-wider"
              :class="
                entry.source === 'auto'
                  ? 'bg-gold/10 text-gold/80 border border-gold/15'
                  : 'bg-raised text-ink-faint border border-edge'
              "
            >
              {{ entry.source }}
            </span>
            <button
              class="flex-shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 text-ink-faint hover:text-flame transition-all duration-150"
              @click="removeProfileVocab(entry.wrong)"
            >
              <svg
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        </div>
        <div class="flex gap-1.5 items-center">
          <BaseInput
            v-model="newVocabWrong"
            size="sm"
            placeholder="Wrong word"
            class="flex-1"
            @keydown.enter="addProfileVocab"
          />
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="text-ink-faint flex-shrink-0"
          >
            <line x1="5" y1="12" x2="19" y2="12" />
            <polyline points="12 5 19 12 12 19" />
          </svg>
          <BaseInput
            v-model="newVocabCorrect"
            size="sm"
            placeholder="Correct word"
            class="flex-1"
            @keydown.enter="addProfileVocab"
          />
          <BaseButton
            variant="secondary"
            size="sm"
            :disabled="!newVocabWrong || !newVocabCorrect || newVocabWrong === newVocabCorrect"
            @click="addProfileVocab"
          >
            Add
          </BaseButton>
        </div>
      </div>

      <SettingsRow
        label="Vocabulary learning"
        helper="When in this app, attribute auto-learned corrections to this profile."
      >
        <div class="flex items-center gap-1">
          <button
            v-for="opt in ['inherit', 'on', 'off'] as const"
            :key="opt"
            type="button"
            class="px-2 py-1 rounded-md text-[10px] font-semibold transition-all duration-150"
            :class="
              learningOverrideKind === opt
                ? 'bg-gold/[0.06] border border-gold/30 text-ink'
                : 'bg-panel border border-edge text-ink-muted hover:bg-raised hover:border-edge-strong'
            "
            @click="setLearningOverride(opt)"
          >
            {{ opt === 'inherit' ? 'Inherit' : opt === 'on' ? 'On' : 'Off' }}
          </button>
        </div>
      </SettingsRow>

      <div class="flex justify-end gap-1.5">
        <BaseButton variant="link" size="md" @click="emit('cancel')">Cancel</BaseButton>
        <BaseButton variant="primary" size="md" :disabled="saveDisabled" @click="handleSave">
          Save profile
        </BaseButton>
      </div>
    </div>
  </BaseCard>
</template>
