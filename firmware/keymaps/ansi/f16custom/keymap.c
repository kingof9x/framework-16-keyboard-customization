// Copyright 2022 Framework Computer, 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Framework Laptop 16 ANSI keyboard (RGB) - "f16custom" keymap.
//
// The key layout is identical to Framework's default keymap. What is added
// (in users/f16custom) is:
//   * a host-driven RGB effect the PC app can stream colors to
//   * raw HID commands to read/set effect, speed, brightness and color
//   * a hotkey lock (set from the PC app, saved in EEPROM) that turns the
//     Fn lighting keys (RGB / backlight) into no-ops

#include QMK_KEYBOARD_H
#include "framework.h"
#include "f16custom.h"

enum _layers {
  _BASE,
  _FN,
  _FN_LOCK,
  _FM
};

const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
     /*
     *         ┌─────┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬────┐
     * 14 keys │ESC  │Mut│vDn│vUp│Prv│Ply│Nxt│bDn│bUp│Scn│Air│Prt│App│Del │
     *         ├───┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴────┤
     * 14 keys │ ` │ 1 │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │ 0 │ - │ = │Backsp│
     *         ├───┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬────┤
     * 14 keys │ Tab │ Q │ W │ E │ R │ T │ Y │ U │ I │ O │ P │ [ │ ] │ \  │
     *         ├─────┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴────┤
     * 13 keys │ Caps │ A │ S │ D │ F │ G │ H │ J │ K │ L │ ; │ ' │ Enter │
     *         ├──────┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴───────┤
     * 12 keys │ Shift  │ Z │ X │ C │ V │ B │ N │ M │ , │ . │ / │  Shift  │
     *         ├────┬───┼───┼───┼───┴───┴───┴───┴───┼───┼───┼───┴┬───┬────┤
     *         │    │   │   │   │                   │   │   │    │↑  │    │
     * 11 keys │Ctrl│FN │GUI│Alt│                   │Alt│Ctl│ ←  ├───┤  → │
     *         │    │   │   │   │                   │   │   │    │  ↓│    │
     *         └────┴───┴───┴───┴───────────────────┴───┴───┴────┴───┴────┘
     * 78 total
     */
    [_BASE] = LAYOUT(
        KC_ESC,  KC_MUTE, KC_VOLD, KC_VOLU, KC_MPRV, KC_MPLY, KC_MNXT, KC_BRID, KC_BRIU, KC_SCRN, KC_AIRP, KC_PSCR, KC_MSEL, KC_DEL,
        KC_GRV,  KC_1,    KC_2,    KC_3,    KC_4,    KC_5,    KC_6,    KC_7,    KC_8,    KC_9,    KC_0,    KC_MINS, KC_EQL,  KC_BSPC,
        KC_TAB,  KC_Q,    KC_W,    KC_E,    KC_R,    KC_T,    KC_Y,    KC_U,    KC_I,    KC_O,    KC_P,    KC_LBRC, KC_RBRC, KC_BSLS,
        KC_CAPS, KC_A,    KC_S,    KC_D,    KC_F,    KC_G,    KC_H,    KC_J,    KC_K,    KC_L,    KC_SCLN, KC_QUOT,          KC_ENT,
        KC_LSFT,          KC_Z,    KC_X,    KC_C,    KC_V,    KC_B,    KC_N,    KC_M,    KC_COMM, KC_DOT,  KC_SLSH,          KC_RSFT,
        KC_LCTL, MO(_FN), KC_LGUI, KC_LALT,          KC_SPC,                    KC_RALT, KC_RCTL, KC_LEFT,   KC_UP, KC_DOWN, KC_RGHT
    ),
     /*
     * Function layer
     *         ┌─────┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬────┐
     * 14 keys │FN lk│F1 │F2 │F3 │F4 │F5 │F6 │F7 │F8 │F9 │F10│F11│F12│Ins │
     *         ├───┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴────┤
     * 14 keys │   │   │   │   │   │   │   │   │   │   │   │   │   │      │
     *         ├───┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬────┤
     * 14 keys │     │   │RGB│Nxt│Hue│Sat│Spd│Brt│   │   │Pau│   │   │    │
     *         ├─────┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴┬──┴────┤
     * 13 keys │      │   │   │Prv│Hue│Sat│Spd│Brt│ScL│   │   │   │       │
     *         ├──────┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴─┬─┴───────┤
     * 12 keys │        │   │   │   │   │   │Brk│   │   │   │   │         │
     *         ├────┬───┼───┼───┼───┴───┴───┴───┴───┼───┼───┼───┴┬───┬────┤
     *         │    │   │   │   │                   │   │   │    │PgU│    │
     * 11 keys │    │   │   │   │ Toggle Backlight  │   │   │Home├───┤End │
     *         │    │   │   │   │                   │   │   │    │PgD│    │
     *         └────┴───┴───┴───┴───────────────────┴───┴───┴────┴───┴────┘
     * 78 total
     */
    [_FN] = LAYOUT(
        FN_LOCK, KC_F1,   KC_F2,   KC_F3,   KC_F4,   KC_F5,   KC_F6,   KC_F7,   KC_F8,   KC_F9,   KC_F10,  KC_F11,  KC_F12,  KC_INS,
        _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,
        _______, _______, RGB_TOG, RGB_MOD, RGB_HUI, RGB_SAI, RGB_SPI, RGB_VAI, _______, _______, KC_PAUS, _______, _______, _______,
        _______, _______, _______, RGB_RMOD,RGB_HUD, RGB_SAD, RGB_SPD, RGB_VAD, KC_SCRL, _______, _______, _______,          _______,
        _______,          _______, _______, BL_BRTG, _______, KC_BRK,  _______, _______, _______, _______, _______,          _______,
        _______, _______, _______, _______,          BL_STEP,                   _______, _______, KC_HOME, KC_PGUP, KC_PGDN, KC_END
    ),
    // Function lock layer
    // Everything on F-row locked to function layer, except ESC and DEL
    [_FN_LOCK] = LAYOUT(
        _______, KC_F1,   KC_F2,   KC_F3,   KC_F4,   KC_F5,   KC_F6,   KC_F7,   KC_F8,   KC_F9,   KC_F10,  KC_F11,  KC_F12,  _______,
        _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,
        _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,
        _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,          _______,
        _______,          _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,          _______,
        _______, MO(_FM), _______, _______,          _______,                   _______, _______, _______, _______, _______, _______
    ),
    // Locked+temporary FN (back to base plus extra keys)
    [_FM] = LAYOUT(
        FN_LOCK, KC_MUTE, KC_VOLD, KC_VOLU, KC_MPRV, KC_MPLY, KC_MNXT, KC_BRID, KC_BRIU, KC_SCRN, KC_AIRP, KC_PSCR, KC_MSEL,  KC_INS,
        _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______, _______,
        _______, _______, RGB_TOG, RGB_MOD, RGB_HUI, RGB_SAI, RGB_SPI, RGB_VAI, _______, _______, KC_PAUS, _______, _______, _______,
        _______, _______, _______, RGB_RMOD,RGB_HUD, RGB_SAD, RGB_SPD, RGB_VAD, KC_SCRL, _______, _______, _______,          _______,
        _______,          _______, _______, BL_BRTG, _______, KC_BRK,  _______, _______, _______, _______, _______,          _______,
        _______, _______, _______, _______,          BL_STEP,                   _______, _______, KC_HOME, KC_PGUP, KC_PGDN, KC_END
    ),
};

uint8_t hostfx_device_kind(void) {
    return HOSTFX_KIND_KEYBOARD;
}

// ---------------------------------------------------------------------------
// F12 shortcuts: hold F12 and press
//   Right / Left  next / previous lighting effect (not saved; only effects that show
//                 something without typing)
//   Down          back to the saved effect
//   Up            Ctrl+Alt+Shift+M: audio mode on/off in the PC app (and White Rabbit)
// F12 is matched by its position, so it works whatever F12 is set to send. F12's
// own job happens when it is let go without an arrow (so it no longer repeats
// when held).
// ---------------------------------------------------------------------------

#define F12_ROW 3
#define F12_COL 13
#define LEFT_ROW 6
#define LEFT_COL 11
#define UP_ROW 1
#define UP_COL 13
#define DOWN_ROW 1
#define DOWN_COL 8
#define RIGHT_ROW 2
#define RIGHT_COL 15

static bool     f12_held    = false;
static bool     f12_used    = false; // an arrow was pressed while F12 was held
static uint16_t f12_keycode = KC_NO; // what F12 sends, from when it went down

bool hostfx_has_shortcuts(void) {
    return true;
}

static bool at(keyrecord_t *record, uint8_t row, uint8_t col) {
    return record->event.key.row == row && record->event.key.col == col;
}

// Effects the F12 shortcuts step through: the ones that show something on their own.
// Left out: the PC-controlled and painted-keys-only effects (dark without the app or
// painted keys) and the typing effects that stay dark until you type (they can still be
// picked in the app).
static bool shown_on_its_own(uint8_t mode) {
    switch (mode) {
        case RGB_MATRIX_NONE:
        case RGB_MATRIX_CUSTOM_HOST_DRIVEN:
        case RGB_MATRIX_CUSTOM_PAINT_ONLY:
        case RGB_MATRIX_TYPING_HEATMAP:
        case RGB_MATRIX_SOLID_REACTIVE_SIMPLE:
        case RGB_MATRIX_SOLID_REACTIVE_WIDE:
        case RGB_MATRIX_SOLID_REACTIVE_MULTIWIDE:
        case RGB_MATRIX_SOLID_REACTIVE_CROSS:
        case RGB_MATRIX_SOLID_REACTIVE_MULTICROSS:
        case RGB_MATRIX_SOLID_REACTIVE_NEXUS:
        case RGB_MATRIX_SOLID_REACTIVE_MULTINEXUS:
        case RGB_MATRIX_SPLASH:
        case RGB_MATRIX_MULTISPLASH:
        case RGB_MATRIX_SOLID_SPLASH:
        case RGB_MATRIX_SOLID_MULTISPLASH:
            return false;
        default:
            return true;
    }
}

static void step_effect(bool forward) {
    uint8_t mode = rgb_matrix_get_mode();
    for (uint8_t tries = 0; tries < RGB_MATRIX_EFFECT_MAX; tries++) {
        if (forward) {
            mode = (mode + 1 < RGB_MATRIX_EFFECT_MAX) ? mode + 1 : 1;
        } else {
            mode = (mode > 1) ? mode - 1 : RGB_MATRIX_EFFECT_MAX - 1;
        }
        if (shown_on_its_own(mode)) break;
    }
    rgb_matrix_mode_noeeprom(mode);
    // Lights that were turned off (or set to zero brightness) come back on, so the step shows
    if (!rgb_matrix_is_enabled()) rgb_matrix_enable_noeeprom();
    if (rgb_matrix_get_val() == 0) {
        rgb_matrix_sethsv_noeeprom(rgb_matrix_get_hue(), rgb_matrix_get_sat(), 128);
    }
}

bool pre_process_record_keymap(uint16_t keycode, keyrecord_t *record) {
    if (!IS_KEYEVENT(record->event) || !hostfx_shortcuts_get()) return true;
    bool pressed = record->event.pressed;
    if (at(record, F12_ROW, F12_COL)) {
        if (pressed) {
            f12_held    = true;
            f12_used    = false;
            f12_keycode = keycode;
        } else {
            f12_held = false;
            if (!f12_used && f12_keycode != KC_NO && f12_keycode != KC_TRNS) {
                tap_code16(f12_keycode); // F12 on its own: do its normal job
            }
        }
        return false;
    }
    if (!f12_held) return true;
    bool left = at(record, LEFT_ROW, LEFT_COL), right = at(record, RIGHT_ROW, RIGHT_COL);
    bool up = at(record, UP_ROW, UP_COL), down = at(record, DOWN_ROW, DOWN_COL);
    if (!(left || right || up || down)) return true;
    if (pressed) {
        f12_used = true;
        // The lighting shortcuts follow the "Fn lighting keys" switch, and leave the
        // PC alone while it is drawing (audio mode, app effects)
        bool lights_ok = !hostfx_hotkey_lock_get() && !hostfx_frames_fresh();
        if ((left || right) && lights_ok) {
            step_effect(right);
        } else if (down && lights_ok) {
            rgb_matrix_reload_from_eeprom();
            surface_paint_reload();
        } else if (up) {
            tap_code16(LCTL(LALT(LSFT(KC_M))));
        }
    }
    return false; // the arrow itself isn't typed
}

// Make sure to keep FN Lock even after reset
bool process_record_keymap(uint16_t keycode, keyrecord_t *record) {
    switch (keycode) {
        case FN_LOCK:
            if (record->event.pressed) {
                if (layer_state_is(_FN)) {
                    set_single_persistent_default_layer(_FN_LOCK);
                }
                if (layer_state_is(_FM)) {
                    set_single_persistent_default_layer(_BASE);
                }
            }
            return false;
        default:
            break;
    }
    return true;
}
