# Music Drums

Menu bar app for macOS that taps system audio and buzzes an MX Master 4 on the beat.

Swift UI, Rust core, Apple Silicon. Talks to the mouse over HID++ (`0x19B0`), based on the MasterMice / logiops reverse engineering work.

**While Drums mode is on, Logi Options+ is parked.** Bindings and Actions Ring will not work until you turn Drums mode off (or run `Scripts/logi-mode.sh enable`).

## Requirements

- macOS 14.4+
- Apple Silicon
- MX Master 4 (Bolt or Bluetooth)
- Rust (`rustup`) and Xcode CLT / Swift 6

## Quick start

```bash
# CLI pulse test (park Options+ first)
./Scripts/logi-mode.sh disable
cargo run -p music_drums_core --release --bin music-drums-cli -- pulse 70 strong
./Scripts/logi-mode.sh enable

# Menu bar app
./Scripts/build.sh
open build/MusicDrums.app
```

Grant Audio Capture the first time you enable Drums mode.

## Usage

1. Open Music Drums from the menu bar.
2. Turn on Drums mode (Options+ parks, engine starts).
3. Play music; tweak Sensitivity.
4. Test pulse fires a strong hit (only while Drums mode is on).
5. Turn Drums mode off to bring Options+ back.

## Layout

```
apps/MusicDrums/          Swift menu bar + Process Tap
crates/music_drums_core/  Rust DSP, mapper, HID++, CLI
Scripts/logi-mode.sh      Options+ session toggle
Scripts/build.sh          builds build/MusicDrums.app
docs/                     architecture + HID++ notes
```

## Troubleshooting

### `IOHIDDeviceSetReport` / `0xE00002F0`

Wrong HID collection, or a short (`0x10`) report on Bluetooth. On macOS Bluetooth, HID++ is `page=0xFF43 usage=0x0202` and wants long (`0x11`) reports.

```bash
./Scripts/logi-mode.sh disable
cargo run -p music_drums_core --release --bin music-drums-cli -- devices
cargo run -p music_drums_core --release --bin music-drums-cli -- pulse 70 strong
./Scripts/logi-mode.sh enable
```

Look for `HIDPP-BT(FF43/202)` on the MX Master 4 line.

### `hid_open_path` / `0xE00002E2` (not permitted)

Give the terminal (or the app) Input Monitoring under System Settings → Privacy & Security.

### Mouse freezes in Drums mode

Exclusive HID open can seize the Bluetooth mouse. This project opens non-exclusively (`macos-shared-device`). Rebuild with `./Scripts/build.sh` if you are on an older binary.

## Docs

- [docs/architecture.md](docs/architecture.md)
- [docs/hidpp-mx4.md](docs/hidpp-mx4.md)

References: [MasterMice](https://github.com/olafnew/MasterMice), [logiops #520](https://github.com/PixlOne/logiops/issues/520), [Logitech cpg-docs](https://github.com/Logitech/cpg-docs).

## License

MIT
