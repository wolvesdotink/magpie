import { ref, onUnmounted } from 'vue';

/**
 * Tiny toast helper. Sets a value, clears it after `timeoutMs`.
 * Returns reactive `value` plus `show(payload)` to fire a new toast.
 */
export function useToast<T>(timeoutMs = 3000) {
  const value = ref<T | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function show(payload: T) {
    value.value = payload;
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(() => {
      value.value = null;
      timer = null;
    }, timeoutMs);
  }

  function dismiss() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    value.value = null;
  }

  onUnmounted(dismiss);

  return { value, show, dismiss };
}
