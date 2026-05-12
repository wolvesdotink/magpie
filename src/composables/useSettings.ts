import { ref, computed, onMounted } from "vue";
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
} from "@/lib/commands";
import { settingToCode, codeToSetting } from "@/lib/languages";

export function useSettings() {
  const settings = ref<UserSettings | null>(null);
  const models = ref<ModelInfo[]>([]);
  const correctionModels = ref<CorrectionModelInfo[]>([]);
  const loading = ref(true);
  const launchAtLoginStatus = ref<LaunchAtLoginStatus>("notRegistered");

  /** Current language as a UI code ("auto", "en", "de", …) */
  const currentLanguageCode = computed(() =>
    settingToCode(settings.value?.language ?? null),
  );

  /** Whether the currently selected model is English-only */
  const isEnglishOnlyModel = computed(() => {
    const modelId = settings.value?.selectedModel;
    if (!modelId) return false;
    const model = models.value.find((m) => m.id === modelId);
    return model?.englishOnly ?? false;
  });

  /** Currently selected model info */
  const currentModel = computed(() => {
    const modelId = settings.value?.selectedModel;
    if (!modelId) return null;
    return models.value.find((m) => m.id === modelId) ?? null;
  });

  async function load() {
    try {
      const [s, m, cm, las] = await Promise.all([
        getSettings(),
        getAvailableModels(),
        getAvailableCorrectionModels(),
        getLaunchAtLoginStatus().catch(() => "notRegistered" as const),
      ]);
      settings.value = s;
      models.value = m;
      correctionModels.value = cm;
      launchAtLoginStatus.value = las;
    } catch (e) {
      console.error("Failed to load settings:", e);
    } finally {
      loading.value = false;
    }
  }

  /** Persist the current settings ref */
  async function persist() {
    if (!settings.value) return;
    try {
      await updateSettings(settings.value);
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  /** Update the transcription language */
  async function updateLanguage(code: string) {
    if (!settings.value) return;
    settings.value = { ...settings.value, language: codeToSetting(code) };
    await persist();
  }

  /** Update activation mode */
  async function updateActivationMode(
    mode: "holdFn" | "tapFn" | "doubleTapFn" | "shortcut",
  ) {
    if (!settings.value) return;
    settings.value = { ...settings.value, activationMode: mode };
    await persist();
    await restartFnKeyMonitor();
  }

  /**
   * Update the user's custom global shortcut. The backend re-registers the
   * shortcut and persists the setting; we only mirror it back into the local
   * ref on success. Errors propagate so the UI can surface them.
   */
  async function updateCustomShortcut(shortcut: string | null) {
    if (!settings.value) return;
    await updateGlobalShortcut(shortcut);
    settings.value = { ...settings.value, customShortcut: shortcut };
  }

  /**
   * Toggle launch-at-login. Persisting the setting also triggers the
   * backend register/unregister via SMAppService, then we re-read the
   * actual OS state — registration can resolve to `requiresApproval` or
   * fail silently in dev (unstable code signing). If the OS state doesn't
   * match the requested state, snap the toggle to reality so the UI
   * doesn't lie to the user.
   */
  async function updateAutoStart(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, autoStart: enabled };
    await persist();
    try {
      launchAtLoginStatus.value = await getLaunchAtLoginStatus();
    } catch (e) {
      console.error("Failed to read launch-at-login status:", e);
      return;
    }
    const actuallyOn =
      launchAtLoginStatus.value === "enabled" ||
      launchAtLoginStatus.value === "requiresApproval";
    if (settings.value && settings.value.autoStart !== actuallyOn) {
      settings.value = { ...settings.value, autoStart: actuallyOn };
    }
  }

  /** Toggle filler word removal */
  async function updateRemoveFillers(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, removeFillers: enabled };
    await persist();
  }

  /** Toggle self-correction detection */
  async function updateSelfCorrection(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, selfCorrection: enabled };
    await persist();
  }

  /** Toggle vocabulary learning from corrections */
  async function updateVocabularyLearning(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, vocabularyLearning: enabled };
    await persist();
  }

  /** Toggle live partial-caption preview while recording */
  async function updateStreamingPreview(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, streamingPreview: enabled };
    await persist();
  }

  /** Mark setup wizard as complete */
  async function updateSetupComplete(completed: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, setupComplete: completed };
    await persist();
  }

  /** Update selected correction model in settings */
  async function updateSelectedCorrectionModel(modelId: string) {
    if (!settings.value) return;
    settings.value = { ...settings.value, selectedCorrectionModel: modelId };
    await persist();
  }

  /** Update selected model in settings */
  async function updateSelectedModel(modelId: string) {
    if (!settings.value) return;
    settings.value = { ...settings.value, selectedModel: modelId };
    await persist();
  }

  /** Reload settings and models from backend */
  async function reload() {
    loading.value = true;
    await load();
  }

  onMounted(load);

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
