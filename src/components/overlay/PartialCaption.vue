<script setup lang="ts">
import { ref, watch, onMounted, nextTick } from 'vue';

// Live-caption pill rendered above the recording pill while the streaming
// worker emits partials. Each call to `text` is the latest cumulative
// partial — usually a prefix-extended version of the previous one. New
// words fade / translate / blur in (staggered), while existing words stay
// put. The pill's `width` is set explicitly in pixels so it can transition
// smoothly to fit the new content rather than snapping.
//
// When the appended content would push the pill past `max-width`, the slot
// is cleared and only the words newly arrived from whisper this update are
// rendered — so the user always sees a coherent partial line that fits.
const props = defineProps<{
  text: string;
}>();

const WORD_DURATION_MS = 280;
const WORD_STAGGER_MS = 40;
const WORD_TRANSLATE_Y_PX = 8;
const WORD_BLUR_PX = 2;
const WIDTH_DURATION_MS = 320;
const WIDTH_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';
const WORD_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';

const pillRef = ref<HTMLDivElement | null>(null);
const slotRef = ref<HTMLSpanElement | null>(null);
// What's currently rendered in the slot. After an overflow-driven reset,
// this is a suffix of the cumulative whisper output, not the full thing.
let displayedText = '';
// The full cumulative partial from whisper at the time of the last render.
// Diffed against the next incoming `props.text` to compute which words are
// new this update (the only ones to animate, and the candidates kept after
// an overflow reset).
let previousFullText = '';

function getMaxWidthPx(el: HTMLElement): number {
  const computed = parseFloat(getComputedStyle(el).maxWidth);
  return Number.isFinite(computed) ? computed : Number.POSITIVE_INFINITY;
}

function splitWords(s: string): string[] {
  return s.trim().split(/\s+/).filter(Boolean);
}

function commonPrefixCount(a: string[], b: string[]): number {
  const max = Math.min(a.length, b.length);
  let n = 0;
  while (n < max && a[n] === b[n]) n++;
  return n;
}

// Replace `slot` contents with one `<span class="caption-word">` per word.
// `allVisible=true` locks every span at its final state (used on initial
// render, where the pill's overall enter animation handles the fade-in).
function renderWords(slot: HTMLElement, text: string, allVisible: boolean) {
  slot.innerHTML = '';
  const words = splitWords(text);
  words.forEach((w, i) => {
    const span = document.createElement('span');
    span.className = 'caption-word';
    span.textContent = (i === 0 ? '' : ' ') + w;
    if (allVisible) {
      span.style.opacity = '1';
      span.style.transform = 'translateY(0)';
      span.style.filter = 'blur(0)';
    }
    slot.appendChild(span);
  });
}

// Measure the pill's natural width with new content in place, then animate
// the explicit `width` property from the current rendered width to that
// target. `applyContent` mutates the slot inside the measurement step so
// the new content is what gets measured.
function growPillTo(pill: HTMLElement, applyContent: () => void) {
  const startWidth = pill.offsetWidth;

  pill.style.transition = 'none';
  pill.style.width = 'auto';
  applyContent();
  const targetWidth = pill.offsetWidth;

  pill.style.width = `${startWidth}px`;
  void pill.offsetHeight; // force reflow so the from-state actually sticks

  pill.style.transition = `width ${WIDTH_DURATION_MS}ms ${WIDTH_EASING}`;
  pill.style.width = `${targetWidth}px`;
}

// Words 0..keepCount stay static; words from keepCount onward fade /
// translate / blur in with a per-word stagger.
function animateNewWords(slot: HTMLElement, keepCount: number) {
  const spans = slot.querySelectorAll<HTMLSpanElement>('.caption-word');
  spans.forEach((span, i) => {
    if (i < keepCount) {
      span.style.transition = 'none';
      span.style.opacity = '1';
      span.style.transform = 'translateY(0)';
      span.style.filter = 'blur(0)';
      return;
    }
    span.style.transition = 'none';
    span.style.opacity = '0';
    span.style.transform = `translateY(${WORD_TRANSLATE_Y_PX}px)`;
    span.style.filter = `blur(${WORD_BLUR_PX}px)`;
  });
  // Force reflow so the from-state sticks before re-enabling transitions.
  void slot.offsetHeight;
  spans.forEach((span, i) => {
    if (i < keepCount) return;
    const delay = (i - keepCount) * WORD_STAGGER_MS;
    span.style.transition =
      `opacity ${WORD_DURATION_MS}ms ${WORD_EASING} ${delay}ms,` +
      `transform ${WORD_DURATION_MS}ms ${WORD_EASING} ${delay}ms,` +
      `filter ${WORD_DURATION_MS}ms ${WORD_EASING} ${delay}ms`;
    span.style.opacity = '1';
    span.style.transform = 'translateY(0)';
    span.style.filter = 'blur(0)';
  });
}

onMounted(() => {
  const pill = pillRef.value;
  const slot = slotRef.value;
  if (!pill || !slot) return;

  // Initial render: all words at final state. The pill's own enter
  // animation (driven by OverlayApp's <Transition name="caption">) handles
  // the fade-in; word-level animation kicks in on subsequent updates.
  renderWords(slot, props.text, true);

  // Lock in a numeric `width` so subsequent `growPillTo` has somewhere to
  // animate FROM. Without this, the first growth would snap (auto → px).
  pill.style.width = 'auto';
  pill.style.width = `${pill.offsetWidth}px`;

  displayedText = props.text;
  previousFullText = props.text;
});

watch(
  () => props.text,
  async (newText) => {
    await nextTick();
    const pill = pillRef.value;
    const slot = slotRef.value;
    if (!pill || !slot) return;

    const newFullWords = splitWords(newText);
    const prevFullWords = splitWords(previousFullText);
    const commonFull = commonPrefixCount(prevFullWords, newFullWords);

    // Pure append vs. revision. If whisper revised words it had previously
    // emitted (commonFull < prevFullWords.length), drop everything from the
    // common point forward and re-render. Otherwise, extend the currently
    // displayed text with the newly-arrived words.
    let proposedWords: string[];
    let keep: number;
    if (commonFull < prevFullWords.length) {
      proposedWords = newFullWords.slice(commonFull);
      keep = 0;
    } else {
      const displayedWords = splitWords(displayedText);
      const addedWords = newFullWords.slice(commonFull);
      proposedWords = [...displayedWords, ...addedWords];
      keep = displayedWords.length;
    }

    // Words newly arrived from whisper this update — the candidates to
    // keep after an overflow-driven reset (clear the pill, show only what's
    // new). Computed up front so the closure below can capture a stable
    // value.
    const tailWords = newFullWords.slice(commonFull);

    let finalKeep = keep;
    let finalText = proposedWords.join(' ');

    growPillTo(pill, () => {
      renderWords(slot, finalText, false);
      // Inside growPillTo, pill.style.width is "auto" so offsetWidth is
      // the natural width with the just-rendered content. If that natural
      // width breaches max-width, clear the slot and render only the
      // freshly-arrived words from this update.
      const maxW = getMaxWidthPx(pill);
      if (pill.offsetWidth > maxW && tailWords.length > 0) {
        finalText = tailWords.join(' ');
        renderWords(slot, finalText, false);
        finalKeep = 0;
      }
    });
    animateNewWords(slot, finalKeep);

    displayedText = finalText;
    previousFullText = newText;
  },
);
</script>

<template>
  <div ref="pillRef" class="partial-caption">
    <span ref="slotRef" class="caption-slot"></span>
  </div>
</template>

<style scoped>
.partial-caption {
  display: inline-block;
  max-width: 320px;
  padding: 4px 12px;
  border-radius: 12px;
  background: var(--bg-elevated);
  border: 1px solid var(--border);
  backdrop-filter: blur(20px);
  -webkit-backdrop-filter: blur(20px);
  color: var(--text-secondary, rgba(255, 255, 255, 0.92));
  font-size: 11px;
  font-weight: 400;
  line-height: 1.3;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: clip;
  will-change: width, opacity, transform, filter;
}

.caption-slot {
  display: inline-block;
}

.caption-word {
  display: inline-block;
  white-space: pre;
  will-change: opacity, transform, filter;
}
</style>
