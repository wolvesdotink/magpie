# ADR-0002: Feature flags for staged rollout

- **Status:** Accepted
- **Date:** 2026-05-12
- **Deciders:** Magpie contributors

## Context

Phase 3 introduces several features (batch / file-import transcription,
vocabulary export, persistent history) that should land in the codebase
ahead of their public ship. Local-first products can't A/B test against
remote config; we need a way to merge unfinished features without
exposing them to all users.

We also want a cheap way to flip an experimental code path on for
development without rebuilding (`MAGPIE_FEATURE_*=1 bun tauri dev`).

## Decision

Implement **compile-time feature flags resolved at runtime**, stored in
`src-tauri/src/features.rs::FeatureFlags` with this precedence:

1. `MAGPIE_FEATURE_<NAME>` env var (override; intended for dev/CI only)
2. `UserSettings` fields where the flag is also user-facing
3. `FeatureFlags::default()` — safe-by-default, every flag off until UAT'd

The frontend mirror at `src/lib/features.ts` is hand-maintained until
Phase 5's `ts-rs` generator lands. The Tauri command `get_feature_flags`
returns the resolved set once at app start; UI affordances key off it.

No remote-config service. The local-first promise rules out anything
that phones home; even a "fetch latest flag set" call would cross that
line.

## Lifecycle

```
1. Introduce flag (default=false). Code path guarded by `if flags.x`.
2. Internal testing. Override via MAGPIE_FEATURE_X=1 in dev.
3. Optional opt-in: surface in UserSettings + Settings UI.
4. UAT passes — flip default to true.
5. After one stable release with flag default-on, remove the field
   (Rust + TS) and every `if flags.x` check.
```

## Consequences

**Positive**

- Unfinished features can land on `main` without shipping to users.
- Devs and bug reporters can opt in via env without a custom build.
- The set is enumerated in one place (`FeatureFlags`), so the dead-code
  cleanup at stage 5 is mechanical.

**Negative**

- Every guard is one more conditional. Long-lived flags become tech
  debt; the lifecycle's stage 5 cleanup must actually happen.
- Frontend mirror drift: a flag added on one side and forgotten on the
  other silently disables the feature. (Mitigation: Phase 5's `ts-rs`
  generates the mirror automatically.)

**Neutral**

- No persistent toggle UI for non-user-facing flags. Internal flags
  flip via env only.

## Alternatives considered

- **Cargo features** — rejected. Cargo features are compile-time only,
  so a beta tester can't flip them without rebuilding. We need runtime
  toggles.
- **A separate `flags.toml` config** — rejected as redundant with
  `UserSettings`. Adds a second file the user can corrupt.
- **Remote config (LaunchDarkly etc.)** — rejected. Violates the
  local-first promise. Even an opt-in fetch crosses the line because
  every install would need to be explicit about whether it phones home.
