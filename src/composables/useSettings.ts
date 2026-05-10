import { ref, computed, onMounted } from "vue";
import {
  getSettings,
  updateSettings,
  getAvailableModels,
  getAvailableCorrectionModels,
  restartFnKeyMonitor,
  type UserSettings,
  type ModelInfo,
  type CorrectionModelInfo,
} from "@/lib/commands";
import { settingToCode, codeToSetting } from "@/lib/languages";

export function useSettings() {
  const settings = ref<UserSettings | null>(null);
  const models = ref<ModelInfo[]>([]);
  const correctionModels = ref<CorrectionModelInfo[]>([]);
  const loading = ref(true);

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
      const [s, m, cm] = await Promise.all([
        getSettings(),
        getAvailableModels(),
        getAvailableCorrectionModels(),
      ]);
      settings.value = s;
      models.value = m;
      correctionModels.value = cm;
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
    mode: "holdFn" | "doubleTapFn" | "shortcut",
  ) {
    if (!settings.value) return;
    settings.value = { ...settings.value, activationMode: mode };
    await persist();
    await restartFnKeyMonitor();
  }

  /** Toggle auto-start */
  async function updateAutoStart(enabled: boolean) {
    if (!settings.value) return;
    settings.value = { ...settings.value, autoStart: enabled };
    await persist();
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
    updateLanguage,
    updateActivationMode,
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
