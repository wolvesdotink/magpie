import { ref } from 'vue';
import { onProfilesChanged } from '@/lib/events';
import {
  getProfiles,
  addProfile as addProfileCmd,
  updateProfile as updateProfileCmd,
  deleteProfile as deleteProfileCmd,
  duplicateProfile as duplicateProfileCmd,
  setProfileEnabled as setProfileEnabledCmd,
  resetBuiltInPresets as resetBuiltInPresetsCmd,
  getFrontmostApp,
  type AppProfile,
  type FrontmostApp,
} from '@/lib/commands';

const profiles = ref<AppProfile[]>([]);
const loading = ref(true);

let loadPromise: Promise<void> | null = null;
let listenerInstalled = false;

async function load() {
  try {
    profiles.value = await getProfiles();
  } catch (e) {
    console.error('Failed to load profiles:', e);
  } finally {
    loading.value = false;
  }
}

async function ensureLoaded(): Promise<void> {
  if (!loadPromise) {
    loadPromise = load();
    if (!listenerInstalled) {
      listenerInstalled = true;
      await onProfilesChanged(() => {
        void load();
      });
    }
  }
  return loadPromise;
}

async function addProfile(profile: AppProfile): Promise<AppProfile> {
  const added = await addProfileCmd(profile);
  await load();
  return added;
}

async function updateProfile(id: string, profile: AppProfile): Promise<AppProfile> {
  const updated = await updateProfileCmd(id, profile);
  await load();
  return updated;
}

async function deleteProfile(id: string): Promise<void> {
  await deleteProfileCmd(id);
  await load();
}

async function duplicateProfile(id: string): Promise<AppProfile> {
  const dup = await duplicateProfileCmd(id);
  await load();
  return dup;
}

async function setProfileEnabled(id: string, enabled: boolean): Promise<void> {
  await setProfileEnabledCmd(id, enabled);
  await load();
}

async function resetBuiltInPresets(): Promise<void> {
  await resetBuiltInPresetsCmd();
  await load();
}

async function detectFrontmostApp(): Promise<FrontmostApp | null> {
  return getFrontmostApp();
}

export function useProfiles() {
  void ensureLoaded();

  return {
    profiles,
    loading,
    addProfile,
    updateProfile,
    deleteProfile,
    duplicateProfile,
    setProfileEnabled,
    resetBuiltInPresets,
    detectFrontmostApp,
    reload: load,
  };
}
