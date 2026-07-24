# Music Drums — Architecture

## Overview

Music Drums is a macOS menu-bar app that turns system audio onsets into MX Master 4 haptic pulses via HID++ feature `0x19B0`.

```
System audio → Swift Process Tap → md_push_audio_frames
                                         ↓
                              Rust onset DSP + mapper
                                         ↓
                         HID++ transport (Bolt | Bluetooth)
                                         ↓
                                   MX Master 4 LRA
```

While **Drums mode** is on, `Scripts/logi-mode.sh` parks Logi Options+ (and Logi Plugin Service) so the app can own the HID interfaces. Exiting drums mode restores the LaunchAgent.

## Components

| Piece | Role |
|-------|------|
| `apps/MusicDrums` | SwiftUI menu bar, Process Tap, Options+ toggle, TCC |
| `crates/music_drums_core` | DSP, mapper, HID++, C ABI |
| `Scripts/logi-mode.sh` | Session disable/enable Options+ |
| `Scripts/build.sh` | arm64 Rust staticlib + swiftc `.app` |

## Why Process Tap is in Swift

`CATapDescription` is an Objective-C Core Audio type. Hosting the tap in the signed Swift app keeps a single TCC identity for **Audio Capture** (`NSAudioCaptureUsageDescription`). PCM is pushed into Rust for realtime analysis.

## Transports

`open_best_transport()` prefers **Logi Bolt**, then **Bluetooth**:

- Bolt: short (usage `0x0001`) + long (usage `0x0002`) vendor collections; haptic commands use `send_output_report` (SET_REPORT) per MasterMice findings.
- Bluetooth: MX Master 4 product / name match; same `0x19B0` payloads; device index `0xFF` then `0x01`.

Reconnect: engine retries transport open with backoff if set/trigger fails.

## C ABI

See `crates/music_drums_core/include/music_drums.h`:

- `md_start` / `md_stop`
- `md_push_audio_frames`
- `md_set_sensitivity`
- `md_status_json` / `md_string_free`
- `md_test_pulse`

## Permissions / signing

- macOS 14.4+
- Ad-hoc or Developer ID signature (unsigned Process Taps often deliver silence)
- User grants Audio Capture on first start
