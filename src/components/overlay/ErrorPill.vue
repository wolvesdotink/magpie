<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  /** Raw error message. Truncated for display; the full text lives in the
   *  Tauri log + dev console. */
  message: string;
}>();

// Truncate long error messages so the pill doesn't grow unbounded.
const ERROR_MAX_CHARS = 80;
const short = computed(() =>
  props.message.length > ERROR_MAX_CHARS
    ? props.message.slice(0, ERROR_MAX_CHARS - 1) + '…'
    : props.message,
);
</script>

<template>
  <div class="error-pill" role="alert">
    <span class="error-icon" aria-hidden="true">!</span>
    <span class="error-text">{{ short }}</span>
  </div>
</template>

<style scoped>
.error-pill {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 320px;
  padding: 5px 12px;
  border-radius: 14px;
  background: var(--bg-elevated);
  border: 1px solid rgba(224, 85, 85, 0.45);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  color: var(--recording, #e05555);
  font-size: 11px;
  font-weight: 500;
  line-height: 1.3;
  text-align: left;
  box-shadow: 0 0 12px rgba(224, 85, 85, 0.18);
  will-change: opacity, transform;
}
.error-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: rgba(224, 85, 85, 0.18);
  font-size: 10px;
  font-weight: 700;
  flex-shrink: 0;
}
.error-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
