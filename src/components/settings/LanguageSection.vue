<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useSettings } from '@/composables/useSettings';
import { LANGUAGES } from '@/lib/languages';

const { currentLanguageCode, isEnglishOnlyModel, updateLanguage } = useSettings();

const languageOpen = ref(false);
const languageRef = ref<HTMLElement | null>(null);

const explicitLanguages = LANGUAGES.slice(1);

const currentLanguageLabel = computed(() => {
  if (currentLanguageCode.value === 'auto') return 'Auto-detect';
  const lang = LANGUAGES.find((l) => l.code === currentLanguageCode.value);
  return lang ? lang.nativeName : currentLanguageCode.value.toUpperCase();
});

function selectLanguage(code: string) {
  updateLanguage(code);
  languageOpen.value = false;
}

function onClickOutsideLang(e: MouseEvent) {
  if (languageRef.value && !languageRef.value.contains(e.target as Node)) {
    languageOpen.value = false;
  }
}

onMounted(() => document.addEventListener('mousedown', onClickOutsideLang));
onUnmounted(() => document.removeEventListener('mousedown', onClickOutsideLang));
</script>

<template>
  <section class="settings-section">
    <div class="section-header">
      <div class="section-icon">
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="10" />
          <path d="M2 12h20" />
          <path
            d="M12 2a15.3 15.3 0 014 10 15.3 15.3 0 01-4 10 15.3 15.3 0 01-4-10 15.3 15.3 0 014-10z"
          />
        </svg>
      </div>
      <span class="section-label">Language</span>
    </div>

    <!-- English-only model hint -->
    <div
      v-if="isEnglishOnlyModel"
      class="p-2 rounded-md bg-gold/[0.06] border border-gold/15 mb-2.5"
    >
      <span class="text-[10px] text-gold leading-snug">
        Switch to a multilingual model to unlock other languages.
      </span>
    </div>

    <div ref="languageRef" class="relative">
      <button
        class="w-full flex items-center justify-between p-2.5 rounded-lg bg-panel border transition-all duration-150"
        :class="
          languageOpen ? 'border-gold/30 shadow-glow-gold' : 'border-edge hover:border-edge-strong'
        "
        @click="languageOpen = !languageOpen"
      >
        <div class="flex items-center gap-2">
          <span class="text-[12px] font-semibold text-ink">
            {{ currentLanguageLabel }}
          </span>
        </div>
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="text-ink-faint transition-transform duration-200"
          :class="{ 'rotate-180': languageOpen }"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>

      <!-- Language dropdown -->
      <Transition
        enter-active-class="transition duration-150 ease-out"
        enter-from-class="opacity-0 -translate-y-1 scale-[0.98]"
        enter-to-class="opacity-100 translate-y-0 scale-100"
        leave-active-class="transition duration-100 ease-in"
        leave-from-class="opacity-100 translate-y-0 scale-100"
        leave-to-class="opacity-0 -translate-y-1 scale-[0.98]"
      >
        <div
          v-if="languageOpen"
          class="absolute top-full left-0 right-0 mt-1.5 bg-panel border border-edge rounded-lg shadow-elevated overflow-hidden z-50"
        >
          <div class="max-h-[200px] overflow-y-auto py-1">
            <!-- Auto-detect -->
            <button
              class="lang-option"
              :class="
                currentLanguageCode === 'auto'
                  ? 'bg-gold/[0.06] text-ink'
                  : 'text-ink-muted hover:bg-raised hover:text-ink'
              "
              @click="selectLanguage('auto')"
            >
              <span class="text-[11px] font-medium">Auto-detect</span>
              <svg
                v-if="currentLanguageCode === 'auto'"
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="3"
                class="text-gold"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </button>
            <div class="h-px bg-edge mx-2 my-0.5" />
            <button
              v-for="lang in explicitLanguages"
              :key="lang.code"
              class="lang-option"
              :class="[
                currentLanguageCode === lang.code ? 'bg-gold/[0.06] text-ink' : 'hover:bg-raised',
                isEnglishOnlyModel && lang.code !== 'en'
                  ? 'opacity-30 pointer-events-none'
                  : 'text-ink-muted hover:text-ink',
              ]"
              @click="selectLanguage(lang.code)"
            >
              <div class="flex items-baseline gap-1.5 min-w-0">
                <span class="text-[11px] font-medium truncate">
                  {{ lang.nativeName }}
                </span>
                <span
                  v-if="lang.nativeName !== lang.name"
                  class="text-[9px] text-ink-faint truncate"
                >
                  {{ lang.name }}
                </span>
              </div>
              <svg
                v-if="currentLanguageCode === lang.code"
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="3"
                class="text-gold flex-shrink-0"
              >
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </button>
          </div>
        </div>
      </Transition>
    </div>
  </section>
</template>

<style scoped>
.lang-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 5px 10px;
  text-align: left;
  transition: all 0.1s ease;
}
</style>
