# Music Drums architecture

System audio onsets become MX Master 4 haptic pulses via HID++ `0x19B0`.

```
System audio → Swift Process Tap → md_push_audio_frames
                                         ↓
                              Rust onset DSP + mapper
                                         ↓
                         HID++ transport (Bolt | Bluetooth)
                                         ↓
                                   MX Master 4 LRA
```

Drums mode runs `Scripts/logi-mode.sh` to park Options+ (and Logi Plugin Service) so we can open the HID interfaces. Exiting drums mode restores the LaunchAgent.

## Components

| Piece | Role |
|-------|------|
| `apps/MusicDrums` | Menu bar, Process Tap, Options+ toggle, TCC |
| `crates/music_drums_core` | DSP, mapper, HID++, C ABI |
| `Scripts/logi-mode.sh` | Session disable/enable Options+ |
| `Scripts/build.sh` | arm64 Rust staticlib + swiftc `.app` |

## Why Process Tap lives in Swift

`CATapDescription` is Objective-C. Keeping the tap in the signed app gives one TCC identity for Audio Capture. PCM is pushed into Rust for analysis.

## Transports

`open_best_transport()` prefers Bolt, then Bluetooth:

- **Bolt:** short (`0x0001`) + long (`0x0002`) on `0xFF00`; haptics use short + SET_REPORT; device index often `0x02`.
- **Bluetooth (macOS):** `0xFF43` / `0x0202`; haptic config/trigger as long (`0x11`) reports; device index usually `0xFF`.

Non-exclusive open on macOS (`macos-shared-device`) so the cursor still works. Engine reconnects with backoff if set/trigger fails.

## C ABI

See `crates/music_drums_core/include/music_drums.h`:

- `md_start` / `md_stop`
- `md_push_audio_frames`
- `md_set_sensitivity`
- `md_status_json` / `md_string_free`
- `md_test_pulse`

## Permissions / signing

- macOS 14.4+
- Ad-hoc or Developer ID signing (unsigned Process Taps often deliver silence)
- Audio Capture on first start
- Input Monitoring if HID open is denied
