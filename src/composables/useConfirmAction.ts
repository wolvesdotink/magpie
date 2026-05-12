import { ref } from 'vue';

/**
 * Two-step confirmation pattern. First call arms the action (returns false),
 * second call within `timeoutMs` runs the work (returns true). After the
 * timeout the arm resets.
 *
 * Used for destructive buttons like "Delete model" and "Clear all words".
 */
export function useConfirmAction(timeoutMs = 3000) {
  /** Key of the currently-armed action, or null. Per-row callers pass the
   *  row's id so multiple rows can share one composable instance. */
  const armed = ref<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clear() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    armed.value = null;
  }

  /**
   * @returns true when the action is confirmed and the caller should run it.
   */
  function confirm(key: string = '_'): boolean {
    if (armed.value === key) {
      clear();
      return true;
    }
    if (timer !== null) clearTimeout(timer);
    armed.value = key;
    timer = setTimeout(() => {
      if (armed.value === key) clear();
    }, timeoutMs);
    return false;
  }

  function isArmed(key: string = '_'): boolean {
    return armed.value === key;
  }

  return { armed, confirm, isArmed, clear };
}
