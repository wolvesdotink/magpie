# Magpie

Open-source, local-first voice-to-text for macOS.

Hold a hotkey, dictate, and the transcript is pasted at your cursor — entirely on-device. Powered by [Whisper](https://github.com/openai/whisper) for transcription and [llama.cpp](https://github.com/ggerganov/llama.cpp) for optional self-correction. No accounts, no cloud, no telemetry.

**Website:** [wolves.ink/projects/magpie](https://wolves.ink/projects/magpie)
**By:** [Wolves Software](https://wolves.ink)

## Install

Download the latest `Magpie.dmg` from the [releases page](https://github.com/wolvesdotink/magpie/releases/latest), open it, and drag Magpie to Applications.

The app is signed and notarized by Apple, so it should open without warnings on first launch.

### Requirements

- macOS 13 (Ventura) or later
- Apple Silicon (arm64) — Intel support is on the roadmap

## Build from source

```bash
bun install
bun tauri dev          # run in development
bun run build:mac      # local release build (.app + .dmg)
```

Release builds (signed, notarized, with auto-updater payload) happen in CI when a `v*.*.*` tag is pushed. See [`.github/workflows/release.yml`](.github/workflows/release.yml) and [`scripts/build-macos.sh`](scripts/build-macos.sh).

## License

Magpie is licensed under the [MIT License](LICENSE).
