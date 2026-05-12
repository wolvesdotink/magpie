<script setup lang="ts">
import { computed } from 'vue';
import { useSettings } from '@/composables/useSettings';
import { LANGUAGES, languageLabel } from '@/lib/languages';
import Dropdown from '@/components/base/Dropdown.vue';

type Variant = 'compact' | 'full';

const props = withDefaults(
  defineProps<{
    variant?: Variant;
  }>(),
  {
    variant: 'compact',
  },
);

const { currentLanguageCode, isEnglishOnlyModel, updateLanguage, loading } = useSettings();

const explicitLanguages = LANGUAGES.slice(1);

const chipLabel = computed(() => languageLabel(currentLanguageCode.value));
const fullLabel = computed(() => {
  if (currentLanguageCode.value === 'auto') return 'Auto-detect';
  const lang = LANGUAGES.find((l) => l.code === currentLanguageCode.value);
  return lang ? lang.nativeName : currentLanguageCode.value.toUpperCase();
});

function select(code: string) {
  updateLanguage(code);
}

const placement = computed(() => (props.variant === 'compact' ? 'top' : 'bottom'));
const menuClass = computed(() => (props.variant === 'compact' ? 'right-0 w-52' : 'left-0 right-0'));
</script>

<template>
  <Dropdown :placement="placement" :menu-class="menuClass">
    <template #trigger="{ open, toggle }">
      <!-- Compact chip trigger -->
      <button
        v-if="variant === 'compact'"
        type="button"
        class="inline-flex items-center gap-1 px-1.5 py-0.5 text-[10px] font-semibold leading-none bg-raised rounded border shadow-soft transition-colors duration-150"
        :class="
          open
            ? 'border-gold/30 text-ink-muted'
            : 'border-edge text-ink-faint hover:text-ink-muted hover:border-edge-strong'
        "
        @click="toggle"
      >
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
          <path
            d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"
          />
        </svg>
        <span>{{ chipLabel }}</span>
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

      <!-- Full row trigger -->
      <button
        v-else
        type="button"
        class="w-full flex items-center justify-between p-2.5 rounded-lg bg-panel border transition-all duration-150"
        :class="open ? 'border-gold/30 shadow-glow-gold' : 'border-edge hover:border-edge-strong'"
        @click="toggle"
      >
        <span class="text-[12px] font-semibold text-ink">{{ fullLabel }}</span>
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
          :class="{ 'rotate-180': open }"
        >
          <polyline points="6 9 12 15 18 9" />
        </svg>
      </button>
    </template>

    <template #menu="{ close }">
      <template v-if="!loading">
        <div
          v-if="isEnglishOnlyModel && variant === 'compact'"
          class="px-3 py-1.5 text-[10px] text-gold bg-gold/[0.06] border-b border-edge leading-snug"
        >
          Multilingual model required for other languages
        </div>
        <div
          class="overflow-y-auto py-1"
          :class="variant === 'compact' ? 'max-h-[280px]' : 'max-h-[200px]'"
        >
          <button
            type="button"
            class="flex items-center justify-between w-full text-left transition-colors duration-100"
            :class="[
              variant === 'compact' ? 'px-3 py-1.5' : 'px-2.5 py-[5px]',
              currentLanguageCode === 'auto'
                ? 'bg-gold/[0.06] text-ink'
                : 'text-ink-muted hover:bg-raised hover:text-ink',
            ]"
            @click="
              select('auto');
              close();
            "
          >
            <span
              :class="variant === 'compact' ? 'text-[12px] font-medium' : 'text-[11px] font-medium'"
            >
              Auto-detect
            </span>
            <svg
              v-if="currentLanguageCode === 'auto'"
              :width="variant === 'compact' ? 12 : 10"
              :height="variant === 'compact' ? 12 : 10"
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
          <div class="h-px bg-edge mx-2 my-0.5" />
          <button
            v-for="lang in explicitLanguages"
            :key="lang.code"
            type="button"
            class="flex items-center justify-between w-full text-left transition-colors duration-100"
            :class="[
              variant === 'compact' ? 'px-3 py-1.5' : 'px-2.5 py-[5px]',
              currentLanguageCode === lang.code ? 'bg-gold/[0.06] text-ink' : 'hover:bg-raised',
              isEnglishOnlyModel && lang.code !== 'en'
                ? 'opacity-40 pointer-events-none'
                : 'text-ink-muted hover:text-ink',
            ]"
            @click="
              select(lang.code);
              close();
            "
          >
            <div class="flex items-baseline gap-1.5 min-w-0">
              <span
                class="font-medium truncate"
                :class="variant === 'compact' ? 'text-[12px]' : 'text-[11px]'"
              >
                {{ lang.nativeName }}
              </span>
              <span
                v-if="lang.nativeName !== lang.name"
                class="text-ink-faint truncate"
                :class="variant === 'compact' ? 'text-[10px]' : 'text-[9px]'"
              >
                {{ lang.name }}
              </span>
            </div>
            <svg
              v-if="currentLanguageCode === lang.code"
              :width="variant === 'compact' ? 12 : 10"
              :height="variant === 'compact' ? 12 : 10"
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
        </div>
      </template>
    </template>
  </Dropdown>
</template>
