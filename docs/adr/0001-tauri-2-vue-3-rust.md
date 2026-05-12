# ADR-0001: Tauri 2 + Vue 3 + Rust for desktop app shell

- **Status:** Accepted (retrocaptured)
- **Date:** 2026-05-12
- **Deciders:** Magpie contributors

## Context

Magpie is a local-first voice-to-text app for macOS. The core compute path is
native Rust (whisper.cpp, llama.cpp, cpal). We needed a UI shell that:

- Embeds native Rust without IPC overhead larger than a function call
- Ships a small binary (no full Chromium runtime — that would dwarf the ML models)
- Supports macOS tray icon, multiple windows (main / overlay / settings), and
  the Tauri updater plugin
- Can plausibly grow to Windows / Linux / iOS later without a rewrite

## Decision

Use **Tauri 2** as the desktop shell, with a **Vue 3 + TypeScript** frontend
built by **Vite**, talking to a **Rust** backend via Tauri's `invoke` /
event bridge.

## Consequences

**Positive**

- Single binary; no embedded Chromium (uses system WebView)
- Rust backend hosts all native dependencies in-process — no FFI or sidecar
  overhead for whisper.cpp / llama.cpp calls
- Tauri 2's plugin ecosystem covers global shortcuts, updater, tray, store,
  notifications, dialog — first-class support, not third-party patches
- Vue 3 + Vite gives sub-second HMR; small frontend bundle
- Cross-platform groundwork already done by Tauri — adding Linux / Windows is
  a contained change (Phase 4 of the architecture plan), not a fork

**Negative**

- macOS uses WKWebView; rendering edge cases differ from Chromium. The overlay
  window's transparency + always-on-top behavior required platform-specific
  tweaks
- Tauri's bundling does not handle dynamically-linked dylibs (we use
  `dynamic-link` on `llama-cpp-2` to avoid GGML symbol collisions with
  `whisper-rs-sys`). We had to write our own `scripts/build-macos.sh` to
  inject dylibs + patch install names — a real, ongoing maintenance cost
- TypeScript ↔ Rust type contract is hand-maintained today (Phase 5 plans
  to generate it via `ts-rs`)

**Neutral**

- The three-window architecture (main / overlay / settings) is a Tauri
  feature, not a Vue feature; switching to a different frontend framework
  would not change the window topology

## Alternatives considered

- **Electron** — rejected. Bundle size (~120 MB minimum before ML models)
  would dwarf the app payload and look hostile to a "local-first, no bloat"
  pitch. Memory footprint is also large for an always-on tray app.
- **Native macOS (SwiftUI + Swift)** — rejected. Would block any future
  cross-platform work entirely. Also splits the dev surface between Swift
  (UI) and Rust (compute) when Tauri lets us keep one Rust core.
- **Slint / egui / native Rust GUI** — rejected. Settings UI ergonomics
  matter (this is a daily-use app), and Vue + Tailwind delivers polish
  faster than retained-mode Rust GUI today.
- **Tauri 1** — rejected. Tauri 2 is the supported line going forward;
  starting on 1 would mean a forced migration in months.
