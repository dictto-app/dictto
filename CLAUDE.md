# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Build & Dev Commands

```bash
pnpm dev              # Start Tauri dev mode (frontend + backend hot-reload)
pnpm build            # Production build

# Rust-only checks (from apps/desktop/src-tauri/):
cargo check           # Fast compile check without running
cargo build           # Full build
```

## Architecture

**Dictto** is a voice-to-text Windows desktop app (Tauri v2 + React + Rust). The user holds a hotkey, speaks, releases, and cleaned text is pasted at their cursor.

### Two-Window Design

1. **Recording Bar** (`"recording-bar"` label) — Transparent floating overlay at screen bottom. Shows waveform while recording.
2. **Settings** (`"main"` label) — Hidden by default, toggled via system tray. Has tabs: General, Audio, API.

### Recording Pipeline

Triggered by `Ctrl+Win` (push-to-talk):
1. Key press -> starts CPAL audio capture
2. Key release -> stops recording, returns WAV bytes (16kHz mono)
3. Transcribe via OpenAI Whisper API
4. Clean text via OpenAI GPT
5. Inject text via clipboard (Ctrl+V)

### Backend Module Structure

```
src-tauri/src/
├── lib.rs                    # App setup, AppState
├── tray.rs                   # System tray menu
├── commands/                 # Tauri IPC command handlers
│   ├── audio.rs              # Recording controls
│   ├── transcription.rs      # Transcription
│   ├── llm.rs                # Text processing
│   ├── settings.rs           # Settings management
│   └── window.rs             # Window management
└── services/                 # Business logic
    ├── audio/recorder.rs     # CPAL capture
    ├── transcription/        # TranscriptionEngine trait + impls
    ├── llm/                  # LLMProcessor trait + impls
    ├── injector/             # Text injection (enigo + arboard)
    ├── hotkey/mod.rs         # Global shortcut (WH_KEYBOARD_LL)
    ├── pipeline.rs           # Full recording->paste orchestrator
    ├── sound/mod.rs          # UI sounds
    └── db/                   # SQLite + keyring
```

### Key Traits

- `TranscriptionEngine` — async trait for speech-to-text engines
- `LLMProcessor` — async trait for text cleanup processors

Both use `async-trait`. Import the trait explicitly when using in commands.

### Frontend Patterns

- **State**: Zustand store (`stores/appStore.ts`)
- **Events**: Tauri events (`recording-state-changed`, `waveform-data`)
- **IPC**: `invoke` from `@tauri-apps/api/core`
- **Styling**: TailwindCSS v4

## Conventions

- **Package manager**: pnpm (monorepo workspaces)
- **Error handling**: `thiserror` derive macros with `Serialize` for Tauri IPC
- **Async**: `tokio` runtime, `async-trait` for trait definitions
- **State**: `Mutex<T>` in `AppState`, accessed via `State<AppState>` in commands

## Tauri v2 Notes

- Hotkey uses `WH_KEYBOARD_LL` via Microsoft `windows` crate
- Clipboard permissions: `clipboard-manager:allow-read-text` / `allow-write-text`
- Commands must be registered in `generate_handler![]` in `lib.rs`
- Use `@tauri-apps/api/core` for `invoke` (not `@tauri-apps/api/tauri`)
