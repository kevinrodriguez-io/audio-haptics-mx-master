# MX Master 4 HID++ haptics (`0x19B0`)

Notes from community reverse engineering. Not affiliated with Logitech.

## Sources

- [MasterMice](https://github.com/olafnew/MasterMice): MX4 haptic RE on Windows (intensity, pulse bitmasks, Bolt SET_REPORT)
- [logiops #520](https://github.com/PixlOne/logiops/issues/520) / [PR #524](https://github.com/PixlOne/logiops/pull/524): Linux `PlayEffect` + strength
- [Logitech cpg-docs HID++ 2.0](https://github.com/Logitech/cpg-docs/tree/master/hidpp20)
- [OpenLogi HID++ reference](https://openlogi.org/en/hidpp)

## Feature

| Item | Value |
|------|-------|
| Feature ID | `0x19B0` |
| Typical index | `0x0B` (prefer Root `getFeature`; fall back if discovery fails) |
| Function 2 | Enable + intensity `0…100` (params: `0x01`, intensity) |
| Function 4 | Trigger pulse bitmask |

Function byte: `(function_id << 4) | software_id` (e.g. function 4 + swid `0xA` = `0x4A`).

## Pulse vocabulary (MasterMice)

| Name | Bit |
|------|-----|
| Reset | `0x00` |
| Light | `0x02` |
| Tick | `0x04` |
| Strong | `0x08` |

OR bits for compounds (e.g. Strong\|Tick = `0x0C`).

## Transport quirks

- **Bolt device index:** MasterMice uses `0x02` for MX4 haptics (not `0x01`).
- **Bolt:** page `0xFF00` with short `usage=0x0001` (report `0x10`, 7 bytes) and long `usage=0x0002` (report `0x11`, 20 bytes). Haptics go on the short collection via SET_REPORT.
- **macOS Bluetooth:** page `0xFF43` / usage `0x0202`. Usually only long reports (`0x11`, 20 bytes) work. Short `0x10` fails with `IOHIDDeviceSetReport` / `0xE00002F0`. Do not open the generic Desktop Mouse (`page=0x0001`).
- **Options+:** park with `Scripts/logi-mode.sh disable` before testing, or it will fight for the device.
- Diagnose: `music-drums-cli devices` (look for `HIDPP-BT(FF43/202)` or `SHORT(FF00/1)`).

## Why direct HID++

Options+ plugins only expose a fixed set of named waveforms. Direct `0x19B0` gives intensity `0…100` and pulse bitmasks, which is what the drums mapper needs. Tradeoff: Options+ cannot own the mouse at the same time.
