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
| `01` | INFO | `'F' 'X' version(2) led_count effect_count host_effect_id kind(0=kb,1=pad) hotkey_lock layer(0xFF=n/a) max_brightness paint_only_effect_id` | – |
| `02` | STATE | `enabled mode hue sat val speed streaming` | `enabled mode hue sat val speed save` (`save=1` writes EEPROM) |
| `03` | HOTKEY_LOCK | `on` | `on` (always saved to EEPROM) |
| `04` | LED_INFO | request `start`; answer `start count (x y flags)×count` (max 9). Positions are the real ones, also while joined | – |
| `05` | FRAME | – | `start count (r g b)×count` (max 8 LEDs per message) |
| `06` | HOST_MODE | `on` | `1` = show PC frames (not saved), `0` = back to the saved effect |
| `07` | KEY_EVENTS | `count led_index×count` (presses since the last poll, max 28) | – |
| `08` | LAYER | `layer` | `0` = MIDI, `1` = F-keys (macropad only, saved) |
| `09` | SPAN | `on x_start x_end` | `on x_start x_end`: joined surface, this device's slice of 0–224 (saved) |
| `0A` | CLOCK | – | `ms` (u32, little endian): sets the effect clock; sent to both joined devices back to back |
| `0B` | PAINT | request `start`; answer `start count (hue sat on)×count` (max 9) | `start count (hue sat on)×count` (max 8) |
| `0C` | PAINT_CTRL | `follow count` | `op`: 0 clear, 1 save to EEPROM, 2 follow off, 3 follow on, 4 forget unsaved changes |
| `0D` | COMBOS | `top_on bottom_on top_action(u16 BE) bottom_action(u16 BE) corner_lights` | same (saved). Macropad only; the keyboard answers `FF` |

Notes:

* The PC-driven effect is a normal QMK effect (`RGB_MATRIX_CUSTOM_HOST_DRIVEN`,
  id 45 with Framework's v0.3.1 effect list). Its colors are scaled by the
  device's brightness, so Fn brightness keys still work.
* **Joined surface:** QMK effects place LEDs on a 0–224 × 0–64 grid. With SPAN
  on, the device squeezes its LEDs' x positions into `x_start..x_end`, so the
  same built-in effect on both devices flows across them as one surface. The
  app lines up the effect clocks with CLOCK every 10 s while it runs. The
  clock offset is added only while a frame is drawn, so QMK's own timing is
  unchanged.
* **Painted keys** are applied after the effect has drawn each frame. With
  "follow" on, a painted LED takes the brightness the effect gave it (read
  back from the LED driver's frame buffer), with a floor of 30%. Painted keys,
  the follow flag, the hotkey lock and the span are stored in the EEPROM user
  datablock (390 bytes).
* **Macropad combos:** the two corner combos match on key *position* (QMK's
  `COMBO_ONLY_FROM_LAYER`, using Framework's factory layer), so they keep
  working after the corner keys are changed. An action of 0 is the built-in job
  (top: switch layer A/B, bottom: lighting overlay); any other value is a key
  code that is tapped. `corner_lights` makes the top corners keep the layer
  color (cyan A, orange B). These live in spare bytes at the end of the paint
  store, which the macropad's 24 LEDs leave unused.
* **Keys:** the app changes keys with VIA's own dynamic keymap commands
  (`0x04` get keycode, `0x05` set keycode, `0x12` read the keymap buffer), so
  changes are saved on the device and VIA sees them too. On the keyboard the
  app keeps the Fn lock layers in step: a top-row change for "normal" also goes
  to layer 3, and one for "with Fn" to layer 2. "Open an app" keys send
  Ctrl+Shift+Alt+F13 to F24, which the Windows app catches.
* **Macropad brightness:** Framework sets the macropad's LED driver to a higher
  global current than the keyboard's (185 vs 104), so it looked brighter at the
  same setting. The f16custom macropad firmware sets 104 to match.
* Effect ids with Framework's v0.3.1 list: 1–44 built-in, 45 PC controlled
  (host driven), 46 off with painted keys only.
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
