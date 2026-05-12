<script setup lang="ts">
defineProps<{
  text: string;
}>();
</script>

<template>
  <div class="partial-caption">{{ text }}</div>
</template>

<style scoped>
.partial-caption {
  max-width: 320px;
  padding: 4px 12px;
  border-radius: 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  color: var(--text-secondary, rgba(255, 255, 255, 0.85));
  font-size: 11px;
  font-weight: 400;
  line-height: 1.3;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  will-change: opacity, transform;
}
</style>
