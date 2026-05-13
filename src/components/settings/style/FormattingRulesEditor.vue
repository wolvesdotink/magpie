<script setup lang="ts">
import { computed } from 'vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseInput from '@/components/base/BaseInput.vue';
import SettingsRow from '@/components/base/SettingsRow.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import type { FormattingRules, CasingMode, PunctuationMode } from '@/lib/commands';

const props = defineProps<{
  modelValue: FormattingRules;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: FormattingRules];
}>();

function patch(p: Partial<FormattingRules>) {
  emit('update:modelValue', { ...props.modelValue, ...p });
}

const CASING_OPTIONS: { value: CasingMode; label: string; helper: string }[] = [
  { value: 'sentence', label: 'Sentence', helper: 'Capitalize the first letter.' },
  { value: 'preserve', label: 'Preserve', helper: "Use Whisper's output as-is." },
  { value: 'lowercase', label: 'lowercase', helper: 'force everything lower case.' },
  { value: 'uppercase', label: 'UPPERCASE', helper: 'force everything upper case.' },
  { value: 'snakeCase', label: 'snake_case', helper: 'identifier_style_for_editors' },
  { value: 'kebabCase', label: 'kebab-case', helper: 'identifier-style-with-dashes' },
  { value: 'camelCase', label: 'camelCase', helper: 'capitalizeJoinedTokens' },
  { value: 'pascalCase', label: 'PascalCase', helper: 'ClassNameStyle' },
  { value: 'screamSnake', label: 'SCREAM_SNAKE', helper: 'CONSTANT_STYLE' },
];

const PUNCTUATION_BASE_OPTIONS: {
  value: PunctuationMode['kind'];
  label: string;
  helper: string;
}[] = [
  { value: 'auto', label: 'Auto', helper: "Keep Whisper's punctuation." },
  { value: 'strip', label: 'Strip all', helper: 'Remove all punctuation.' },
  { value: 'sentenceOnly', label: 'Sentence-ending only', helper: 'Keep . ! ? at sentence ends.' },
  { value: 'custom', label: 'Custom keep-list', helper: 'Keep only the characters you list.' },
];

const punctuationKind = computed(() => props.modelValue.punctuation.kind);

const customChars = computed(() => {
  const p = props.modelValue.punctuation;
  return p.kind === 'custom' ? p.chars.join('') : '';
});

function setCasing(value: CasingMode) {
  patch({ casing: value });
}

function setPunctuationKind(kind: PunctuationMode['kind']) {
  let next: PunctuationMode;
  switch (kind) {
    case 'auto':
      next = { kind: 'auto' };
      break;
    case 'strip':
      next = { kind: 'strip' };
      break;
    case 'sentenceOnly':
      next = { kind: 'sentenceOnly' };
      break;
    case 'custom':
      next = {
        kind: 'custom',
        chars:
          props.modelValue.punctuation.kind === 'custom'
            ? props.modelValue.punctuation.chars
            : [],
      };
      break;
  }
  patch({ punctuation: next });
}

function setCustomChars(s: string) {
  // Dedup while preserving entry order; surface only single chars.
  const seen = new Set<string>();
  const chars: string[] = [];
  for (const c of s) {
    if (!seen.has(c)) {
      seen.add(c);
      chars.push(c);
    }
  }
  patch({ punctuation: { kind: 'custom', chars } });
}
</script>

<template>
  <div class="flex flex-col gap-2">
    <div>
      <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1">Casing</div>
      <div class="grid grid-cols-3 gap-1">
        <button
          v-for="opt in CASING_OPTIONS"
          :key="opt.value"
          type="button"
          class="flex flex-col items-start gap-0.5 px-2 py-1.5 rounded-md border text-left transition-all duration-150"
          :class="
            modelValue.casing === opt.value
              ? 'bg-gold/[0.06] border-gold/30 text-ink'
              : 'bg-panel border-edge text-ink-muted hover:bg-raised hover:border-edge-strong'
          "
          @click="setCasing(opt.value)"
        >
          <span class="text-[11px] font-semibold">{{ opt.label }}</span>
          <span class="text-[9px] text-ink-faint truncate w-full">{{ opt.helper }}</span>
        </button>
      </div>
    </div>

    <div>
      <div class="text-[10px] font-semibold text-ink-faint tracking-[0.02em] mb-1">Punctuation</div>
      <div class="grid grid-cols-2 gap-1">
        <button
          v-for="opt in PUNCTUATION_BASE_OPTIONS"
          :key="opt.value"
          type="button"
          class="flex flex-col items-start gap-0.5 px-2 py-1.5 rounded-md border text-left transition-all duration-150"
          :class="
            punctuationKind === opt.value
              ? 'bg-gold/[0.06] border-gold/30 text-ink'
              : 'bg-panel border-edge text-ink-muted hover:bg-raised hover:border-edge-strong'
          "
          @click="setPunctuationKind(opt.value)"
        >
          <span class="text-[11px] font-semibold">{{ opt.label }}</span>
          <span class="text-[9px] text-ink-faint truncate w-full">{{ opt.helper }}</span>
        </button>
      </div>
      <div v-if="punctuationKind === 'custom'" class="mt-1.5">
        <BaseCard tone="dashed">
          <div class="flex flex-col gap-1">
            <span class="text-[10px] text-ink-faint">
              Characters to KEEP (everything else stripped). Each character counts once.
            </span>
            <BaseInput
              size="sm"
              :model-value="customChars"
              placeholder=". - / @"
              @update:model-value="setCustomChars($event)"
            />
          </div>
        </BaseCard>
      </div>
    </div>

    <SettingsRow
      label="Capitalize after sentence ends"
      helper="When casing is Sentence, also uppercase the first letter after . ! ?"
    >
      <BaseToggle
        :model-value="modelValue.autoCapitalizeAfterSentence"
        :disabled="modelValue.casing !== 'sentence'"
        @update:model-value="patch({ autoCapitalizeAfterSentence: $event })"
      />
    </SettingsRow>

    <SettingsRow
      label="Strip trailing period"
      helper="Remove . ! ? from the very end of the transcription."
    >
      <BaseToggle
        :model-value="modelValue.removeTrailingPeriod"
        @update:model-value="patch({ removeTrailingPeriod: $event })"
      />
    </SettingsRow>

    <SettingsRow
      label="Collapse whitespace"
      helper="Collapse runs of whitespace to a single space and trim edges."
    >
      <BaseToggle
        :model-value="modelValue.collapseWhitespace"
        @update:model-value="patch({ collapseWhitespace: $event })"
      />
    </SettingsRow>
  </div>
</template>
