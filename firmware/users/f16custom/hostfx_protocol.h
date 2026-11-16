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
#define HOSTFX_PROTOCOL_VERSION 1

enum hostfx_value_id {
    HOSTFX_INFO        = 0x01, // get: magic, version, LED count, effect count, host effect id, device kind, lock, layer
    HOSTFX_STATE       = 0x02, // get/set: enabled, mode, hue, sat, val, speed (set: + save flag)
    HOSTFX_HOTKEY_LOCK = 0x03, // get/set: 1 = Fn lighting keys disabled (saved in EEPROM)
    HOSTFX_LED_INFO    = 0x04, // get: [start] -> [count, (x, y, flags) * count]
    HOSTFX_FRAME       = 0x05, // set: [start, count, (r, g, b) * count]  (max 8 LEDs per message)
    HOSTFX_HOST_MODE   = 0x06, // get/set: 1 = switch to host-driven effect, 0 = back to saved effect
    HOSTFX_KEY_EVENTS  = 0x07, // get: [count, led_index * count] key presses since last poll (max 28)
    HOSTFX_LAYER       = 0x08, // get/set: macropad layer (0 = MIDI, 1 = F-keys); keyboard answers 0xFF
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
