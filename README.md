# Speech to Text

A small Linux desktop app that records your microphone and transcribes it with
OpenAI's `gpt-4o-transcribe`. Built with [GPUI Kit](https://gpui-kit.com) for the
UI and [rig](https://rig.rs) for the OpenAI call.

- Start/stop listening with one button
- Each transcript is a card with its date and time
- Copy one card, or tick several and copy them together
- Delete a card with the × button (asks for confirmation)
- Transcripts and settings are saved locally as JSON

## Requirements

- Rust (stable, edition 2024)
- Ubuntu/Debian system libraries:

  ```bash
  sudo apt install libasound2-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev
  ```

- An OpenAI API key

## OpenAI API key

Open the app, click the ⚙ button in the top-right corner, paste your key and
click **Save**. If no key is saved, the app falls back to the `OPENAI_API_KEY`
environment variable.

## Run from source

```bash
cargo run
```

## Install

The install script builds a release binary and adds the app to your
application launcher (no sudo needed):

```bash
./scripts/install.sh
```

It installs:

| File | Location |
| --- | --- |
| Binary | `~/.local/bin/rs-speech-to-text` |
| Launcher entry | `~/.local/share/applications/rs-speech-to-text.desktop` |
| Icon | `~/.local/share/icons/hicolor/scalable/apps/rs-speech-to-text.svg` |

After installing, search for **Speech to Text** in Activities. To update the
installed app after changing the code, run the script again.

## Uninstall

```bash
./scripts/install.sh --uninstall
```

This removes the binary, launcher entry and icon. Your transcripts and
settings are kept; delete them manually if you want a clean removal:

```bash
rm -rf ~/.local/share/rs-speech-to-text ~/.config/rs-speech-to-text
```

## Data locations

| Data | Location |
| --- | --- |
| Transcripts | `~/.local/share/rs-speech-to-text/transcripts.json` |
| Settings (API key) | `~/.config/rs-speech-to-text/settings.json` (mode `600`) |

## Reducing the binary size

The default release binary is about 63 MB because it keeps debug symbols.
Adding this to `Cargo.toml` usually brings it down to roughly 25–35 MB, at the
cost of slower release builds:

```toml
[profile.release]
strip = true
lto = "thin"
codegen-units = 1
```

Then reinstall with `./scripts/install.sh`.

## Tests

```bash
cargo test
```
