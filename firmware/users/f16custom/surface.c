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
// frame, painted LEDs are recolored. "Follow" takes the brightness the effect
// gave that LED (never below 30%); "tint" also takes the effect's color, turned
// round the color wheel by the painted hue, so effects still wash over them.

#include "f16custom.h"
#include "eeconfig.h"
#include "timer.h"
#include "is31flcommon.h"
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

// An LED's own x position, as if the devices were not joined.
uint8_t surface_led_x(uint8_t i) {
    return orig_saved ? orig_x[i] : g_led_config.point[i].x;
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

#if RGB_MATRIX_LED_COUNT + SURFACE_EXTRA_SLOTS <= HOSTFX_PAINT_MAX_LEDS
void surface_extra_get(uint8_t slot, uint8_t out[3]) {
    memcpy(out, paint.led[HOSTFX_PAINT_MAX_LEDS - 1 - slot], 3);
}

// Saved right away, without saving unsaved painting.
void surface_extra_set(uint8_t slot, const uint8_t v[3]) {
    uint8_t idx = HOSTFX_PAINT_MAX_LEDS - 1 - slot;
    memcpy(paint.led[idx], v, 3);
    store_t s;
    eeconfig_read_user_datablock(&s);
    if (s.version != PAINT_VERSION) {
        memset(&s, 0, sizeof(s));
        s.version = PAINT_VERSION;
        s.follow  = 1;
    }
    memcpy(s.led[idx], v, 3);
    eeconfig_update_user_datablock(&s);
}
#else
void surface_extra_get(uint8_t slot, uint8_t out[3]) {
    memset(out, 0, 3);
}
void surface_extra_set(uint8_t slot, const uint8_t v[3]) {}
#endif

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
            paint.follow = HOSTFX_PAINT_MODE_STEADY;
            return true;
        case HOSTFX_PAINT_FOLLOW_ON:
            paint.follow = HOSTFX_PAINT_MODE_FOLLOW;
            return true;
        case HOSTFX_PAINT_TINT:
            paint.follow = HOSTFX_PAINT_MODE_TINT;
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

static void led_rgb(uint8_t i, uint8_t *r, uint8_t *g, uint8_t *b) {
    is31_led led;
    memcpy_P(&led, &g_is31_leds[i], sizeof(led));
    *r = g_pwm_buffer[led.driver][led.r];
    *g = g_pwm_buffer[led.driver][led.g];
    *b = g_pwm_buffer[led.driver][led.b];
}

// QMK's own scale: hue 0..255 in six 43-wide sections, like hsv_to_rgb.
static HSV rgb_to_hsv_qmk(uint8_t r, uint8_t g, uint8_t b) {
    uint8_t max = r > g ? (r > b ? r : b) : (g > b ? g : b);
    uint8_t min = r < g ? (r < b ? r : b) : (g < b ? g : b);
    HSV     out = {0, 0, max};
    if (max == 0) return out;
    uint8_t delta = max - min;
    out.s         = (uint16_t)delta * 255 / max;
    if (delta == 0) return out;
    int16_t h;
    if (max == r) {
        h = 43 * ((int16_t)g - b) / delta;
    } else if (max == g) {
        h = 85 + 43 * ((int16_t)b - r) / delta;
    } else {
        h = 171 + 43 * ((int16_t)r - g) / delta;
    }
    out.h = (uint8_t)h; // wraps negative values round the circle
    return out;
}

static void paint_frame(void) {
    if (!paint_count) return;
    uint8_t val     = rgb_matrix_get_val();
    bool    only    = rgb_matrix_get_mode() == RGB_MATRIX_CUSTOM_PAINT_ONLY;
    uint8_t floor_v = val * 3 / 10;
    uint8_t base_h  = rgb_matrix_get_hsv().h;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) {
        if (!paint.led[i][2]) continue;
        HSV want = {paint.led[i][0], paint.led[i][1], val};
        if (!only && paint.follow != HOSTFX_PAINT_MODE_STEADY) {
            uint8_t r, g, b;
            led_rgb(i, &r, &g, &b);
            HSV effect = rgb_to_hsv_qmk(r, g, b);
            want.v     = effect.v > floor_v ? effect.v : floor_v;
            if (paint.follow == HOSTFX_PAINT_MODE_TINT) {
                // The effect's color, turned round the color wheel by how far the painted
                // color is from the effect's own color: a key painted the effect's color looks
                // unchanged, one painted opposite shows the opposite color as the effect moves.
                want.h = effect.h + (uint8_t)(paint.led[i][0] - base_h);
                want.s = effect.s == 0 ? paint.led[i][1] : (uint16_t)effect.s * paint.led[i][1] / 255;
            }
        }
        RGB rgb = hsv_to_rgb(want);
        rgb_matrix_set_color(i, rgb.r, rgb.g, rgb.b);
    }
}

// ---------------------------------------------------------------------------
// LED calibration: driver current and per-color gains, to match the devices
// ---------------------------------------------------------------------------

#ifndef F16_DEFAULT_GLOBALCURRENT
#    define F16_DEFAULT_GLOBALCURRENT ISSI_GLOBALCURRENT
#endif

static bool     cal_loaded = false;
static uint8_t  cal_current;           // 0 = default
static uint8_t  cal_gain[3];           // 0 is stored for "full" (255) so a new store means no change
static uint32_t cal_timer   = 0;
static bool     cal_pending = true;

static void cal_load(void) {
    if (cal_loaded) return;
    uint8_t a[3], b[3];
    surface_extra_get(3, a);
    surface_extra_get(4, b);
    cal_current = a[0];
    cal_gain[0] = a[1];
    cal_gain[1] = a[2];
    cal_gain[2] = b[0];
    cal_loaded  = true;
}

static uint8_t gain(uint8_t c) {
    return cal_gain[c] ? cal_gain[c] : 255;
}

void surface_led_cal_get(uint8_t *d) {
    cal_load();
    d[0] = cal_current ? cal_current : F16_DEFAULT_GLOBALCURRENT;
    d[1] = gain(0);
    d[2] = gain(1);
    d[3] = gain(2);
    d[4] = F16_DEFAULT_GLOBALCURRENT;
}

bool surface_led_cal_set(const uint8_t *d) {
    cal_load();
    cal_current = d[0] == F16_DEFAULT_GLOBALCURRENT ? 0 : d[0];
    for (uint8_t c = 0; c < 3; c++) cal_gain[c] = d[1 + c] == 255 ? 0 : d[1 + c];
    cal_pending = true;
    if (d[4]) {
        uint8_t a[3] = {cal_current, cal_gain[0], cal_gain[1]};
        uint8_t b[3] = {cal_gain[2], 0, 0};
        surface_extra_set(3, a);
        surface_extra_set(4, b);
    }
    return true;
}

static void write_current(uint8_t addr, uint8_t current) {
    IS31FL_unlock_register(addr, ISSI_PAGE_FUNCTION);
    IS31FL_write_single_register(addr, ISSI_REG_GLOBALCURRENT, current);
}

// Sets the driver current after a change, and again every few seconds in case the
// LED chip was set up again (waking from sleep resets it to Framework's value).
static void cal_apply_current(void) {
    if (!cal_pending && timer_elapsed32(cal_timer) < 5000) return;
    uint8_t current = cal_current ? cal_current : F16_DEFAULT_GLOBALCURRENT;
    write_current(DRIVER_ADDR_1, current);
#if DRIVER_COUNT > 1
    write_current(DRIVER_ADDR_2, current);
#endif
    cal_pending = false;
    cal_timer   = timer_read32();
}

static void cal_frame(void) {
    cal_load();
    cal_apply_current();
    if (!cal_gain[0] && !cal_gain[1] && !cal_gain[2]) return;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) {
        uint8_t r, g, b;
        led_rgb(i, &r, &g, &b);
        rgb_matrix_set_color(i, (uint16_t)r * gain(0) / 255, (uint16_t)g * gain(1) / 255, (uint16_t)b * gain(2) / 255);
    }
}

// Runs once per frame, after the effect has drawn every LED.
bool rgb_matrix_indicators_user(void) {
    paint_frame();
    cal_frame();
    clock_frame_done();
    return true;
}
