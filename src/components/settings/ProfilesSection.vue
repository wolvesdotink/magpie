<script setup lang="ts">
import { ref, computed } from 'vue';
import SettingsSection from '@/components/base/SettingsSection.vue';
import BaseButton from '@/components/base/BaseButton.vue';
import BaseCard from '@/components/base/BaseCard.vue';
import BaseToggle from '@/components/base/BaseToggle.vue';
import ProfileEditor from './profile/ProfileEditor.vue';
import { useProfiles } from '@/composables/useProfiles';
import { useStyles } from '@/composables/useStyles';
import { useConfirmAction } from '@/composables/useConfirmAction';
import type { AppProfile } from '@/lib/commands';

const emit = defineEmits<{
  'navigate-to-style': [styleId: string];
}>();

const {
  profiles,
  addProfile,
  updateProfile,
  deleteProfile,
  duplicateProfile,
  setProfileEnabled,
  resetBuiltInPresets,
} = useProfiles();
const { styles } = useStyles();
const confirmDelete = useConfirmAction();
const confirmReset = useConfirmAction();

const editingId = ref<string | null>(null);
const error = ref<string | null>(null);

const sortedProfiles = computed(() => {
  const all = [...profiles.value];
  return all.sort((a, b) => a.displayName.localeCompare(b.displayName));
});

function styleName(id: string): string {
  return styles.value.find((s) => s.id === id)?.name ?? 'Unknown style';
}

async function handleSave(profile: AppProfile) {
  error.value = null;
  try {
    await updateProfile(profile.id, profile);
    editingId.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleNew() {
  error.value = null;
  try {
    const defaultStyle = styles.value.find((s) => s.id === 'builtin-default')?.id ?? '';
    const added = await addProfile({
      id: '',
      bundleId: '',
      displayName: 'New profile',
      enabled: true,
      styleId: defaultStyle,
      vocabulary: [],
      vocabularyLearningOverride: null,
      createdAt: '',
      updatedAt: '',
    });
    editingId.value = added.id;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleDuplicate(id: string) {
  error.value = null;
  try {
    const dup = await duplicateProfile(id);
    editingId.value = dup.id;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleDelete(id: string) {
  if (!confirmDelete.confirm(id)) return;
  error.value = null;
  try {
    await deleteProfile(id);
    if (editingId.value === id) editingId.value = null;
  } catch (e) {
    error.value = String(e);
  }
}

async function handleToggle(id: string, enabled: boolean) {
  error.value = null;
  try {
    await setProfileEnabled(id, enabled);
  } catch (e) {
    error.value = String(e);
  }
}

async function handleReset() {
  if (!confirmReset.confirm()) return;
  error.value = null;
  try {
    await resetBuiltInPresets();
  } catch (e) {
    error.value = String(e);
  }
}

function handleEditStyle(styleId: string) {
  emit('navigate-to-style', styleId);
}
</script>

<template>
  <SettingsSection label="App Profiles">
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
        <rect x="3" y="3" width="7" height="7" rx="1" />
        <rect x="14" y="3" width="7" height="7" rx="1" />
        <rect x="3" y="14" width="7" height="7" rx="1" />
        <rect x="14" y="14" width="7" height="7" rx="1" />
      </svg>
    </template>
    <template #header-extra>
      <span class="ml-auto text-[9px] text-ink-faint tabular-nums">
        {{ profiles.length }} {{ profiles.length === 1 ? 'profile' : 'profiles' }}
      </span>
    </template>

    <div v-if="error" class="text-[11px] text-flame mb-2">{{ error }}</div>

    <div v-if="profiles.length === 0" class="text-[11px] text-ink-faint italic mb-2">
      No profiles yet. Click "Reset built-ins" to install Slack, Mail, Terminal, and more.
    </div>

    <div class="flex flex-col gap-1.5">
      <div v-for="profile in sortedProfiles" :key="profile.id">
        <BaseCard v-if="editingId !== profile.id" tone="neutral">
          <div class="flex items-center gap-2">
            <BaseToggle
              :model-value="profile.enabled"
              @update:model-value="handleToggle(profile.id, $event)"
            />
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5">
                <span class="text-[12px] font-semibold text-ink truncate">
                  {{ profile.displayName }}
                </span>
                <span
                  class="px-1 py-0.5 rounded text-[8px] font-medium bg-raised text-ink-faint border border-edge truncate"
                  :title="profile.bundleId"
                >
                  {{ profile.bundleId }}
                </span>
              </div>
              <div class="flex items-center gap-1 mt-0.5">
                <span class="text-[10px] text-ink-faint">Style:</span>
                <span class="text-[10px] text-ink-muted">{{ styleName(profile.styleId) }}</span>
                <span
                  v-if="profile.vocabulary.length > 0"
                  class="text-[10px] text-ink-faint ml-2"
                >
                  {{ profile.vocabulary.length }} words
                </span>
              </div>
            </div>
            <div class="flex items-center gap-1 flex-shrink-0">
              <BaseButton variant="ghost" size="sm" @click="editingId = profile.id">
                Edit
              </BaseButton>
              <BaseButton variant="ghost" size="sm" @click="handleDuplicate(profile.id)">
                Duplicate
              </BaseButton>
              <button
                class="px-2 py-1 rounded-md text-[10px] font-semibold transition-all duration-150"
                :class="
                  confirmDelete.isArmed(profile.id)
                    ? 'bg-flame/15 border border-flame/30 text-flame'
                    : 'text-ink-faint hover:text-flame hover:bg-flame/5'
                "
                @click="handleDelete(profile.id)"
              >
                {{ confirmDelete.isArmed(profile.id) ? 'Confirm' : 'Delete' }}
              </button>
            </div>
          </div>
        </BaseCard>

        <ProfileEditor
          v-else
          :profile="profile"
          @save="handleSave"
          @cancel="editingId = null"
          @edit-style="handleEditStyle"
        />
      </div>
    </div>

    <div class="flex gap-1.5 mt-2">
      <BaseButton variant="primary" size="sm" @click="handleNew">
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
        New profile
      </BaseButton>
      <button
        class="px-2.5 py-1 rounded-lg text-[10px] font-semibold transition-all duration-150"
        :class="
          confirmReset.isArmed()
            ? 'bg-flame/15 border border-flame/30 text-flame'
            : 'bg-panel border border-edge text-ink-faint hover:text-ink hover:bg-raised'
        "
        @click="handleReset"
      >
        {{ confirmReset.isArmed() ? 'Confirm reset' : 'Reset built-ins' }}
      </button>
    </div>
  </SettingsSection>
</template>
