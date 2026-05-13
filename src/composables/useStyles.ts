import { ref, computed } from 'vue';
import { onStylesChanged } from '@/lib/events';
import {
  getStyles,
  addStyle as addStyleCmd,
  updateStyle as updateStyleCmd,
  deleteStyle as deleteStyleCmd,
  duplicateStyle as duplicateStyleCmd,
  resetStyleToDefault as resetStyleToDefaultCmd,
  previewStyle as previewStyleCmd,
  validateTransform as validateTransformCmd,
  type Style,
  type TextTransform,
  type ValidationResult,
} from '@/lib/commands';

const styles = ref<Style[]>([]);
const loading = ref(true);

let loadPromise: Promise<void> | null = null;
let listenerInstalled = false;

async function load() {
  try {
    styles.value = await getStyles();
  } catch (e) {
    console.error('Failed to load styles:', e);
  } finally {
    loading.value = false;
  }
}

async function ensureLoaded(): Promise<void> {
  if (!loadPromise) {
    loadPromise = load();
    if (!listenerInstalled) {
      listenerInstalled = true;
      await onStylesChanged(() => {
        void load();
      });
    }
  }
  return loadPromise;
}

async function addStyle(style: Style): Promise<Style> {
  const added = await addStyleCmd(style);
  await load();
  return added;
}

async function updateStyle(id: string, style: Style): Promise<Style> {
  const updated = await updateStyleCmd(id, style);
  await load();
  return updated;
}

async function deleteStyle(id: string): Promise<void> {
  await deleteStyleCmd(id);
  await load();
}

async function duplicateStyle(id: string): Promise<Style> {
  const dup = await duplicateStyleCmd(id);
  await load();
  return dup;
}

async function resetStyleToDefault(id: string): Promise<Style> {
  const reset = await resetStyleToDefaultCmd(id);
  await load();
  return reset;
}

async function previewStyle(style: Style, sampleText: string): Promise<string> {
  return previewStyleCmd(style, sampleText);
}

async function validateTransform(transform: TextTransform): Promise<ValidationResult> {
  return validateTransformCmd(transform);
}

function getStyle(id: string): Style | undefined {
  return styles.value.find((s) => s.id === id);
}

const builtinStyles = computed(() => styles.value.filter((s) => s.builtin));
const userStyles = computed(() => styles.value.filter((s) => !s.builtin));

export function useStyles() {
  void ensureLoaded();

  return {
    styles,
    loading,
    builtinStyles,
    userStyles,
    getStyle,
    addStyle,
    updateStyle,
    deleteStyle,
    duplicateStyle,
    resetStyleToDefault,
    previewStyle,
    validateTransform,
    reload: load,
  };
}
