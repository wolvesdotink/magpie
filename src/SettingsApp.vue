<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import SettingsView from '@/components/SettingsView.vue';

const appWindow = getCurrentWindow();

async function closeWindow() {
  await appWindow.hide();
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') {
    e.preventDefault();
    void closeWindow();
    return;
  }
  if (e.key.toLowerCase() === 'w' && e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
    e.preventDefault();
    void closeWindow();
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleKeydown);
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});
</script>

<template>
  <div class="flex flex-col h-full bg-canvas surface-grain">
    <!-- Native macOS title bar overlay: reserve room for the traffic lights
         on the left, render our title centered on the right of them.
         `="deep"` lets the H1 itself trigger drag (interactive children are
         auto-excluded by Tauri's drag.js). -->
    <div
      data-tauri-drag-region="deep"
      class="flex items-center pl-[80px] pr-4 h-[44px] flex-shrink-0 select-none"
    >
      <h1 class="text-[13px] font-semibold tracking-tight text-ink">Settings</h1>
    </div>

    <SettingsView :standalone="true" />
  </div>
</template>
