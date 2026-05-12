// Flat-config ESLint for Vue 3 + TypeScript. Flat config is the default in
// ESLint 9 and is what `eslint .` picks up automatically.
//
// Keep this list tight. We want failures to be actionable, not aesthetic.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import pluginVue from 'eslint-plugin-vue';
import prettier from 'eslint-config-prettier';

export default [
  // Files ESLint should ignore (flat-config replacement for .eslintignore).
  {
    ignores: [
      'dist/**',
      'dist-release/**',
      'node_modules/**',
      'src-tauri/target/**',
      'src-tauri/gen/**',
      '**/*.config.ts',
      '**/*.config.js',
      'eslint.config.js',
    ],
  },

  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...pluginVue.configs['flat/recommended'],

  // Vue/TS source files
  {
    files: ['**/*.{ts,tsx,vue}'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        ecmaVersion: 'latest',
        sourceType: 'module',
        extraFileExtensions: ['.vue'],
      },
    },
    rules: {
      // We use `vue-tsc --noEmit` for type errors; don't double-report here.
      '@typescript-eslint/no-explicit-any': 'warn',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],

      // Vue conventions — strict enough to keep PRs consistent.
      'vue/multi-word-component-names': 'off', // App.vue, etc.
      'vue/no-v-html': 'warn',
      'vue/component-name-in-template-casing': ['error', 'PascalCase'],
    },
  },

  // Disable any rule that conflicts with Prettier. Keep this last.
  prettier,
];
