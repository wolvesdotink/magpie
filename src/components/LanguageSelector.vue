<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useSettings } from "@/composables/useSettings";
import { LANGUAGES, languageLabel } from "@/lib/languages";

const { currentLanguageCode, isEnglishOnlyModel, updateLanguage, loading } =
  useSettings();

const open = ref(false);
const dropdownRef = ref<HTMLElement | null>(null);
const triggerRef = ref<HTMLElement | null>(null);

const chipLabel = computed(() => languageLabel(currentLanguageCode.value));

const explicitLanguages = LANGUAGES.slice(1);

function select(code: string) {
  updateLanguage(code);
  open.value = false;
}

function toggle() {
  open.value = !open.value;
}

// Close on click outside
function onClickOutside(e: MouseEvent) {
  if (
    dropdownRef.value &&
    !dropdownRef.value.contains(e.target as Node) &&
    triggerRef.value &&
    !triggerRef.value.contains(e.target as Node)
  ) {
    open.value = false;
  }
}

onMounted(() => document.addEventListener("mousedown", onClickOutside));
onUnmounted(() => document.removeEventListener("mousedown", onClickOutside));
</script>

<template>
  <div class="relative">
    <!-- Trigger chip -->
    <button
      ref="triggerRef"
      class="inline-flex items-center gap-1 px-1.5 py-0.5
             text-[10px] font-semibold leading-none
             bg-raised rounded border border-edge shadow-soft
             text-ink-faint hover:text-ink-muted hover:border-edge-strong
             transition-colors duration-150"
      :class="{ 'border-gold/30 text-ink-muted': open }"
      @click="toggle"
    >
      <!-- Globe icon -->
      <svg
        width="10"
        height="10"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="opacity-70"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M2 12h20" />
        <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
      </svg>
      <span>{{ chipLabel }}</span>
      <!-- Chevron -->
      <svg
        width="8"
        height="8"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="opacity-50 transition-transform duration-150"
        :class="{ 'rotate-180': open }"
      >
        <polyline points="18 15 12 9 6 15" />
      </svg>
    </button>

    <!-- Dropdown (opens upward) -->
    <Transition
      enter-active-class="transition duration-150 ease-out"
      enter-from-class="opacity-0 translate-y-1"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-100 ease-in"
      leave-from-class="opacity-100 translate-y-0"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-if="open && !loading"
        ref="dropdownRef"
        class="absolute bottom-full right-0 mb-1.5 w-52
               bg-panel border border-edge rounded-lg shadow-elevated
               overflow-hidden z-50"
      >
        <!-- English-only model hint -->
        <div
          v-if="isEnglishOnlyModel"
          class="px-3 py-1.5 text-[10px] text-gold bg-gold/[0.06]
                 border-b border-edge leading-snug"
        >
          Multilingual model required for other languages
        </div>

        <div class="max-h-[280px] overflow-y-auto py-1">
          <!-- Auto-detect option -->
          <button
            class="flex items-center justify-between w-full px-3 py-1.5
                   text-left transition-colors duration-100"
            :class="
              currentLanguageCode === 'auto'
                ? 'bg-gold/[0.06] text-ink'
                : 'text-ink-muted hover:bg-raised hover:text-ink'
            "
            @click="select('auto')"
          >
            <span class="text-[12px] font-medium">Auto-detect</span>
            <svg
              v-if="currentLanguageCode === 'auto'"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="3"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="text-gold flex-shrink-0"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>

          <!-- Divider -->
          <div class="h-px bg-edge mx-2 my-1" />

          <!-- Language list -->
          <button
            v-for="lang in explicitLanguages"
            :key="lang.code"
            class="flex items-center justify-between w-full px-3 py-1.5
                   text-left transition-colors duration-100"
            :class="[
              currentLanguageCode === lang.code
                ? 'bg-gold/[0.06] text-ink'
                : 'hover:bg-raised',
              isEnglishOnlyModel && lang.code !== 'en'
                ? 'opacity-40 text-ink-faint'
                : 'text-ink-muted hover:text-ink',
            ]"
            @click="select(lang.code)"
          >
            <div class="flex items-baseline gap-1.5 min-w-0">
              <span class="text-[12px] font-medium truncate">
                {{ lang.nativeName }}
              </span>
              <span
                v-if="lang.nativeName !== lang.name"
                class="text-[10px] text-ink-faint truncate"
              >
                {{ lang.name }}
              </span>
            </div>
            <svg
              v-if="currentLanguageCode === lang.code"
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="3"
              stroke-linecap="round"
              stroke-linejoin="round"
              class="text-gold flex-shrink-0 ml-2"
            >
              <polyline points="20 6 9 17 4 12" />
            </svg>
          </button>
        </div>
      </div>
    </Transition>
  </div>
</template>
