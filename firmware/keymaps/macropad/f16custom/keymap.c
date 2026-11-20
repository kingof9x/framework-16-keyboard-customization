// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Framework Laptop 16 RGB macropad - "f16custom" keymap for VJ use (Resolume).
//
// Layer A (MIDI):   24 MIDI notes, MIDI note 48 (top left) to 71 (bottom right)
// Layer B (numpad): Framework's numpad layout
//
// Press the two TOP corner keys together  -> switch between layer A and B
//                                            (remembered after unplugging)
// Press the two BOTTOM corner keys together -> lighting controls on/off
// Each combo can be turned off or given another job from the PC app.
//
// The whole pad flashes the active layer's color on a switch (A = cyan,
// B = orange). The two top corner keys can also keep that color (app setting).
// See resolume/MAPPING.md for the key-by-key sheet.

#include QMK_KEYBOARD_H
#include "framework.h"
#include "f16custom.h"

// _FACTORY (= 2, used by Framework's factory test tool) comes from macropad.h
enum pad_layers {
    _MIDI  = 0, // layer A
    _FKEYS = 1, // layer B
    _LIGHT = 3, // lighting controls overlay
};
_Static_assert(_FACTORY == 2, "layer numbering changed in macropad.h");

enum pad_keycodes {
    PAD_SWAP = KC_CPLT + 1, // switch layer A <-> B
    PAD_LIGHT,              // toggle the lighting overlay
};


// clang-format off
const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {
    /* Layer A - MIDI notes, channel 1, velocity 127. Numbers are MIDI note
     * numbers (apps name them differently: 48 is "C2" in some, "C3" in others)
     * ┌──────┬──────┬──────┬──────┐
     * │ 48 C │ 49 C#│ 50 D │ 51 D#│  <- top corners = layer switch combo
     * ├──────┼──────┼──────┼──────┤
     * │ 52 E │ 53 F │ 54 F#│ 55 G │
     * ├──────┼──────┼──────┼──────┤
     * │ 56 G#│ 57 A │ 58 A#│ 59 B │
     * ├──────┼──────┼──────┼──────┤
     * │ 60 C │ 61 C#│ 62 D │ 63 D#│
     * ├──────┼──────┼──────┼──────┤
     * │ 64 E │ 65 F │ 66 F#│ 67 G │
     * ├──────┼──────┼──────┼──────┤
     * │ 68 G#│ 69 A │ 70 A#│ 71 B │  <- bottom corners = lighting combo
     * └──────┴──────┴──────┴──────┘
     */
    [_MIDI] = LAYOUT(
        MI_C,   MI_Cs,  MI_D,   MI_Ds,
        MI_E,   MI_F,   MI_Fs,  MI_G,
        MI_Gs,  MI_A,   MI_As,  MI_B,
        MI_C1,  MI_Cs1, MI_D1,  MI_Ds1,
        MI_E1,  MI_F1,  MI_Fs1, MI_G1,
        MI_Gs1, MI_A1,  MI_As1, MI_B1
    ),
    /* Layer B - Framework's numpad layout (change any key in the app's Key mapping page)
     * The numpad's big keys (+, Enter, 0) take two of the macropad's squares.
     * ┌──────┬──────┬──────┬──────┐
     * │ Esc  │ Calc │  =   │ Bksp │
     * ├──────┼──────┼──────┼──────┤
     * │ Num  │  /   │  *   │  -   │
     * ├──────┼──────┼──────┼──────┤
     * │  7   │  8   │  9   │  +   │
     * ├──────┼──────┼──────┼──────┤
     * │  4   │  5   │  6   │  +   │
     * ├──────┼──────┼──────┼──────┤
     * │  1   │  2   │  3   │Enter │
     * ├──────┼──────┼──────┼──────┤
     * │  0   │  0   │  .   │Enter │
     * └──────┴──────┴──────┴──────┘
     */
    [_FKEYS] = LAYOUT(
        KC_ESC,  KC_CALC, KC_EQL,  KC_BSPC,
        KC_NUM,  KC_PSLS, KC_PAST, KC_PMNS,
        KC_P7,   KC_P8,   KC_P9,   KC_PPLS,
        KC_P4,   KC_P5,   KC_P6,   KC_PPLS,
        KC_P1,   KC_P2,   KC_P3,   KC_PENT,
        KC_P0,   KC_P0,   KC_PDOT, KC_PENT
    ),
    // Framework factory test layer (only switched on by their test tool)
    [_FACTORY] = LAYOUT(
        KC_A,    KC_B,    KC_C,    KC_D,
        KC_E,    KC_F,    KC_G,    KC_H,
        KC_I,    KC_J,    KC_K,    KC_L,
        KC_M,    KC_N,    KC_O,    KC_P,
        KC_Q,    KC_R,    KC_S,    KC_T,
        KC_U,    KC_V,    KC_W,    KC_X
    ),
    /* Lighting overlay (bottom corners combo toggles it)
     * ┌──────┬──────┬──────┬──────┐
     * │ (A/B)│On/Off│Effect│ (A/B)│
     * │      │      │ next │      │
     * ├──────┼──────┼──────┼──────┤
     * │ Hue+ │ Sat+ │Brigh+│Speed+│
     * ├──────┼──────┼──────┼──────┤
     * │ Hue- │ Sat- │Brigh-│Speed-│
     * ├──────┼──────┼──────┼──────┤
     * │Effect│      │      │      │
     * │ prev │      │      │      │
     * ├──────┼──────┼──────┼──────┤
     * │      │      │      │      │
     * ├──────┼──────┼──────┼──────┤
     * │(exit)│      │      │(exit)│
     * └──────┴──────┴──────┴──────┘
     */
    [_LIGHT] = LAYOUT(
        _______, RGB_TOG, RGB_MOD, _______,
        RGB_HUI, RGB_SAI, RGB_VAI, RGB_SPI,
        RGB_HUD, RGB_SAD, RGB_VAD, RGB_SPD,
        RGB_RMOD,KC_NO,   KC_NO,   KC_NO,
        KC_NO,   KC_NO,   KC_NO,   KC_NO,
        _______, KC_NO,   KC_NO,   _______
    ),
};
// clang-format on

// The combos are looked up on the factory layer (COMBO_ONLY_FROM_LAYER in
// config.h), so they follow the corner keys' positions on every layer.
const uint16_t PROGMEM swap_combo[]  = {KC_A, KC_D, COMBO_END}; // top corners
const uint16_t PROGMEM light_combo[] = {KC_U, KC_X, COMBO_END}; // bottom corners
const uint16_t PROGMEM four_combo[]  = {KC_A, KC_B, KC_C, KC_D, COMBO_END}; // whole top row

enum combo_ids { COMBO_TOP, COMBO_BOTTOM, COMBO_TOP_ROW };

combo_t key_combos[] = {
    [COMBO_TOP]     = COMBO(swap_combo, PAD_SWAP),
    [COMBO_BOTTOM]  = COMBO(light_combo, PAD_LIGHT),
    [COMBO_TOP_ROW] = COMBO(four_combo, PAD_SWAP), // the top combo, as all four top keys (app setting)
};

// Four keys take a little longer to land together than two
uint16_t get_combo_term(uint16_t index, combo_t *combo) {
    return index == COMBO_TOP_ROW ? 90 : COMBO_TERM;
}

// ---------------------------------------------------------------------------
// Combo settings from the app (HOSTFX_COMBOS), kept in the paint store's spare bytes
//   extra slot 0: [flags: bit 0 top off, bit 1 bottom off, bit 2 corner layer lights on,
//                  bit 3 top combo is all four top keys instead of the two corners]
//   extra slot 1: top action (u16 big endian), 0 = switch layers
//   extra slot 2: bottom action, 0 = lighting controls
// ---------------------------------------------------------------------------

static bool    combos_loaded = false;
static uint8_t combo_flags   = 0;
static uint16_t combo_action[2];

static void combos_load(void) {
    if (combos_loaded) return;
    uint8_t v[3];
    surface_extra_get(0, v);
    combo_flags = v[0];
    for (uint8_t i = 0; i < 2; i++) {
        surface_extra_get(1 + i, v);
        combo_action[i] = ((uint16_t)v[0] << 8) | v[1];
    }
    combos_loaded = true;
}

bool hostfx_combos_get(uint8_t *d) {
    combos_load();
    d[0] = !(combo_flags & 1);
    d[1] = !(combo_flags & 2);
    d[2] = combo_action[0] >> 8;
    d[3] = combo_action[0] & 0xFF;
    d[4] = combo_action[1] >> 8;
    d[5] = combo_action[1] & 0xFF;
    d[6] = (combo_flags & 4) != 0;
    d[7] = (combo_flags & 8) != 0;
    return true;
}

bool hostfx_combos_set(const uint8_t *d) {
    combos_load();
    combo_flags     = (d[0] ? 0 : 1) | (d[1] ? 0 : 2) | (d[6] ? 4 : 0) | (d[7] ? 8 : 0);
    combo_action[0] = ((uint16_t)d[2] << 8) | d[3];
    combo_action[1] = ((uint16_t)d[4] << 8) | d[5];
    uint8_t v[3]    = {combo_flags, 0, 0};
    surface_extra_set(0, v);
    for (uint8_t i = 0; i < 2; i++) {
        v[0] = combo_action[i] >> 8;
        v[1] = combo_action[i] & 0xFF;
        surface_extra_set(1 + i, v);
    }
    return true;
}

bool combo_should_trigger(uint16_t combo_index, combo_t *combo, uint16_t keycode, keyrecord_t *record) {
    combos_load();
    bool top_row = combo_flags & 8;
    switch (combo_index) {
        case COMBO_TOP:
            return !(combo_flags & 1) && !top_row;
        case COMBO_TOP_ROW:
            return !(combo_flags & 1) && top_row;
        default:
            return !(combo_flags & 2);
    }
}

// ---------------------------------------------------------------------------
// Layers
// ---------------------------------------------------------------------------

#define LAYER_FLASH_MS 600
static uint32_t flash_timer  = 0;
static bool     flash_active = false;

uint8_t hostfx_device_kind(void) {
    return HOSTFX_KIND_MACROPAD;
}

uint8_t hostfx_get_layer(void) {
    return get_highest_layer(default_layer_state) == _FKEYS ? 1 : 0;
}

bool hostfx_set_layer(uint8_t layer) {
    if (layer > 1) return false;
    set_single_persistent_default_layer(layer == 1 ? _FKEYS : _MIDI);
    flash_timer  = timer_read32();
    flash_active = true;
    return true;
}

void enable_factory_mode(bool enable) {
    if (enable)
        layer_on(_FACTORY);
    else
        layer_off(_FACTORY);
}

bool process_record_keymap(uint16_t keycode, keyrecord_t *record) {
    switch (keycode) {
        case PAD_SWAP:
            if (record->event.pressed) {
                combos_load();
                if (combo_action[0]) {
                    tap_code16(combo_action[0]);
                } else {
                    hostfx_set_layer(hostfx_get_layer() == 0 ? 1 : 0);
                }
            }
            return false;
        case PAD_LIGHT:
            if (record->event.pressed) {
                combos_load();
                if (combo_action[1]) {
                    tap_code16(combo_action[1]);
                } else {
                    layer_invert(_LIGHT);
                }
            }
            return false;
    }
    return true;
}

// ---------------------------------------------------------------------------
// Layer indicator LEDs
// ---------------------------------------------------------------------------

// Matrix positions (row, col) of the corner keys, from macropad.h's LAYOUT
#define TOP_LEFT_RC 2, 1
#define TOP_RIGHT_RC 2, 4
#define BOTTOM_LEFT_RC 1, 0
#define BOTTOM_RIGHT_RC 2, 0

static void set_key_hsv(uint8_t row, uint8_t col, uint8_t h, uint8_t s, uint8_t v, uint8_t led_min, uint8_t led_max) {
    uint8_t led = g_led_config.matrix_co[row][col];
    if (led == NO_LED || led < led_min || led >= led_max) return;
    RGB rgb = hsv_to_rgb((HSV){h, s, v});
    rgb_matrix_set_color(led, rgb.r, rgb.g, rgb.b);
}

#define HUE_LAYER_A 128 // cyan
#define HUE_LAYER_B 21  // orange

bool rgb_matrix_indicators_advanced_user(uint8_t led_min, uint8_t led_max) {
    uint8_t val = rgb_matrix_get_val();
    if (val < 64) val = 64; // keep the indicator visible on low brightness
    uint8_t hue = hostfx_get_layer() == 0 ? HUE_LAYER_A : HUE_LAYER_B;

    if (flash_active) {
        if (timer_elapsed32(flash_timer) < LAYER_FLASH_MS) {
            RGB rgb = hsv_to_rgb((HSV){hue, 255, val});
            for (uint8_t i = led_min; i < led_max; i++) {
                rgb_matrix_set_color(i, rgb.r, rgb.g, rgb.b);
            }
            return false;
        }
        flash_active = false;
    }

    // The top corners show the layer only when that is turned on in the app;
    // otherwise they follow the effect like every other key.
    combos_load();
    if (combo_flags & 4) {
        set_key_hsv(TOP_LEFT_RC, hue, 255, val, led_min, led_max);
        set_key_hsv(TOP_RIGHT_RC, hue, 255, val, led_min, led_max);
    }
    if (layer_state_is(_LIGHT)) {
        set_key_hsv(BOTTOM_LEFT_RC, 0, 0, val, led_min, led_max);
        set_key_hsv(BOTTOM_RIGHT_RC, 0, 0, val, led_min, led_max);
    }
    return false;
}
