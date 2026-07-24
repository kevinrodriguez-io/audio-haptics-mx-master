# Music Drums

macOS menu-bar app that listens to **system audio** and pulses an **MX Master 4** in time with onsets (drums MVP).

Built as **Swift UI + Rust core** (Apple Silicon). Haptics go through **HID++ feature `0x19B0`** (MasterMice-style), not HapticWeb.

> **Important:** While Drums mode is on, **Logi Options+ is parked** so this app can own the mouse HID interfaces. Your Options+ bindings / Actions Ring will not work until you turn Drums mode off (or run `Scripts/logi-mode.sh enable`).

## Requirements

- macOS 14.4+
- Apple Silicon (arm64)
- MX Master 4 on **Logi Bolt** and/or **Bluetooth**
- Rust toolchain (`rustup`) + Xcode CLT / Swift 6
- Quit or allow the app to park Logi Options+ during drums

## Quick start

```bash
# Park Options+ for a CLI pulse test
./Scripts/logi-mode.sh disable
cargo run -p music_drums_core --release --bin music-drums-cli -- pulse 70 strong
./Scripts/logi-mode.sh enable

# Build the menu-bar app
./Scripts/build.sh
open build/MusicDrums.app
```

On first run, grant **Audio Capture** when prompted.

## Usage

1. Open **Music Drums** (menu bar metronome / music note).
2. Enable **Drums mode** — Options+ is disabled for the session, engine starts, Process Tap begins.
3. Play music from any app; adjust **Sensitivity**.
4. **Test pulse** fires a strong haptic (Options+ must already be parked).
5. Disable Drums mode to restore Options+.

## Layout

```
apps/MusicDrums/          Swift menu bar + Process Tap
crates/music_drums_core/  Rust DSP, mapper, HID++, C ABI, CLI
Scripts/logi-mode.sh      Options+ session toggle
Scripts/build.sh          arm64 build → build/MusicDrums.app
docs/                     Architecture + HID++ notes
```

## Troubleshooting

### `IOHIDDeviceSetReport` / `0xE00002F0` (data was not found)

Usually either the wrong HID collection was opened, or a **short** (`0x10`) report was sent on Bluetooth FF43 (which wants **long** `0x11` reports). On Bluetooth macOS, HID++ is **`page=0xFF43 usage=0x0202`**. Run:

```bash
./Scripts/logi-mode.sh disable
cargo run -p music_drums_core --release --bin music-drums-cli -- devices
cargo run -p music_drums_core --release --bin music-drums-cli -- pulse 70 strong
./Scripts/logi-mode.sh enable
```

You should see a line tagged `HIDPP-BT(FF43/202)` for the MX Master 4.

### `hid_open_path` / `0xE00002E2` (not permitted)

Grant **Input Monitoring** to your terminal app (Terminal / iTerm / Cursor) under  
System Settings → Privacy & Security → Input Monitoring, then retry. The menu-bar `.app` needs the same if the CLI works but the app cannot open HID.

### Mouse freezes when Drums mode is on

hidapi defaults to **exclusive** open on macOS, which can seize the whole Bluetooth mouse. Music Drums forces non-exclusive / `macos-shared-device` so the cursor keeps working while HID++ stays open. Rebuild with `./Scripts/build.sh` after pulling that fix.

## License

MIT
