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
// Macropad corner combos (HOSTFX_COMBOS payload); false = not supported
bool hostfx_combos_get(uint8_t *d);
bool hostfx_combos_set(const uint8_t *d);

// Keymap hook run before anything else; return false to swallow the key.
bool pre_process_record_keymap(uint16_t keycode, keyrecord_t *record);
// The keyboard's F12 shortcuts (F12 + arrows). Weak default: not supported.
bool hostfx_has_shortcuts(void);
bool hostfx_shortcuts_get(void);
void hostfx_shortcuts_set(bool on);

// Hotkey lock: when on, all lighting keycodes (RGB_* and BL_*) do nothing.
bool hostfx_hotkey_lock_get(void);
void hostfx_hotkey_lock_set(bool on);

// Host-driven effect state, used by rgb_matrix_user.inc
extern uint8_t hostfx_frame[RGB_MATRIX_LED_COUNT][3];
bool hostfx_host_mode_active(void);
void hostfx_host_mode_set(bool on);
bool hostfx_frames_fresh(void); // the PC is streaming frames right now

// surface.c: joined surface, effect clock, painted keys
void surface_apply_span(bool on, uint8_t x0, uint8_t x1);
void surface_span_get(uint8_t *d);
uint8_t surface_led_x(uint8_t i);  // x before squeezing for the joined surface
void surface_set_clock(uint32_t ms);
void surface_housekeeping(void);
void     surface_paint_reload(void);
void     surface_store_reset(void);
uint32_t surface_config_read(void);
void     surface_config_write(uint32_t config);
void surface_paint_get(uint8_t *d);
bool surface_paint_set(const uint8_t *d);
void surface_paint_ctrl_get(uint8_t *d);
// Small saved settings kept in the unused end of the paint store (devices with
// fewer LEDs than HOSTFX_PAINT_MAX_LEDS only). slot < SURFACE_EXTRA_SLOTS.
// Slots 0-2: macropad combos (keymap.c). Slots 3-4: LED calibration (surface.c).
#define SURFACE_EXTRA_SLOTS 5
void surface_extra_get(uint8_t slot, uint8_t out[3]);
void surface_extra_set(uint8_t slot, const uint8_t v[3]);
bool surface_paint_ctrl(uint8_t op);
void surface_led_cal_get(uint8_t *d);
bool surface_led_cal_set(const uint8_t *d);
