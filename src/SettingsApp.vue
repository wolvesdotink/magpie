<script setup lang="ts">
import { getCurrentWindow } from '@tauri-apps/api/window';
import SettingsView from '@/components/SettingsView.vue';

const appWindow = getCurrentWindow();

async function closeWindow() {
  await appWindow.hide();
}
</script>

<template>
  <div class="w-full h-full p-2">
    <div
      class="flex flex-col h-full bg-canvas rounded-xl overflow-hidden relative surface-grain ring-1 ring-white/[0.06] shadow-elevated"
    >
      <!-- Decorative top edge -->
      <div class="h-[1.5px] bg-gradient-to-r from-transparent via-ink-faint/20 to-transparent" />

      <!-- Custom draggable title bar -->
      <div
        data-tauri-drag-region
        class="flex items-center justify-between px-5 pt-4 pb-3 min-h-[44px] cursor-grab active:cursor-grabbing select-none"
      >
        <h1 class="text-[15px] font-bold tracking-tight text-ink">Settings</h1>
        <button
          class="flex items-center justify-center w-6 h-6 rounded-md text-ink-faint hover:text-ink hover:bg-raised transition-all duration-150 active:scale-90"
          title="Close"
          @click="closeWindow"
        >
          <svg
            width="12"
            height="12"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <!-- Settings content -->
      <SettingsView :standalone="true" />
    </div>
  </div>
</template>
