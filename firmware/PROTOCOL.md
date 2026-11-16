# Raw HID protocol (f16custom firmware)

The PC app talks to the keyboard and macropad over the same raw HID interface
VIA uses (usage page `0xFF60`, usage `0x61`, 32-byte reports). Our commands sit
inside VIA's "custom value" commands on **channel 0**, which VIA reserves for
keyboard-specific features, so VIA and Framework's `qmk_hid` tool keep working.

| Device   | USB VID | USB PID |
|----------|---------|---------|
| Keyboard (ANSI, RGB) | `0x32AC` | `0x0012` |
| RGB Macropad         | `0x32AC` | `0x0013` |

## Message format

```
byte 0   0x07 = set, 0x08 = get
byte 1   0x00 (channel)
byte 2   value id (table below)
byte 3+  payload
```

The device answers every message with a 32-byte report. Bytes 0–2 are echoed;
if byte 0 is `0xFF`, the command was not understood (for example on stock
firmware).

| Id | Name | get payload (answer) | set payload |
|----|------|----------------------|-------------|
| `01` | INFO | `'F' 'X' version led_count effect_count host_effect_id kind(0=kb,1=pad) hotkey_lock layer(0xFF=n/a) max_brightness` | – |
| `02` | STATE | `enabled mode hue sat val speed streaming` | `enabled mode hue sat val speed save` (`save=1` writes EEPROM) |
| `03` | HOTKEY_LOCK | `on` | `on` (always saved to EEPROM) |
| `04` | LED_INFO | request `start`; answer `start count (x y flags)×count` (max 9) | – |
| `05` | FRAME | – | `start count (r g b)×count` (max 8 LEDs per message) |
| `06` | HOST_MODE | `on` | `1` = show PC frames (not saved), `0` = back to the saved effect |
| `07` | KEY_EVENTS | `count led_index×count` (presses since the last poll, max 28) | – |
| `08` | LAYER | `layer` | `0` = MIDI, `1` = F-keys (macropad only, saved) |

Notes:

* The PC-driven effect is a normal QMK effect (`RGB_MATRIX_CUSTOM_HOST_DRIVEN`,
  id 45 with Framework's v0.3.1 effect list). Its colours are scaled by the
  device's brightness, so Fn brightness keys still work.
* If no FRAME arrives for 3 seconds while host mode is on, the device returns to
  the effect saved in EEPROM by itself. A crashed app never leaves the lights
  frozen.
* Messages to the device only use the first 29 bytes. Framework's VIA code
  notes the RP2040 sometimes receives only 31 of the 32 bytes.
* LED positions use QMK's grid: x 0–224, y 0–64 on each device.
* Restarting into the bootloader uses Framework's own factory command
  (`0x0B 0xFE`), which their stock firmware also supports.
* The definitions live in `users/f16custom/hostfx_protocol.h` (firmware) and
  `app/core/src/protocol.rs` (app). Keep both in sync.
