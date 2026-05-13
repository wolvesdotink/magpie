<script setup lang="ts">
import { ref, watch } from 'vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import BaseInput from '@/components/base/BaseInput.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import { validateTransform, type TextTransform, type TransformKind } from '@/lib/commands';

const props = defineProps<{
  modelValue: TextTransform[];
}>();

const emit = defineEmits<{
  'update:modelValue': [value: TextTransform[]];
}>();

const errors = ref<Record<string, string>>({});

function patch(idx: number, p: Partial<TextTransform>) {
  const next = [...props.modelValue];
  next[idx] = { ...next[idx], ...p };
  emit('update:modelValue', next);
}

function patchKind(idx: number, kind: TransformKind) {
  patch(idx, { kind });
}

function addRule() {
  const newRule: TextTransform = {
    id: `tx-${Date.now()}-${Math.floor(Math.random() * 10000)}`,
    enabled: true,
    label: null,
    kind: {
      kind: 'replace',
      pattern: '',
      replacement: '',
      isRegex: false,
      caseSensitive: false,
      wholeWord: false,
    },
  };
  emit('update:modelValue', [...props.modelValue, newRule]);
}

function removeRule(idx: number) {
  const next = [...props.modelValue];
  next.splice(idx, 1);
  emit('update:modelValue', next);
}

function move(idx: number, delta: number) {
  const target = idx + delta;
  if (target < 0 || target >= props.modelValue.length) return;
  const next = [...props.modelValue];
  const [item] = next.splice(idx, 1);
  next.splice(target, 0, item);
  emit('update:modelValue', next);
}

function setKindKind(idx: number, kind: TransformKind['kind']) {
  let newKind: TransformKind;
  switch (kind) {
    case 'replace':
      newKind = {
        kind: 'replace',
        pattern: '',
        replacement: '',
        isRegex: false,
        caseSensitive: false,
        wholeWord: false,
      };
      break;
    case 'prepend':
      newKind = { kind: 'prepend', text: '' };
      break;
    case 'append':
      newKind = { kind: 'append', text: '' };
      break;
    case 'trimEdges':
      newKind = { kind: 'trimEdges' };
      break;
    case 'squeezeChars':
      newKind = { kind: 'squeezeChars', chars: '' };
      break;
  }
  patchKind(idx, newKind);
}

// Debounced validation on every change.
let validateTimer: ReturnType<typeof setTimeout> | null = null;

watch(
  () => props.modelValue,
  (next) => {
    if (validateTimer) clearTimeout(validateTimer);
    validateTimer = setTimeout(async () => {
      const out: Record<string, string> = {};
      for (const t of next) {
        if (!t.enabled) continue;
        try {
          const result = await validateTransform(t);
          if (!result.ok && result.error) {
            out[t.id] = result.error;
          }
        } catch (e) {
          out[t.id] = String(e);
        }
      }
      errors.value = out;
    }, 250);
  },
  { deep: true, immediate: true },
);

const KIND_LABELS: { value: TransformKind['kind']; label: string }[] = [
  { value: 'replace', label: 'Find & replace' },
  { value: 'prepend', label: 'Prepend text' },
  { value: 'append', label: 'Append text' },
  { value: 'trimEdges', label: 'Trim edges' },
  { value: 'squeezeChars', label: 'Squeeze chars' },
];

const hasErrors = (id: string) => Boolean(errors.value[id]);
</script>

<template>
  <div class="flex flex-col gap-2">
    <div v-if="modelValue.length === 0" class="text-[10px] text-ink-faint italic">
      No custom rules. Add one below to transform the text after vocabulary and before casing.
    </div>

    <div v-for="(rule, idx) in modelValue" :key="rule.id" class="flex flex-col gap-1.5">
      <BaseCard :tone="hasErrors(rule.id) ? 'flame' : 'neutral'">
        <div class="flex flex-col gap-2">
          <div class="flex items-center gap-2">
            <BaseToggle
              :model-value="rule.enabled"
              @update:model-value="patch(idx, { enabled: $event })"
            />
            <BaseInput
              size="sm"
              :model-value="rule.label ?? ''"
              placeholder="Rule label (optional)"
              class="flex-1"
              @update:model-value="patch(idx, { label: $event || null })"
            />
            <div class="flex items-center gap-0.5">
              <button
                class="p-1 rounded-md text-ink-faint hover:text-ink hover:bg-raised disabled:opacity-30 disabled:cursor-not-allowed"
                :disabled="idx === 0"
                title="Move up"
                @click="move(idx, -1)"
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
                >
                  <polyline points="18 15 12 9 6 15" />
                </svg>
              </button>
              <button
                class="p-1 rounded-md text-ink-faint hover:text-ink hover:bg-raised disabled:opacity-30 disabled:cursor-not-allowed"
                :disabled="idx === modelValue.length - 1"
                title="Move down"
                @click="move(idx, 1)"
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
                >
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
              <button
                class="p-1 rounded-md text-ink-faint hover:text-flame"
                title="Delete rule"
                @click="removeRule(idx)"
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
                >
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </button>
            </div>
          </div>

          <div class="flex items-center gap-1.5 flex-wrap">
            <select
              :value="rule.kind.kind"
              class="rounded-md bg-raised border border-edge text-ink text-[11px] px-2 py-1 focus:outline-none focus:border-gold/40"
              @change="
                setKindKind(
                  idx,
                  ($event.target as HTMLSelectElement).value as TransformKind['kind'],
                )
              "
            >
              <option v-for="k in KIND_LABELS" :key="k.value" :value="k.value">
                {{ k.label }}
              </option>
            </select>
          </div>

          <template v-if="rule.kind.kind === 'replace'">
            <div class="flex items-center gap-1.5">
              <BaseInput
                size="sm"
                :model-value="rule.kind.pattern"
                placeholder="Find"
                class="flex-1"
                @update:model-value="
                  patchKind(idx, {
                    ...(rule.kind as Extract<TransformKind, { kind: 'replace' }>),
                    pattern: $event,
                  })
                "
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
                size="sm"
                :model-value="rule.kind.replacement"
                placeholder="Replace with"
                class="flex-1"
                @update:model-value="
                  patchKind(idx, {
                    ...(rule.kind as Extract<TransformKind, { kind: 'replace' }>),
                    replacement: $event,
                  })
                "
              />
            </div>
            <div class="flex items-center gap-3 text-[11px] text-ink-muted">
              <label class="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  :checked="rule.kind.isRegex"
                  class="rounded border-edge"
                  @change="
                    patchKind(idx, {
                      ...(rule.kind as Extract<TransformKind, { kind: 'replace' }>),
                      isRegex: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
                Regex
              </label>
              <label class="flex items-center gap-1 cursor-pointer">
                <input
                  type="checkbox"
                  :checked="rule.kind.caseSensitive"
                  class="rounded border-edge"
                  @change="
                    patchKind(idx, {
                      ...(rule.kind as Extract<TransformKind, { kind: 'replace' }>),
                      caseSensitive: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
                Match case
              </label>
              <label
                class="flex items-center gap-1 cursor-pointer"
                :class="rule.kind.isRegex ? 'opacity-40' : ''"
              >
                <input
                  type="checkbox"
                  :checked="rule.kind.wholeWord"
                  :disabled="rule.kind.isRegex"
                  class="rounded border-edge"
                  @change="
                    patchKind(idx, {
                      ...(rule.kind as Extract<TransformKind, { kind: 'replace' }>),
                      wholeWord: ($event.target as HTMLInputElement).checked,
                    })
                  "
                />
                Whole word
              </label>
            </div>
          </template>

          <template v-if="rule.kind.kind === 'prepend'">
            <BaseInput
              size="sm"
              :model-value="rule.kind.text"
              placeholder="Text to prepend"
              @update:model-value="patchKind(idx, { kind: 'prepend', text: $event })"
            />
          </template>

          <template v-if="rule.kind.kind === 'append'">
            <BaseInput
              size="sm"
              :model-value="rule.kind.text"
              placeholder="Text to append"
              @update:model-value="patchKind(idx, { kind: 'append', text: $event })"
            />
          </template>

          <template v-if="rule.kind.kind === 'squeezeChars'">
            <BaseInput
              size="sm"
              :model-value="rule.kind.chars"
              placeholder="Characters to squeeze (e.g. .,;)"
              @update:model-value="patchKind(idx, { kind: 'squeezeChars', chars: $event })"
            />
          </template>

          <template v-if="rule.kind.kind === 'trimEdges'">
            <span class="text-[10px] text-ink-faint italic"
              >No options — trims leading and trailing whitespace.</span
            >
          </template>

          <div v-if="errors[rule.id]" class="text-[10px] text-flame">
            {{ errors[rule.id] }}
          </div>
        </div>
      </BaseCard>
    </div>

    <BaseButton variant="secondary" size="sm" full-width @click="addRule">
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
        <line x1="12" y1="5" x2="12" y2="19" />
        <line x1="5" y1="12" x2="19" y2="12" />
      </svg>
      Add rule
    </BaseButton>
  </div>
</template>
