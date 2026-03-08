# Dictto

**Voice-to-text for Windows. Hold a hotkey, speak, release — clean text appears at your cursor.**

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![GitHub Release](https://img.shields.io/github/v/release/dictto-app/dictto)](https://github.com/dictto-app/dictto/releases/latest)

<!-- TODO: Add demo GIF/video here -->

## Features

- **Push-to-talk**: Hold `Ctrl+Win`, speak, release — text is pasted where your cursor is
- **Works everywhere**: Any text field in any app (VS Code, Slack, Notepad, browsers)
- **Smart cleanup**: AI removes filler words ("um", "uh"), fixes grammar, formats text
- **Spanglish support**: Handles mixed Spanish/English naturally
- **BYOK (Bring Your Own Key)**: Use your own OpenAI API key — your data, your control
- **Lightweight**: ~15MB installer, minimal resource usage (built with Tauri + Rust)
- **Privacy-first**: Audio is processed and discarded — never stored

## Install

1. Download the latest `.exe` from [GitHub Releases](https://github.com/dictto-app/dictto/releases/latest)
2. Run the installer
3. Windows SmartScreen may show a warning — click "More info" then "Run anyway" (we're working on code signing)
4. Open Dictto from the system tray
5. Go to Settings > API and enter your OpenAI API key
6. Hold `Ctrl+Win` and start talking!

## Build from source

**Prerequisites:** Node.js (LTS), pnpm, Rust (stable), Tauri CLI

```bash
git clone https://github.com/dictto-app/dictto.git
cd dictto
pnpm install
pnpm dev        # Development mode with hot-reload
pnpm build      # Production build (outputs .exe installer)
```

## How it works

1. **Hotkey press** (`Ctrl+Win`) — starts recording from your microphone
2. **Hotkey release** — stops recording, sends audio to OpenAI Whisper API for transcription
3. **AI cleanup** — GPT removes filler words, fixes grammar, formats text
4. **Text injection** — cleaned text is pasted at your cursor via clipboard

## Tech stack

- **Frontend:** React + TypeScript + TailwindCSS
- **Backend:** Rust (Tauri v2)
- **Transcription:** OpenAI Whisper API (BYOK)
- **Text cleanup:** OpenAI GPT (BYOK)
- **Audio:** CPAL (native Rust audio capture)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup instructions and guidelines.

## License

[AGPL-3.0](LICENSE) — you can use, modify, and distribute Dictto freely. If you modify it and distribute or run it as a service, you must share your changes under the same license.
