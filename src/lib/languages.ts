export interface Language {
  /** ISO 639-1 code, or "auto" for auto-detect */
  code: string;
  /** English name */
  name: string;
  /** Name in the language itself */
  nativeName: string;
}

/**
 * Curated list of Whisper's most common languages.
 * Full list: https://github.com/openai/whisper/blob/main/whisper/tokenizer.py
 */
export const LANGUAGES: Language[] = [
  { code: 'auto', name: 'Auto-detect', nativeName: 'Auto-detect' },
  { code: 'en', name: 'English', nativeName: 'English' },
  { code: 'de', name: 'German', nativeName: 'Deutsch' },
  { code: 'fr', name: 'French', nativeName: 'Fran\u00e7ais' },
  { code: 'es', name: 'Spanish', nativeName: 'Espa\u00f1ol' },
  { code: 'it', name: 'Italian', nativeName: 'Italiano' },
  { code: 'pt', name: 'Portuguese', nativeName: 'Portugu\u00eas' },
  { code: 'nl', name: 'Dutch', nativeName: 'Nederlands' },
  { code: 'pl', name: 'Polish', nativeName: 'Polski' },
  { code: 'ru', name: 'Russian', nativeName: '\u0420\u0443\u0441\u0441\u043a\u0438\u0439' },
  {
    code: 'uk',
    name: 'Ukrainian',
    nativeName: '\u0423\u043a\u0440\u0430\u0457\u043d\u0441\u044c\u043a\u0430',
  },
  { code: 'ja', name: 'Japanese', nativeName: '\u65e5\u672c\u8a9e' },
  { code: 'ko', name: 'Korean', nativeName: '\ud55c\uad6d\uc5b4' },
  { code: 'zh', name: 'Chinese', nativeName: '\u4e2d\u6587' },
  { code: 'ar', name: 'Arabic', nativeName: '\u0627\u0644\u0639\u0631\u0628\u064a\u0629' },
  { code: 'hi', name: 'Hindi', nativeName: '\u0939\u093f\u0928\u094d\u0926\u0940' },
  { code: 'tr', name: 'Turkish', nativeName: 'T\u00fcrk\u00e7e' },
  { code: 'sv', name: 'Swedish', nativeName: 'Svenska' },
  { code: 'da', name: 'Danish', nativeName: 'Dansk' },
  { code: 'no', name: 'Norwegian', nativeName: 'Norsk' },
  { code: 'fi', name: 'Finnish', nativeName: 'Suomi' },
  { code: 'cs', name: 'Czech', nativeName: '\u010ce\u0161tina' },
];

/** Map a settings value (string | null) to a UI language code */
export function settingToCode(setting: string | null): string {
  return setting ?? 'auto';
}

/** Map a UI language code back to a settings value */
export function codeToSetting(code: string): string | null {
  return code === 'auto' ? null : code;
}

/** Find a language by its code, falling back to auto-detect */
export function findLanguage(code: string): Language {
  return LANGUAGES.find((l) => l.code === code) ?? LANGUAGES[0];
}

/** Get the short display label for a language code */
export function languageLabel(code: string): string {
  if (code === 'auto') return 'Auto';
  return code.toUpperCase();
}
