<script setup lang="ts">
import { ref, computed, watch } from 'vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseInput from '@/components/base/BaseInput.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import FormattingRulesEditor from './FormattingRulesEditor.vue';
import CustomRulesEditor from './CustomRulesEditor.vue';
import { useStyles } from '@/composables/useStyles';
import { useProfiles } from '@/composables/useProfiles';
import { WRITING_SAMPLES_MAX_CHARS, type Style, type CorrectionOverride } from '@/lib/commands';

const props = defineProps<{
  style: Style;
}>();

const emit = defineEmits<{
  save: [value: Style];
  cancel: [];
}>();

const draft = ref<Style>(JSON.parse(JSON.stringify(props.style)));
const { previewStyle } = useStyles();
const { profiles } = useProfiles();

watch(
  () => props.style,
  (next) => {
    draft.value = JSON.parse(JSON.stringify(next));
  },
);

const correctionKind = computed(() => draft.value.correction.kind);

function setCorrection(kind: CorrectionOverride['kind']) {
  let next: CorrectionOverride;
  switch (kind) {
    case 'inherit':
      next = { kind: 'inherit' };
      break;
    case 'disabled':
      next = { kind: 'disabled' };
      break;
    case 'casual':
      next = { kind: 'casual' };
      break;
    case 'formal':
      next = { kind: 'formal' };
      break;
    case 'custom':
      next = {
        kind: 'custom',
        prompt:
          draft.value.correction.kind === 'custom'
            ? draft.value.correction.prompt
            : 'You are a dictation cleanup assistant. Remove self-corrections only. Output ONLY the cleaned text.',
      };
      break;
  }
  draft.value.correction = next;
}

const customPrompt = computed(() => {
  const c = draft.value.correction;
  return c.kind === 'custom' ? c.prompt : '';
});

function setCustomPrompt(v: string) {
  if (draft.value.correction.kind === 'custom') {
    draft.value.correction.prompt = v;
  }
}

// Writing samples — keep a separate ref bound to the raw textarea so
// mid-typing edits (e.g. an in-progress third blank line) don't get collapsed
// by the split→join round-trip. The watcher commits to `draft.writingSamples`
// on every change.
const writingSamplesText = ref<string>((props.style.writingSamples ?? []).join('\n\n'));

watch(
  () => props.style.id,
  () => {
    writingSamplesText.value = (props.style.writingSamples ?? []).join('\n\n');
  },
);

watch(writingSamplesText, (next) => {
  const parts = next
    .split(/\n{2,}/)
    .map((p) => p.trim())
    .filter((p) => p.length > 0);
  draft.value.writingSamples = parts;
});

const writingSamplesCount = computed(() => writingSamplesText.value.length);
const writingSamplesTooLong = computed(() => writingSamplesCount.value > WRITING_SAMPLES_MAX_CHARS);

const CORRECTION_OPTIONS: { value: CorrectionOverride['kind']; label: string; helper: string }[] = [
  {
    value: 'inherit',
    label: 'Inherit global',
    helper: 'Use the global self-correction toggle and prompt.',
  },
  { value: 'disabled', label: 'Disabled', helper: 'Skip the LLM cleanup step.' },
  {
    value: 'casual',
    label: 'Casual',
    helper: 'Keep contractions and informal register.',
  },
  {
    value: 'formal',
    label: 'Formal',
    helper: 'Polish sentences for written communication.',
  },
  {
    value: 'custom',
    label: 'Custom prompt',
    helper: 'Write your own system prompt for the cleanup model.',
  },
];

// ── Live preview ──
const previewInput = ref(
  'Um, this is a sample sentence. It demonstrates the style preview, e.g. casing and punctuation.',
);
const previewOutput = ref('');
const previewError = ref<string | null>(null);
let previewTimer: ReturnType<typeof setTimeout> | null = null;

async function runPreview() {
  previewError.value = null;
  try {
    previewOutput.value = await previewStyle(draft.value, previewInput.value);
  } catch (e) {
    previewError.value = String(e);
  }
}

watch(
  [draft, previewInput],
  () => {
    if (previewTimer) clearTimeout(previewTimer);
    previewTimer = setTimeout(runPreview, 300);
  },
  { deep: true, immediate: true },
);

const referencingProfiles = computed(() =>
  profiles.value.filter((p) => p.styleId === draft.value.id),
);

const customPromptCount = computed(() => customPrompt.value.length);
const MAX_CUSTOM_PROMPT = 2048;
const tooLong = computed(() => customPromptCount.value > MAX_CUSTOM_PROMPT);

const saveDisabled = computed(() => {
  if (draft.value.name.trim().length === 0) return true;
  if (tooLong.value) return true;
  if (writingSamplesTooLong.value) return true;
  if (draft.value.correction.kind === 'custom' && draft.value.correction.prompt.trim().length === 0)
    return true;
  return false;
});

function handleSave() {
  if (saveDisabled.value) return;
  emit('save', draft.value);
}
</script>

<template>
  <BaseCard padding="lg">
    <div class="flex flex-col gap-3">
      <div class="flex flex-col gap-1">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">Name</span>
        <BaseInput v-model="draft.name" size="md" placeholder="Style name" />
      </div>

      <div class="flex flex-col gap-1">
        <span class="text-[10px] font-semibold text-ink-faint tracking-[0.02em]">
          Description (optional)
        </span>
        <BaseInput
          :model-value="draft.description ?? ''"
          size="md"
          placeholder="Short description shown next to the style name"
          @update:model-value="draft.description = $event || null"
        />
      </div>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Formatting
        </div>
        <FormattingRulesEditor v-model="draft.formatting" />
      </div>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Correction
        </div>
        <div class="grid grid-cols-3 gap-1">
          <button
            v-for="opt in CORRECTION_OPTIONS"
            :key="opt.value"
            type="button"
            class="flex flex-col items-start gap-0.5 px-2 py-1.5 rounded-md border text-left transition-all duration-150"
            :class="
              correctionKind === opt.value
                ? 'bg-gold/[0.06] border-gold/30 text-ink'
                : 'bg-panel border-edge text-ink-muted hover:bg-raised hover:border-edge-strong'
            "
            @click="setCorrection(opt.value)"
          >
            <span class="text-[11px] font-semibold">{{ opt.label }}</span>
            <span class="text-[9px] text-ink-faint truncate w-full">{{ opt.helper }}</span>
          </button>
        </div>
        <div v-if="correctionKind === 'custom'" class="mt-1.5">
          <BaseCard tone="dashed">
            <div class="flex flex-col gap-1">
              <textarea
                :value="customPrompt"
                rows="5"
                placeholder="Enter the system prompt for the cleanup model..."
                class="w-full min-w-0 rounded-md bg-raised border border-edge text-ink text-[11px] px-2 py-1.5 placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 focus:shadow-[0_0_0_3px_rgba(232,175,71,0.08)] resize-y"
                @input="setCustomPrompt(($event.target as HTMLTextAreaElement).value)"
              />
              <div class="flex items-center justify-between text-[9px]">
                <span class="text-ink-faint">
                  The prompt is applied verbatim before the user transcription. Hallucination guards
                  still apply.
                </span>
                <span :class="tooLong ? 'text-flame font-semibold' : 'text-ink-faint'">
                  {{ customPromptCount }} / {{ MAX_CUSTOM_PROMPT }}
                </span>
              </div>
            </div>
          </BaseCard>
        </div>
      </div>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Writing samples
        </div>
        <BaseCard tone="dashed">
          <div class="flex flex-col gap-1">
            <textarea
              v-model="writingSamplesText"
              rows="6"
              placeholder="Paste 1–3 paragraphs you've written. Separate them with a blank line."
              class="w-full min-w-0 rounded-md bg-raised border border-edge text-ink text-[11px] px-2 py-1.5 placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 focus:shadow-[0_0_0_3px_rgba(232,175,71,0.08)] resize-y"
            />
            <div class="flex items-center justify-between text-[9px]">
              <span class="text-ink-faint">
                Stored locally in this style — never sent to a server. Used as a voice reference
                during cleanup. Most effective with Casual, Formal, or Custom correction modes; the
                default Inherit prompt is strict enough that samples have little effect. Preview
                below reflects formatting only.
              </span>
              <span :class="writingSamplesTooLong ? 'text-flame font-semibold' : 'text-ink-faint'">
                {{ writingSamplesCount }} / {{ WRITING_SAMPLES_MAX_CHARS }}
              </span>
            </div>
          </div>
        </BaseCard>
      </div>

      <SettingsRow
        label="Override filler removal"
        helper="If set, this style overrides the global 'Remove filler words' setting."
      >
        <div class="flex items-center gap-2">
          <BaseToggle
            :model-value="draft.fillerOverride !== null"
            @update:model-value="draft.fillerOverride = $event ? true : null"
          />
          <BaseToggle
            v-if="draft.fillerOverride !== null"
            :model-value="draft.fillerOverride"
            @update:model-value="draft.fillerOverride = $event"
          />
        </div>
      </SettingsRow>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Custom rules
        </div>
        <CustomRulesEditor v-model="draft.customRules" />
      </div>

      <div>
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1.5">
          Live preview
        </div>
        <BaseCard tone="dashed">
          <div class="flex flex-col gap-2">
            <div>
              <span class="text-[10px] text-ink-faint">Sample input</span>
              <BaseInput
                v-model="previewInput"
                size="sm"
                placeholder="Type sample text to preview..."
              />
            </div>
            <div>
              <span class="text-[10px] text-ink-faint">Output</span>
              <div
                class="rounded-md bg-raised border border-edge text-ink text-[12px] px-2 py-1.5 min-h-[2rem] whitespace-pre-wrap break-words"
              >
                {{ previewOutput }}
              </div>
              <div v-if="previewError" class="mt-1 text-[10px] text-flame">
                {{ previewError }}
              </div>
            </div>
          </div>
        </BaseCard>
      </div>

      <div v-if="referencingProfiles.length > 0">
        <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1">
          Used by {{ referencingProfiles.length }}
          {{ referencingProfiles.length === 1 ? 'profile' : 'profiles' }}
        </div>
        <div class="flex flex-wrap gap-1">
          <span
            v-for="p in referencingProfiles"
            :key="p.id"
            class="px-1.5 py-0.5 rounded text-[10px] bg-panel border border-edge text-ink-muted"
          >
            {{ p.displayName }}
          </span>
        </div>
      </div>

      <div class="flex justify-end gap-1.5">
        <BaseButton variant="link" size="md" @click="emit('cancel')">Cancel</BaseButton>
        <BaseButton variant="primary" size="md" :disabled="saveDisabled" @click="handleSave">
          Save style
        </BaseButton>
      </div>
    </div>
  </BaseCard>
</template>
