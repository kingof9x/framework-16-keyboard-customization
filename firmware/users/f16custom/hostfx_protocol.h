// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Raw HID protocol shared by the firmware and the PC app (see firmware/PROTOCOL.md).
//
// Every message is a 32 byte raw HID report on the standard VIA interface
// (usage page 0xFF60, usage 0x61). We piggy-back on VIA's "custom value"
// commands using channel 0 (id_custom_channel), which VIA leaves for
// keyboard-specific use. Everything else VIA does (and Framework's qmk_hid
// tool) keeps working unchanged.
//
//   byte 0: 0x07 (set) or 0x08 (get)      -- VIA id_custom_set_value / get_value
//   byte 1: 0x00                          -- VIA id_custom_channel
//   byte 2: HOSTFX_* value id below
//   byte 3..31: payload
//
// The device answers every message with a 32 byte report that echoes bytes
// 0..2 and fills in the payload. If byte 0 of the answer is 0xFF the command
// was not understood (e.g. stock firmware).
#pragma once

#define HOSTFX_MAGIC_0 'F'
#define HOSTFX_MAGIC_1 'X'
#define HOSTFX_PROTOCOL_VERSION 2

enum hostfx_value_id {
    HOSTFX_INFO        = 0x01, // get: magic, version, LED count, effect count, host effect id, device kind, lock, layer, max brightness, paint-only effect id
    HOSTFX_STATE       = 0x02, // get/set: enabled, mode, hue, sat, val, speed (set: + save flag)
    HOSTFX_HOTKEY_LOCK = 0x03, // get/set: 1 = Fn lighting keys disabled (saved in EEPROM)
    HOSTFX_LED_INFO    = 0x04, // get: [start] -> [count, (x, y, flags) * count]
    HOSTFX_FRAME       = 0x05, // set: [start, count, (r, g, b) * count]  (max 8 LEDs per message)
    HOSTFX_HOST_MODE   = 0x06, // get/set: 1 = switch to host-driven effect, 0 = back to saved effect
    HOSTFX_KEY_EVENTS  = 0x07, // get: [count, led_index * count] key presses since last poll (max 28)
    HOSTFX_LAYER       = 0x08, // get/set: macropad layer (0 = MIDI, 1 = F-keys); keyboard answers 0xFF
    HOSTFX_SPAN        = 0x09, // get/set: [on, x_start, x_end] joined surface: this device's slice of 0..224 (saved)
    HOSTFX_CLOCK       = 0x0A, // set: [ms (u32, little endian)] line up the effect clock of joined devices
    HOSTFX_PAINT       = 0x0B, // get: [start] -> [count, (hue, sat, on) * count] (max 9)
                               // set: [start, count, (hue, sat, on) * count] (max 8)
    HOSTFX_PAINT_CTRL  = 0x0C, // get: [follow, painted count]; set: [op] see hostfx_paint_op
    HOSTFX_COMBOS      = 0x0D, // get/set: [top on, bottom on, top action (u16 BE), bottom action (u16 BE), corner layer lights] (saved)
                               // macropad corner combos; action 0 = built-in job. Keyboard: unhandled
    HOSTFX_LED_CAL     = 0x0E, // get: [current, red, green, blue, default current]
                               // set: [current, red, green, blue, save]: LED driver current (0 = default)
                               // and per-color gains (255 = full), to match the two devices' brightness
};

enum hostfx_paint_op {
    HOSTFX_PAINT_CLEAR      = 0, // remove every painted key
    HOSTFX_PAINT_SAVE       = 1, // store painted keys + follow flag in EEPROM
    HOSTFX_PAINT_FOLLOW_OFF = 2, // painted keys keep a steady color
    HOSTFX_PAINT_FOLLOW_ON  = 3, // painted keys brighten and dim with the effect
    HOSTFX_PAINT_RELOAD     = 4, // forget unsaved changes
    HOSTFX_PAINT_TINT       = 5, // painted keys take the effect's color, shifted by their hue
};

// Painted-key modes (the "follow" byte of PAINT_CTRL and the paint store)
enum hostfx_paint_mode {
    HOSTFX_PAINT_MODE_STEADY = 0,
    HOSTFX_PAINT_MODE_FOLLOW = 1, // brightness follows the effect
    HOSTFX_PAINT_MODE_TINT   = 2, // color and brightness follow the effect, hue shifted
};

enum hostfx_device_kind {
    HOSTFX_KIND_KEYBOARD = 0,
    HOSTFX_KIND_MACROPAD = 1,
};

// Host->device messages only use the first 31 bytes: Framework notes the RP2040
// sometimes receives only 31 of the 32 bytes. 3 header + 2 + 8 * 3 = 29.
#define HOSTFX_LEDS_PER_FRAME_MSG 8
#define HOSTFX_LEDS_PER_INFO_MSG 9
#define HOSTFX_KEYS_PER_MSG 28
#define HOSTFX_PAINT_PER_SET_MSG 8
#define HOSTFX_PAINT_PER_GET_MSG 9
// Painted-key storage (EEPROM user datablock): version, follow, 128 x (hue, sat, on)
#define HOSTFX_PAINT_MAX_LEDS 128
