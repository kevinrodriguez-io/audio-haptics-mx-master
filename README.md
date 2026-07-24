# Music Drums

Menu bar app for macOS that taps system audio and buzzes an MX Master 4 on the beat.

Swift UI, Rust core, Apple Silicon. Talks to the mouse over HID++ (`0x19B0`), based on the MasterMice / logiops reverse engineering work.

> **Use at your own risk.** This is experimental software that parks Logi Options+, opens raw HID++ to your mouse, and taps system audio. It is not affiliated with Logitech. It may break mouse bindings, freeze input, drain battery, conflict with other tools, or behave badly after OS/firmware updates. You are responsible for anything that happens to your hardware, software, or data. No warranty — if it bricks your vibe (or worse), that is on you.

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
4. **Configure…** opens presets and detection knobs (Classic is the default).
5. Test pulse fires a strong hit (only while Drums mode is on).
6. Turn Drums mode off to bring Options+ back.

### Presets

Built-in:

- **Classic** — original multi-band detector (default; matches pre-house tuning)
- **House / Electronic** — kick-focused gate, drops hats, lower-latency drains

Shareable JSON lives in `presets/` (examples) and at runtime in  
`~/Library/Application Support/MusicDrums/presets/`. Active config is `active.json`.

```bash
cargo run -p music_drums_core --release --bin music-drums-cli -- presets
cargo run -p music_drums_core --release --bin music-drums-cli -- preset use classic
cargo run -p music_drums_core --release --bin music-drums-cli -- preset use house
cargo run -p music_drums_core --release --bin music-drums-cli -- preset export ~/Desktop/my-drums.json
cargo run -p music_drums_core --release --bin music-drums-cli -- preset import ~/Desktop/my-drums.json
```

FFI: `md_config_json`, `md_set_config_json`, `md_list_presets_json`, `md_load_preset`, `md_save_preset`, `md_export_config`, `md_import_config`.

## Layout

```
apps/MusicDrums/          Swift menu bar + Process Tap
crates/music_drums_core/  Rust DSP, mapper, HID++, CLI
Scripts/logi-mode.sh      Options+ session toggle
Scripts/build.sh          builds build/MusicDrums.app
docs/                     architecture + HID++ notes
```

## Troubleshooting

### CLI works but the app shows `0xE00002E2` (not permitted)

macOS TCC treats **Terminal** and **MusicDrums.app** as separate clients. A working CLI pulse does not mean the menu bar app can open HID.

`./Scripts/build.sh` signs with your **Apple Development** identity when available so Input Monitoring survives rebuilds. Ad-hoc signing (`codesign -s -`) changes the app hash every build and looks like a **new** app to TCC — old Input Monitoring entries stop matching.

Clean permission pass:

1. Quit MusicDrums completely (menu → Quit)
2. Rebuild and reopen:
   ```bash
   ./Scripts/build.sh && open build/MusicDrums.app
   ```
3. System Settings → Privacy & Security → **Input Monitoring**
4. If MusicDrums is already listed but still failing: remove it (−), quit the app, reopen it, add `build/MusicDrums.app` again (+) and enable
5. Toggle Drums mode (allow the system prompt if it appears)

Confirm the binary is Developer-signed (not ad-hoc):

```bash
codesign -dv --verbose=2 build/MusicDrums.app 2>&1 | grep -E 'Authority|Signature='
```

You want `Authority=Apple Development: …`, not `Signature=adhoc`.

### Drums mode toggles on then fails / Options+ keeps coming back

On start failure the app **leaves Options+ parked** (same as `./Scripts/logi-mode.sh disable`) so the next toggle is not fighting Options+ for HID. Turn Drums **off** when you want Options+ restored.

If you previously disabled Options+ from the CLI and the app worked once, that was the parked state helping — not a different HID path.

### `IOHIDDeviceSetReport` / `0xE00002F0`

Wrong HID collection, or a short (`0x10`) report on Bluetooth. On macOS Bluetooth, HID++ is `page=0xFF43 usage=0x0202` and wants long (`0x11`) reports.

```bash
./Scripts/logi-mode.sh disable
cargo run -p music_drums_core --release --bin music-drums-cli -- devices
cargo run -p music_drums_core --release --bin music-drums-cli -- pulse 70 strong
./Scripts/logi-mode.sh enable
```

Look for `HIDPP-BT(FF43/202)` on the MX Master 4 line.

### Mouse freezes in Drums mode

Exclusive HID open can seize the Bluetooth mouse. This project opens non-exclusively (`macos-shared-device`). Rebuild with `./Scripts/build.sh` if you are on an older binary.

## Docs

- [docs/architecture.md](docs/architecture.md)
- [docs/hidpp-mx4.md](docs/hidpp-mx4.md)

References: [MasterMice](https://github.com/olafnew/MasterMice), [logiops #520](https://github.com/PixlOne/logiops/issues/520), [Logitech cpg-docs](https://github.com/Logitech/cpg-docs).

## License

MIT
