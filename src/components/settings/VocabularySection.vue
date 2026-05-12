<script setup lang="ts">
import { ref, onMounted } from 'vue';
import { useSettings } from '@/composables/useSettings';
import {
  getVocabulary,
  addVocabularyEntry,
  removeVocabularyEntry,
  clearVocabulary,
  type VocabularyEntry,
} from '@/lib/commands';

const { settings, updateVocabularyLearning } = useSettings();

const vocabularyEntries = ref<VocabularyEntry[]>([]);
const showAddVocab = ref(false);
const vocabWrong = ref('');
const vocabCorrect = ref('');
const confirmClearVocab = ref(false);

async function loadVocabulary() {
  try {
    vocabularyEntries.value = await getVocabulary();
  } catch (e) {
    console.error('Failed to load vocabulary:', e);
  }
}

async function handleAddVocab() {
  const wrong = vocabWrong.value.trim();
  const correct = vocabCorrect.value.trim();
  if (!wrong || !correct || wrong === correct) return;
  try {
    await addVocabularyEntry(wrong, correct);
    vocabWrong.value = '';
    vocabCorrect.value = '';
    showAddVocab.value = false;
    await loadVocabulary();
  } catch (e) {
    console.error('Failed to add vocabulary entry:', e);
  }
}

async function handleRemoveVocab(wrong: string) {
  try {
    await removeVocabularyEntry(wrong);
    await loadVocabulary();
  } catch (e) {
    console.error('Failed to remove vocabulary entry:', e);
  }
}

async function handleClearVocab() {
  if (!confirmClearVocab.value) {
    confirmClearVocab.value = true;
    setTimeout(() => {
      confirmClearVocab.value = false;
    }, 3000);
    return;
  }
  confirmClearVocab.value = false;
  try {
    await clearVocabulary();
    await loadVocabulary();
  } catch (e) {
    console.error('Failed to clear vocabulary:', e);
  }
}

onMounted(loadVocabulary);
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
          <path d="M4 19.5A2.5 2.5 0 016.5 17H20" />
          <path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z" />
        </svg>
      </div>
      <span class="section-label">Vocabulary</span>
      <span
        v-if="vocabularyEntries.length > 0"
        class="ml-auto text-[9px] text-ink-faint tabular-nums"
      >
        {{ vocabularyEntries.length }} {{ vocabularyEntries.length === 1 ? 'word' : 'words' }}
      </span>
    </div>

    <!-- Learn from corrections toggle -->
    <div class="flex items-center justify-between p-2.5 rounded-lg bg-panel border border-edge">
      <div class="flex flex-col min-w-0 mr-3">
        <span class="text-[12px] font-semibold text-ink"> Learn from corrections </span>
        <span class="text-[10px] text-ink-faint leading-snug mt-0.5">
          Automatically learn words when you correct transcriptions
        </span>
      </div>
      <button
        class="toggle-switch flex-shrink-0"
        :class="settings?.vocabularyLearning ? 'toggle-on' : 'toggle-off'"
        @click="updateVocabularyLearning(!settings?.vocabularyLearning)"
      >
        <div class="toggle-thumb" />
      </button>
    </div>

    <!-- Vocabulary entries list -->
    <div v-if="vocabularyEntries.length > 0" class="mt-2">
      <span class="subsection-label">Learned words</span>
      <div class="flex flex-col gap-1 mt-1.5">
        <div
          v-for="entry in vocabularyEntries"
          :key="entry.wrong"
          class="group flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-panel border border-edge"
        >
          <span class="text-[11px] text-ink-muted truncate">{{ entry.wrong }}</span>
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="flex-shrink-0 text-ink-faint"
          >
            <line x1="5" y1="12" x2="19" y2="12" />
            <polyline points="12 5 19 12 12 19" />
          </svg>
          <span class="text-[11px] font-semibold text-ink truncate">{{ entry.correct }}</span>
          <span
            class="ml-auto flex-shrink-0 px-1 py-0.5 rounded text-[8px] font-bold uppercase tracking-wider"
            :class="
              entry.source === 'auto'
                ? 'bg-gold/10 text-gold/80 border border-gold/15'
                : 'bg-raised text-ink-faint border border-edge'
            "
          >
            {{ entry.source === 'auto' ? 'auto' : 'manual' }}
          </span>
          <button
            class="flex-shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 text-ink-faint hover:text-flame transition-all duration-150"
            @click="handleRemoveVocab(entry.wrong)"
          >
            <svg
              width="10"
              height="10"
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
      </div>
    </div>

    <!-- Add word form -->
    <div class="mt-2">
      <button
        v-if="!showAddVocab"
        class="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg w-full bg-panel border border-edge border-dashed text-[11px] text-ink-faint font-medium hover:bg-raised hover:text-ink-muted hover:border-edge-strong transition-all duration-150"
        @click="showAddVocab = true"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        Add word
      </button>

      <div v-else class="flex flex-col gap-2 p-2.5 rounded-lg bg-panel border border-edge">
        <div class="flex gap-2 items-center">
          <input
            v-model="vocabWrong"
            type="text"
            placeholder="Wrong word"
            class="flex-1 min-w-0 px-2 py-1 text-[11px] rounded-md bg-raised border border-edge text-ink placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 transition-colors duration-150"
            @keydown.enter="handleAddVocab"
          />
          <svg
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="flex-shrink-0 text-ink-faint"
          >
            <line x1="5" y1="12" x2="19" y2="12" />
            <polyline points="12 5 19 12 12 19" />
          </svg>
          <input
            v-model="vocabCorrect"
            type="text"
            placeholder="Correct word"
            class="flex-1 min-w-0 px-2 py-1 text-[11px] rounded-md bg-raised border border-edge text-ink placeholder:text-ink-faint/50 focus:outline-none focus:border-gold/40 transition-colors duration-150"
            @keydown.enter="handleAddVocab"
          />
        </div>
        <div class="flex gap-1.5 justify-end">
          <button
            class="px-2 py-0.5 rounded-md text-[10px] font-semibold text-ink-faint hover:text-ink transition-colors duration-150"
            @click="
              showAddVocab = false;
              vocabWrong = '';
              vocabCorrect = '';
            "
          >
            Cancel
          </button>
          <button
            class="px-2.5 py-0.5 rounded-md text-[10px] font-semibold bg-gold text-canvas hover:bg-gold-hover transition-all duration-150 active:scale-95 disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="
              !vocabWrong.trim() ||
              !vocabCorrect.trim() ||
              vocabWrong.trim() === vocabCorrect.trim()
            "
            @click="handleAddVocab"
          >
            Save
          </button>
        </div>
      </div>
    </div>

    <!-- Clear all button -->
    <button
      v-if="vocabularyEntries.length > 0"
      class="mt-2 flex items-center justify-center gap-1.5 w-full px-2.5 py-1.5 rounded-lg text-[10px] font-semibold transition-all duration-150"
      :class="
        confirmClearVocab
          ? 'bg-flame/15 border border-flame/30 text-flame'
          : 'bg-panel border border-edge text-ink-faint hover:text-flame hover:border-flame/20 hover:bg-flame/5'
      "
      @click="handleClearVocab"
    >
      {{ confirmClearVocab ? 'Click again to clear all' : 'Clear all words' }}
    </button>
  </section>
</template>
