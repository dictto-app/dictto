# Privacy Policy — Dictto

**Last updated:** March 2026

## Summary

Dictto is a privacy-first voice-to-text app. Your audio and text stay on your device. We collect zero telemetry. You bring your own API key — we never see it.

## How Dictto Works

1. You hold a hotkey and speak
2. Audio is captured locally on your device
3. Audio is sent directly from your device to OpenAI's API for transcription and text cleanup
4. The transcribed text is pasted at your cursor
5. Audio is discarded immediately — it is never saved to disk or sent anywhere else

## Your OpenAI API Key (BYOK)

Dictto uses a **Bring Your Own Key** model:

- You provide your own OpenAI API key in Settings
- Your API key is stored locally in Windows Credential Locker (encrypted by Windows)
- Your API key is sent directly from your device to OpenAI — Dictto has no servers that see your key
- Your relationship with OpenAI is governed by [OpenAI's privacy policy](https://openai.com/privacy) and [usage policies](https://openai.com/policies/usage-policies)

## Data We Collect

**None.** Dictto v0.1 collects zero telemetry, zero analytics, zero crash reports. There are no Dictto servers — the app runs entirely on your device.

## Data Stored Locally

Dictto stores the following data on your device only:

| Data | Location | Purpose |
|---|---|---|
| Settings (language, microphone, etc.) | `%LOCALAPPDATA%\com.dictto.app\dictto.db` | App preferences |
| Transcription history | `%LOCALAPPDATA%\com.dictto.app\dictto.db` | Your reference |
| OpenAI API key | Windows Credential Locker | Authentication with OpenAI |

You can delete all local data by uninstalling Dictto and choosing "Remove all data" during uninstallation, or by manually deleting the `%LOCALAPPDATA%\com.dictto.app\` folder.

## Data Sent to Third Parties

| Recipient | Data sent | When | Why |
|---|---|---|---|
| OpenAI | Audio recording (WAV) | Each transcription | Speech-to-text via Whisper API |
| OpenAI | Transcribed text | Each text cleanup | Grammar/filler word cleanup via GPT |

No data is sent to Dictto or any other party. The app communicates directly with OpenAI's API using your personal API key.

## No Accounts, No Servers

Dictto does not require an account. There is no Dictto backend, no user database, no authentication system. The app is fully self-contained on your device.

## Future Changes

If a future version of Dictto adds telemetry (e.g., anonymous usage analytics), it will be:

- **Opt-out** — enabled by default but easily disabled in Settings
- **Anonymous** — no personal data, no transcription content, no API keys
- **Documented** — this privacy policy will be updated before any telemetry is added

## Open Source

Dictto's source code is available at [github.com/dictto-app/dictto](https://github.com/dictto-app/dictto) under the AGPL-3.0 license. You can audit exactly what data the app accesses and where it sends it.

## Contact

For privacy questions, open an issue on [GitHub](https://github.com/dictto-app/dictto/issues).
