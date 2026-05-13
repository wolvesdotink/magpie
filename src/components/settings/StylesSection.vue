<script setup lang="ts">
import { ref, computed } from 'vue';
import SettingsSection from '@/components/base/SettingsSection.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import StyleEditor from './style/StyleEditor.vue';
import { useStyles } from '@/composables/useStyles';
import { useProfiles } from '@/composables/useProfiles';
import { useConfirmAction } from '@/composables/useConfirmAction';
import type { Style } from '@/lib/commands';

const {
  styles,
  addStyle,
  updateStyle,
  deleteStyle,
  duplicateStyle,
  resetStyleToDefault,
} = useStyles();
const { profiles, resetBuiltInPresets } = useProfiles();
const confirmDelete = useConfirmAction();
const confirmReset = useConfirmAction();

const editingId = ref<string | null>(null);
const error = ref<string | null>(null);

function usageCount(styleId: string): number {
  return profiles.value.filter((p) => p.styleId === styleId).length;
}

const sortedStyles = computed(() => {
  // Built-ins first, then user styles, both alphabetically inside their group.
  const all = [...styles.value];
  return all.sort((a, b) => {
    if (a.builtin !== b.builtin) return a.builtin ? -1 : 1;
    return a.name.localeCompare(b.name);
  });
});

async function handleEdit(id: string) {
  editingId.value = id;
  error.value = null;
}

async function handleSave(updated: Style) {
  error.value = null;
  try {
    await updateStyle(updated.id, updated);
    editingId.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleNewStyle() {
  error.value = null;
  try {
    const blank: Style = {
      id: '',
      name: 'Untitled style',
      description: null,
      builtin: false,
      formatting: {
        casing: 'sentence',
        punctuation: { kind: 'auto' },
        removeTrailingPeriod: false,
        autoCapitalizeAfterSentence: false,
        collapseWhitespace: true,
      },
      correction: { kind: 'inherit' },
      customRules: [],
      fillerOverride: null,
      createdAt: '',
      updatedAt: '',
    };
    const added = await addStyle(blank);
    editingId.value = added.id;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleDuplicate(id: string) {
  error.value = null;
  try {
    const dup = await duplicateStyle(id);
    editingId.value = dup.id;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleDelete(id: string) {
  if (!confirmDelete.confirm(id)) return;
  error.value = null;
  try {
    await deleteStyle(id);
    if (editingId.value === id) editingId.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleReset(id: string) {
  error.value = null;
  try {
    await resetStyleToDefault(id);
  } catch (e) {
    error.value = String(e);
  }
}

async function handleResetAll() {
  if (!confirmReset.confirm()) return;
  error.value = null;
  try {
    await resetBuiltInPresets();
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <SettingsSection label="Styles">
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
        <circle cx="13.5" cy="6.5" r="2.5" />
        <path d="M19 19H5l4-6 3 4 2-3z" />
      </svg>
    </template>
    <template #header-extra>
      <span class="ml-auto text-[9px] text-ink-faint tabular-nums">
        {{ styles.length }} {{ styles.length === 1 ? 'style' : 'styles' }}
      </span>
    </template>

    <div v-if="error" class="text-[11px] text-flame mb-2">{{ error }}</div>

    <div class="flex flex-col gap-1.5">
      <div v-for="style in sortedStyles" :key="style.id">
        <BaseCard
          v-if="editingId !== style.id"
          tone="neutral"
          :interactive="false"
          class="group"
        >
          <div class="flex items-center gap-2">
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="text-[12px] font-semibold text-ink truncate">{{ style.name }}</span>
                <span
                  v-if="style.builtin"
                  class="px-1 py-0.5 rounded text-[8px] font-bold uppercase tracking-wider bg-gold/10 text-gold/80 border border-gold/15"
                >
                  built-in
                </span>
                <span
                  v-if="usageCount(style.id) > 0"
                  class="px-1 py-0.5 rounded text-[8px] font-medium bg-raised text-ink-faint border border-edge"
                >
                  used by {{ usageCount(style.id) }}
                </span>
              </div>
              <div v-if="style.description" class="text-[10px] text-ink-faint mt-0.5 truncate">
                {{ style.description }}
              </div>
            </div>

            <div class="flex items-center gap-1 flex-shrink-0">
              <BaseButton variant="ghost" size="sm" @click="handleEdit(style.id)">Edit</BaseButton>
              <BaseButton variant="ghost" size="sm" @click="handleDuplicate(style.id)">
                Duplicate
              </BaseButton>
              <BaseButton
                v-if="style.builtin"
                variant="ghost"
                size="sm"
                @click="handleReset(style.id)"
              >
                Reset
              </BaseButton>
              <button
                v-else
                class="px-2 py-1 rounded-md text-[10px] font-semibold transition-all duration-150"
                :class="
                  confirmDelete.isArmed(style.id)
                    ? 'bg-flame/15 border border-flame/30 text-flame'
                    : 'text-ink-faint hover:text-flame hover:bg-flame/5'
                "
                @click="handleDelete(style.id)"
              >
                {{ confirmDelete.isArmed(style.id) ? 'Confirm' : 'Delete' }}
              </button>
            </div>
          </div>
        </BaseCard>

        <StyleEditor
          v-if="editingId === style.id"
          :style="style"
          @save="handleSave"
          @cancel="editingId = null"
        />
      </div>
    </div>

    <div class="flex gap-1.5 mt-2">
      <BaseButton variant="primary" size="sm" @click="handleNewStyle">
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
        New style
      </BaseButton>
      <button
        class="px-2.5 py-1 rounded-lg text-[10px] font-semibold transition-all duration-150"
        :class="
          confirmReset.isArmed()
            ? 'bg-flame/15 border border-flame/30 text-flame'
            : 'bg-panel border border-edge text-ink-faint hover:text-ink hover:bg-raised'
        "
        @click="handleResetAll"
      >
        {{ confirmReset.isArmed() ? 'Confirm reset built-ins' : 'Reset built-ins' }}
      </button>
    </div>
  </SettingsSection>
</template>
