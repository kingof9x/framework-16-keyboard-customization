// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// "Join into one device" and painted keys.
//
// Joined surface: QMK effects place LEDs on a 0..224 x 0..64 grid. When the
// keyboard and macropad are joined, each device squeezes its LEDs into its own
// slice of that grid (e.g. keyboard 0..181, macropad 186..224), so every
// built-in effect flows across both devices as one surface, with or without
// the PC app running. The app also lines up the two devices' effect clocks.
//
// Painted keys: any LED can have its own hue/sat. After the effect has drawn a
// frame, painted LEDs are recolored. With "follow" on they take the brightness
// the effect gave that LED (never below 30%), so effects still wash over them.

#include "f16custom.h"
#include "eeconfig.h"
#include "timer.h"
#include <string.h>

// ---------------------------------------------------------------------------
// Joined surface
// ---------------------------------------------------------------------------

static uint8_t  orig_x[RGB_MATRIX_LED_COUNT];
static bool     orig_saved = false;
static bool     span_on    = false;
static uint8_t  span_x0 = 0, span_x1 = 224;

static void save_original_points(void) {
    if (orig_saved) return;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) orig_x[i] = g_led_config.point[i].x;
    orig_saved = true;
}

void surface_apply_span(bool on, uint8_t x0, uint8_t x1) {
    save_original_points();
    if (x1 < x0 || x1 > 224) on = false;
    span_on = on;
    span_x0 = x0;
    span_x1 = x1;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) {
        g_led_config.point[i].x = on ? (uint8_t)(x0 + (uint16_t)orig_x[i] * (x1 - x0) / 224) : orig_x[i];
    }
}

void surface_span_get(uint8_t *d) {
    d[0] = span_on;
    d[1] = span_x0;
    d[2] = span_x1;
}

// Effect clock offset: effects read g_rgb_timer, which QMK sets from the
// system timer at the start of every frame. We add our offset after the frame
// has started (housekeeping runs between QMK's "start" and "render" steps) and
// take it off again once the frame is drawn, so QMK's own frame timing is untouched.
static int32_t  clock_offset    = 0;
static bool     clock_applied   = false;
static uint32_t clock_last_raw  = 0;

void surface_set_clock(uint32_t ms) {
    clock_offset = (int32_t)(ms - timer_read32());
}

void surface_housekeeping(void) {
    if (!clock_offset) return;
    if (!clock_applied && g_rgb_timer != clock_last_raw) {
        clock_last_raw = g_rgb_timer;
        g_rgb_timer += clock_offset;
        clock_applied = true;
    }
}

static void clock_frame_done(void) {
    if (clock_applied && g_rgb_timer == clock_last_raw + clock_offset) {
        g_rgb_timer = clock_last_raw;
    }
    clock_applied = false;
}

// ---------------------------------------------------------------------------
// Painted keys
// ---------------------------------------------------------------------------

// Everything we keep in EEPROM. (With a user datablock, QMK has no separate
// 32-bit user config, so the settings word lives here too.)
typedef struct __attribute__((packed)) {
    uint32_t config;                        // see user_config_t in f16custom.c
    uint8_t  version;
    uint8_t  follow;
    uint8_t  led[HOSTFX_PAINT_MAX_LEDS][3]; // hue, sat, on
} store_t;
_Static_assert(sizeof(store_t) == EECONFIG_USER_DATA_SIZE, "EECONFIG_USER_DATA_SIZE must match store_t");
_Static_assert(RGB_MATRIX_LED_COUNT <= HOSTFX_PAINT_MAX_LEDS, "too many LEDs for paint storage");

#define PAINT_VERSION 1
static store_t paint; // working copy; painting changes it, PAINT_SAVE writes it
static uint8_t       paint_count = 0;

static void paint_recount(void) {
    paint_count = 0;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) paint_count += paint.led[i][2] ? 1 : 0;
}

void surface_paint_reload(void) {
    eeconfig_read_user_datablock(&paint);
    if (paint.version != PAINT_VERSION) {
        uint32_t cfg = paint.config;
        memset(&paint, 0, sizeof(paint));
        paint.config  = cfg;
        paint.version = PAINT_VERSION;
        paint.follow  = 1;
    }
    paint_recount();
}

void surface_store_reset(void) {
    memset(&paint, 0, sizeof(paint));
    paint.version = PAINT_VERSION;
    paint.follow  = 1;
    eeconfig_update_user_datablock(&paint);
    paint_count = 0;
}

uint32_t surface_config_read(void) {
    store_t s;
    eeconfig_read_user_datablock(&s);
    return s.version == PAINT_VERSION ? s.config : 0;
}

// Saves only the settings word: unsaved painting stays unsaved.
void surface_config_write(uint32_t config) {
    store_t s;
    eeconfig_read_user_datablock(&s);
    if (s.version != PAINT_VERSION) {
        memset(&s, 0, sizeof(s));
        s.version = PAINT_VERSION;
        s.follow  = 1;
    }
    s.config     = config;
    paint.config = config;
    eeconfig_update_user_datablock(&s);
}

void surface_paint_get(uint8_t *d) {
    uint8_t start = d[0], n = 0;
    for (uint8_t i = 0; i < HOSTFX_PAINT_PER_GET_MSG && start + i < RGB_MATRIX_LED_COUNT; i++, n++) {
        memcpy(&d[2 + i * 3], paint.led[start + i], 3);
    }
    d[1] = n;
}

bool surface_paint_set(const uint8_t *d) {
    uint8_t start = d[0], count = d[1];
    if (count > HOSTFX_PAINT_PER_SET_MSG) return false;
    for (uint8_t i = 0; i < count && start + i < RGB_MATRIX_LED_COUNT; i++) {
        paint.led[start + i][0] = d[2 + i * 3];
        paint.led[start + i][1] = d[3 + i * 3];
        paint.led[start + i][2] = d[4 + i * 3] ? 1 : 0;
    }
    paint_recount();
    return true;
}

void surface_paint_ctrl_get(uint8_t *d) {
    d[0] = paint.follow;
    d[1] = paint_count;
}

bool surface_paint_ctrl(uint8_t op) {
    switch (op) {
        case HOSTFX_PAINT_CLEAR:
            for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) paint.led[i][2] = 0;
            paint_count = 0;
            return true;
        case HOSTFX_PAINT_SAVE:
            paint.config = surface_config_read();
            eeconfig_update_user_datablock(&paint);
            return true;
        case HOSTFX_PAINT_FOLLOW_OFF:
        case HOSTFX_PAINT_FOLLOW_ON:
            paint.follow = op == HOSTFX_PAINT_FOLLOW_ON;
            return true;
        case HOSTFX_PAINT_RELOAD:
            surface_paint_reload();
            return true;
        default:
            return false;
    }
}

// The IS31FL3743A driver keeps the frame it is about to send here; reading it
// tells us how bright the effect made each LED.
extern uint8_t g_pwm_buffer[DRIVER_COUNT][ISSI_MAX_LEDS];

static void paint_frame(void) {
    if (!paint_count) return;
    uint8_t val      = rgb_matrix_get_val();
    bool    only     = rgb_matrix_get_mode() == RGB_MATRIX_CUSTOM_PAINT_ONLY;
    uint8_t floor_v  = val * 3 / 10;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) {
        if (!paint.led[i][2]) continue;
        uint8_t v = val;
        if (paint.follow && !only) {
            is31_led led;
            memcpy_P(&led, &g_is31_leds[i], sizeof(led));
            uint8_t r = g_pwm_buffer[led.driver][led.r], g = g_pwm_buffer[led.driver][led.g], b = g_pwm_buffer[led.driver][led.b];
            uint8_t lvl = r > g ? (r > b ? r : b) : (g > b ? g : b);
            v = lvl > floor_v ? lvl : floor_v;
        }
        RGB rgb = hsv_to_rgb((HSV){paint.led[i][0], paint.led[i][1], v});
        rgb_matrix_set_color(i, rgb.r, rgb.g, rgb.b);
    }
}

// Runs once per frame, after the effect has drawn every LED.
bool rgb_matrix_indicators_user(void) {
    paint_frame();
    clock_frame_done();
    return true;
}
