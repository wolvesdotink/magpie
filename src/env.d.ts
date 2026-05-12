/// <reference types="vite/client" />

declare module '*.vue' {
  import type { DefineComponent } from 'vue';
  // The triple-`any` shape is the canonical Vue 3 + TS module declaration —
  // narrowing it breaks `defineComponent` callers that pass any prop/emit shape.
  // ESLint sees `{}` as suspicious; in this declarative context it means
  // "no required prop or data shape", which is correct.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any, @typescript-eslint/no-empty-object-type
  const component: DefineComponent<{}, {}, any>;
  export default component;
}
