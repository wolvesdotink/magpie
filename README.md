<p align="center">
  <img src="app-icon.svg" alt="Magpie" width="160" height="160">
</p>

# Magpie

Open-source, local-first voice-to-text for macOS.

Hold a hotkey, dictate, and the transcript is pasted at your cursor — entirely on-device. Transcription is powered by [Whisper](https://github.com/openai/whisper) and [Distil-Whisper](https://github.com/huggingface/distil-whisper) running through [whisper.cpp](https://github.com/ggerganov/whisper.cpp), with Metal GPU acceleration and Apple Neural Engine encoding via CoreML where available. Optional self-correction uses [llama.cpp](https://github.com/ggerganov/llama.cpp). No accounts, no cloud, no telemetry.

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
pnpm install
pnpm tauri dev         # run in development
pnpm run build:mac     # local release build (.app + .dmg)
```

Release builds (signed, notarized, with auto-updater payload) happen in CI when a `v*.*.*` tag is pushed. See [`.github/workflows/release.yml`](.github/workflows/release.yml) and [`scripts/build-macos.sh`](scripts/build-macos.sh).

### Live captions and translation

Enable **Live transcription preview** in Transcription settings to see captions while dictating. Previews use a rolling audio window; the final transcript is decoded from the full recording after you stop.

Choose an **Output language** in Language settings to translate captions and final text locally. Translation uses the language model selected in Transcription settings and works independently of the self-correction toggle. Qwen3.5 2B or 4B is recommended; quality and latency depend on the language, model, and hardware. **Same as spoken** turns translation off.

The model picker includes Distil Large v3.5 for English and compact Q5 versions of Whisper Large v3 and Large v3 Turbo. These run through the existing Whisper backend; models from other inference engines are not compatible with this picker.

## License

Magpie is licensed under the [MIT License](LICENSE).
