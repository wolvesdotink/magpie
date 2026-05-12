// Discriminated-union mirror of `src-tauri/src/command_error.rs::CommandError`.
//
// Every Tauri command surfaces errors in this shape. Switch on `kind` to drive
// targeted UI affordances (e.g. show a Settings → Permissions link when
// `permissionDenied` arrives, vs. a generic toast for `other`).
//
// Until Phase 5 promotes us to `ts-rs`-generated types, this file is hand-
// maintained. Drift will surface as a runtime mismatch — keep this in lockstep
// with the Rust enum.

export type CommandError =
  | { kind: 'other'; message: string; details?: string }
  | { kind: 'io'; message: string }
  | { kind: 'network'; message: string }
  | { kind: 'audioDevice'; message: string }
  | { kind: 'backendNotLoaded' }
  | { kind: 'transcription'; message: string }
  | { kind: 'cancelled' }
  | { kind: 'modelNotFound'; modelId: string }
  | { kind: 'correction'; message: string }
  | { kind: 'output'; message: string }
  | { kind: 'permissionDenied'; permission: string }
  | { kind: 'settings'; message: string }
  | { kind: 'invalidArgument'; message: string };

/** Discriminated kind strings, useful for exhaustive switches. */
export type CommandErrorKind = CommandError['kind'];

/**
 * Narrow an `unknown` (from a failed `invoke`) into a `CommandError`.
 *
 * Tauri's `invoke` rejects with the JSON value the backend returned. Today
 * every command returns either `CommandError`-shaped JSON or — until the
 * Phase 1 migration is complete — a legacy `string`. We coerce both into a
 * uniform `CommandError` so call sites only have to handle one shape.
 */
export function toCommandError(value: unknown): CommandError {
  if (typeof value === 'string') {
    return { kind: 'other', message: value };
  }
  if (value && typeof value === 'object' && 'kind' in value) {
    // Trusted by construction (Rust side serializes the enum directly).
    return value as CommandError;
  }
  return {
    kind: 'other',
    message: value instanceof Error ? value.message : String(value ?? 'unknown error'),
  };
}

/** Human-readable summary for toast/banner display. */
export function formatCommandError(err: CommandError): string {
  switch (err.kind) {
    case 'modelNotFound':
      return `Model not found: ${err.modelId}`;
    case 'permissionDenied':
      return `Permission denied: ${err.permission}`;
    case 'backendNotLoaded':
      return 'No transcription model is loaded.';
    case 'cancelled':
      return 'Operation cancelled.';
    case 'other':
    case 'io':
    case 'network':
    case 'audioDevice':
    case 'transcription':
    case 'correction':
    case 'output':
    case 'settings':
    case 'invalidArgument':
      return err.message;
  }
}
