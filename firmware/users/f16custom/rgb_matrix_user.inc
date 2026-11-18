// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// "Host driven" effect: shows the colors the PC app streams over raw HID.
// While no app is streaming it shows the normal solid color instead.

RGB_MATRIX_EFFECT(HOST_DRIVEN)
// All LEDs off; only painted keys light up (see surface.c)
RGB_MATRIX_EFFECT(PAINT_ONLY)

#ifdef RGB_MATRIX_CUSTOM_EFFECT_IMPLS

extern uint8_t hostfx_frame[RGB_MATRIX_LED_COUNT][3];
bool           hostfx_frames_fresh(void);

static bool HOST_DRIVEN(effect_params_t *params) {
    RGB_MATRIX_USE_LIMITS(led_min, led_max);
    bool    fresh = hostfx_frames_fresh();
    uint8_t val   = rgb_matrix_config.hsv.v; // Fn brightness keys still work
    RGB     solid = hsv_to_rgb(rgb_matrix_config.hsv);
    for (uint8_t i = led_min; i < led_max; i++) {
        RGB_MATRIX_TEST_LED_FLAGS();
        if (fresh) {
            rgb_matrix_set_color(i, scale8(hostfx_frame[i][0], val), scale8(hostfx_frame[i][1], val), scale8(hostfx_frame[i][2], val));
        } else {
            rgb_matrix_set_color(i, solid.r, solid.g, solid.b);
        }
    }
    return rgb_matrix_check_finished_leds(led_max);
}

static bool PAINT_ONLY(effect_params_t *params) {
    RGB_MATRIX_USE_LIMITS(led_min, led_max);
    for (uint8_t i = led_min; i < led_max; i++) {
        RGB_MATRIX_TEST_LED_FLAGS();
        rgb_matrix_set_color(i, 0, 0, 0);
    }
    return rgb_matrix_check_finished_leds(led_max);
}

#endif // RGB_MATRIX_CUSTOM_EFFECT_IMPLS
