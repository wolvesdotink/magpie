import { ref, computed } from 'vue';
import {
  getSettings,
  updateSettings,
  getAvailableModels,
  getAvailableCorrectionModels,
  restartFnKeyMonitor,
  updateGlobalShortcut,
  getLaunchAtLoginStatus,
  openLoginItemsSettings,
  type UserSettings,
  type ModelInfo,
  type CorrectionModelInfo,
  type LaunchAtLoginStatus,
} from '@/lib/commands';
import { settingToCode, codeToSetting } from '@/lib/languages';

// Module-level singleton state. Every component that calls `useSettings()`
// shares these refs, so a write in one section (e.g. ModelSection) is
// instantly visible in another (e.g. LanguageSection's `isEnglishOnlyModel`
// computed). Without this, each consumer would get its own stale snapshot
// and `persist()` (which sends the full settings object) would silently
// overwrite a sibling's recent change with a stale view.
const settings = ref<UserSettings | null>(null);
const models = ref<ModelInfo[]>([]);
const correctionModels = ref<CorrectionModelInfo[]>([]);
const loading = ref(true);
const launchAtLoginStatus = ref<LaunchAtLoginStatus>('notRegistered');

let loadPromise: Promise<void> | null = null;

async function load() {
  try {
    const [s, m, cm, las] = await Promise.all([
      getSettings(),
      getAvailableModels(),
      getAvailableCorrectionModels(),
      getLaunchAtLoginStatus().catch(() => 'notRegistered' as const),
    ]);
    settings.value = s;
    models.value = m;
    correctionModels.value = cm;
    launchAtLoginStatus.value = las;
  } catch (e) {
    console.error('Failed to load settings:', e);
  } finally {
    loading.value = false;
  }
}

function ensureLoaded(): Promise<void> {
  if (!loadPromise) {
    loadPromise = load();
  }
  return loadPromise;
}

const currentLanguageCode = computed(() => settingToCode(settings.value?.language ?? null));

const isEnglishOnlyModel = computed(() => {
  const modelId = settings.value?.selectedModel;
  if (!modelId) return false;
  const model = models.value.find((m) => m.id === modelId);
  return model?.englishOnly ?? false;
});

const currentModel = computed(() => {
  const modelId = settings.value?.selectedModel;
  if (!modelId) return null;
  return models.value.find((m) => m.id === modelId) ?? null;
});

async function persist() {
  if (!settings.value) return;
  try {
    await updateSettings(settings.value);
  } catch (e) {
    console.error('Failed to save settings:', e);
  }
}

async function updateLanguage(code: string) {
  if (!settings.value) return;
  settings.value = { ...settings.value, language: codeToSetting(code) };
  await persist();
}

async function updateActivationMode(mode: 'holdFn' | 'tapFn' | 'doubleTapFn' | 'shortcut') {
  if (!settings.value) return;
  settings.value = { ...settings.value, activationMode: mode };
  await persist();
  await restartFnKeyMonitor();
}

async function updateCustomShortcut(shortcut: string | null) {
  if (!settings.value) return;
  await updateGlobalShortcut(shortcut);
  settings.value = { ...settings.value, customShortcut: shortcut };
}

async function updateAutoStart(enabled: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, autoStart: enabled };
  await persist();
  try {
    launchAtLoginStatus.value = await getLaunchAtLoginStatus();
  } catch (e) {
    console.error('Failed to read launch-at-login status:', e);
    return;
  }
  const actuallyOn =
    launchAtLoginStatus.value === 'enabled' || launchAtLoginStatus.value === 'requiresApproval';
  if (settings.value && settings.value.autoStart !== actuallyOn) {
    settings.value = { ...settings.value, autoStart: actuallyOn };
  }
}

async function updateRemoveFillers(enabled: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, removeFillers: enabled };
  await persist();
}

async function updateSelfCorrection(enabled: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, selfCorrection: enabled };
  await persist();
}

async function updateVocabularyLearning(enabled: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, vocabularyLearning: enabled };
  await persist();
}

async function updateStreamingPreview(enabled: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, streamingPreview: enabled };
  await persist();
}

async function updateSetupComplete(completed: boolean) {
  if (!settings.value) return;
  settings.value = { ...settings.value, setupComplete: completed };
  await persist();
}

async function updateSelectedCorrectionModel(modelId: string) {
  if (!settings.value) return;
  settings.value = { ...settings.value, selectedCorrectionModel: modelId };
  await persist();
}

async function updateSelectedModel(modelId: string) {
  if (!settings.value) return;
  settings.value = { ...settings.value, selectedModel: modelId };
  await persist();
}

async function reload() {
  loading.value = true;
  loadPromise = load();
  await loadPromise;
}

export function useSettings() {
  // Trigger the one-time load on first call from any component. We don't
  // await here — the refs start as `null`/`[]`/`true` and templates that
  // read `settings.value?.foo` see them populate when the load resolves.
  void ensureLoaded();

  return {
    settings,
    models,
    correctionModels,
    loading,
    currentLanguageCode,
    isEnglishOnlyModel,
    currentModel,
    launchAtLoginStatus,
    openLoginItemsSettings,
    updateLanguage,
    updateActivationMode,
    updateCustomShortcut,
    updateAutoStart,
    updateRemoveFillers,
    updateSelfCorrection,
    updateVocabularyLearning,
    updateStreamingPreview,
    updateSetupComplete,
    updateSelectedCorrectionModel,
    updateSelectedModel,
    reload,
  };
}
