# MX Master 4 HID++ haptics (`0x19B0`)

Research notes distilled from community reverse engineering. Not affiliated with Logitech.

## Sources

- [MasterMice](https://github.com/olafnew/MasterMice) — deepest public MX4 haptic RE (Windows; intensity + pulse bitmasks; Bolt SET_REPORT)
- [logiops #520](https://github.com/PixlOne/logiops/issues/520) / [PR #524](https://github.com/PixlOne/logiops/pull/524) — Linux `PlayEffect` + strength
- [Logitech cpg-docs HID++ 2.0](https://github.com/Logitech/cpg-docs/tree/master/hidpp20)
- [OpenLogi HID++ reference](https://openlogi.org/en/hidpp)

## Feature

| Item | Value |
|------|-------|
| Feature ID | `0x19B0` |
| Typical index | `0x0B` (discover via Root `getFeature`; do not hardcode in production paths without fallback) |
| Function 2 | Enable + intensity `0…100` (params: `0x01`, intensity) |
| Function 3 | (reserved / unused in our MVP) |
| Function 4 | Trigger pulse bitmask |

Report function byte packing: `(function_id << 4) | software_id` → e.g. function 4 + swid `0xA` = `0x4A`.

## Pulse vocabulary (MasterMice)

| Name | Bit |
|------|-----|
| Reset | `0x00` |
| Light | `0x02` |
| Tick | `0x04` |
| Strong | `0x08` |

Compound patterns OR bits together (e.g. Strong\|Tick = `0x0C`).

## Transport quirks

- **Bolt device index:** MasterMice uses **`0x02`** for MX4 haptic commands (not `0x01`).
- **Bolt:** dual collections on page `0xFF00` — short `usage=0x0001` (report `0x10`, 7 bytes) and long `usage=0x0002` (report `0x11`, 20 bytes). Haptics must use the short collection with SET_REPORT.
- **macOS Bluetooth:** HID++ is a single collection on page **`0xFF43` / usage `0x0202`**. It typically only accepts **long** reports (`0x11`, 20 bytes). Short `0x10` reports fail with `IOHIDDeviceSetReport` / `0xE00002F0`. Do **not** open the generic Desktop Mouse (`page=0x0001`).
- **Options+:** must not hold the device — park with `Scripts/logi-mode.sh disable` before testing.
- Diagnose: `music-drums-cli devices` (look for `HIDPP-BT(FF43/202)` or `SHORT(FF00/1)`).

## Official vs RE

Logi Actions SDK / HapticWeb expose **15 named waveforms** through Options+. Direct `0x19B0` gives intensity control + pulse bitmasks and conflicts with Options+. Music Drums uses the RE path for the drums MVP.
