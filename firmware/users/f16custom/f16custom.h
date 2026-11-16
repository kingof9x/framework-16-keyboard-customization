// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

#include QMK_KEYBOARD_H
#include "hostfx_protocol.h"

// Implemented by each keymap.
bool    process_record_keymap(uint16_t keycode, keyrecord_t *record);
uint8_t hostfx_device_kind(void);
// Macropad layer support (keyboard keymap leaves these as the weak defaults).
uint8_t hostfx_get_layer(void);           // 0xFF = not supported
bool    hostfx_set_layer(uint8_t layer);  // false = not supported

// Hotkey lock: when on, all lighting keycodes (RGB_* and BL_*) do nothing.
bool hostfx_hotkey_lock_get(void);
void hostfx_hotkey_lock_set(bool on);

// Host-driven effect state, used by rgb_matrix_user.inc
extern uint8_t hostfx_frame[RGB_MATRIX_LED_COUNT][3];
bool hostfx_host_mode_active(void);
void hostfx_host_mode_set(bool on);
