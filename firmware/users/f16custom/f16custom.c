// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Shared code for the Framework Laptop 16 keyboard and RGB macropad keymaps:
//  * raw HID commands for the PC app (see hostfx_protocol.h / PROTOCOL.md)
//  * host-driven RGB effect frame buffer
//  * EEPROM-saved "hotkey lock" for the Fn lighting keys
//  * small queue of key presses so the PC app can draw ripples

#include "f16custom.h"
#include "via.h"
#include "timer.h"
#include "eeconfig.h"
#include <string.h>

#ifndef RGB_MATRIX_MAXIMUM_BRIGHTNESS
#    define RGB_MATRIX_MAXIMUM_BRIGHTNESS UINT8_MAX
#endif

typedef union {
    uint32_t raw;
    struct {
        bool    hotkey_lock : 1;
        bool    span_on : 1;   // joined with the other device
        uint8_t unused : 6;
        uint8_t span_x0;       // this device's slice of the shared 0..224 surface
        uint8_t span_x1;
    };
} user_config_t;

static user_config_t user_config;
static bool          user_config_loaded = false;

// Loaded lazily: Framework's macropad code already owns keyboard_post_init_user().
static void user_config_load(void) {
    if (!user_config_loaded) {
        user_config.raw    = surface_config_read();
        user_config_loaded = true;
        surface_apply_span(user_config.span_on, user_config.span_x0, user_config.span_x1);
        surface_paint_reload();
    }
}

uint8_t         hostfx_frame[RGB_MATRIX_LED_COUNT][3];
static bool     host_requested   = false; // PC asked for the host-driven effect
static uint32_t last_frame_timer = 0;

static uint8_t key_queue[HOSTFX_KEY_QUEUE];
static uint8_t key_queue_len = 0;

// ---------------------------------------------------------------------------
// Weak hooks for the keymaps
// ---------------------------------------------------------------------------

__attribute__((weak)) bool process_record_keymap(uint16_t keycode, keyrecord_t *record) {
    return true;
}
__attribute__((weak)) uint8_t hostfx_device_kind(void) {
    return HOSTFX_KIND_KEYBOARD;
}
__attribute__((weak)) uint8_t hostfx_get_layer(void) {
    return 0xFF;
}
__attribute__((weak)) bool hostfx_set_layer(uint8_t layer) {
    return false;
}
__attribute__((weak)) bool hostfx_combos_get(uint8_t *d) {
    return false;
}
__attribute__((weak)) bool hostfx_combos_set(const uint8_t *d) {
    return false;
}

// ---------------------------------------------------------------------------
// Hotkey lock (EEPROM)
// ---------------------------------------------------------------------------

bool hostfx_hotkey_lock_get(void) {
    user_config_load();
    return user_config.hotkey_lock;
}

void hostfx_hotkey_lock_set(bool on) {
    user_config_load();
    user_config.hotkey_lock = on;
    surface_config_write(user_config.raw);
}

void eeconfig_init_user(void) {
    user_config.raw    = 0;
    user_config_loaded = true;
    surface_store_reset();
    surface_apply_span(false, 0, 224);
}

// ---------------------------------------------------------------------------
// Host-driven effect
// ---------------------------------------------------------------------------

bool hostfx_host_mode_active(void) {
    return rgb_matrix_get_mode() == RGB_MATRIX_CUSTOM_HOST_DRIVEN;
}

// True while the PC app is actively streaming frames.
bool hostfx_frames_fresh(void) {
    return host_requested && timer_elapsed32(last_frame_timer) < HOSTFX_TIMEOUT_MS;
}

void hostfx_host_mode_set(bool on) {
    if (on) {
        host_requested   = true;
        last_frame_timer = timer_read32();
        if (!rgb_matrix_is_enabled()) {
            rgb_matrix_enable_noeeprom();
        }
        // noeeprom: the effect saved in EEPROM stays the user's own choice
        rgb_matrix_mode_noeeprom(RGB_MATRIX_CUSTOM_HOST_DRIVEN);
    } else {
        if (host_requested) {
            host_requested = false;
            // Back to whatever was saved (effect, color, on/off)
            rgb_matrix_reload_from_eeprom();
        }
    }
}

void housekeeping_task_user(void) {
    user_config_load();
    surface_housekeeping();
    // The app went away (closed, crashed, unplugged): fall back to the saved effect.
    if (host_requested && timer_elapsed32(last_frame_timer) >= HOSTFX_TIMEOUT_MS) {
        hostfx_host_mode_set(false);
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

static void queue_key_press(keyrecord_t *record) {
    uint8_t row = record->event.key.row;
    uint8_t col = record->event.key.col;
    if (row >= MATRIX_ROWS || col >= MATRIX_COLS) return;
    uint8_t led = g_led_config.matrix_co[row][col];
    if (led == NO_LED) return;
    if (key_queue_len >= HOSTFX_KEY_QUEUE) {
        // Drop the oldest press
        memmove(key_queue, key_queue + 1, HOSTFX_KEY_QUEUE - 1);
        key_queue_len = HOSTFX_KEY_QUEUE - 1;
    }
    key_queue[key_queue_len++] = led;
}

// pre_process_record runs before Framework's keyboard-level code. We use it for
// the hotkey lock because Framework's process_record_kb() ignores the return
// value of process_record_user().
bool pre_process_record_user(uint16_t keycode, keyrecord_t *record) {
    if (IS_KEYEVENT(record->event) && record->event.pressed) {
        queue_key_press(record);
    }
    if (hostfx_hotkey_lock_get() && keycode >= QK_LIGHTING && keycode <= QK_LIGHTING_MAX) {
        return false; // swallow RGB_* / BL_* keys entirely
    }
    return true;
}

bool process_record_user(uint16_t keycode, keyrecord_t *record) {
    return process_record_keymap(keycode, record);
}

// ---------------------------------------------------------------------------
// Raw HID (VIA custom channel 0)
// ---------------------------------------------------------------------------

// Returns false when the value is not supported.
static bool handle_get(uint8_t value_id, uint8_t *d) {
    // d points at byte 3 of the report: 29 bytes of payload
    switch (value_id) {
        case HOSTFX_INFO:
            d[0] = HOSTFX_MAGIC_0;
            d[1] = HOSTFX_MAGIC_1;
            d[2] = HOSTFX_PROTOCOL_VERSION;
            d[3] = RGB_MATRIX_LED_COUNT;
            d[4] = RGB_MATRIX_EFFECT_MAX;
            d[5] = RGB_MATRIX_CUSTOM_HOST_DRIVEN;
            d[6] = hostfx_device_kind();
            d[7] = hostfx_hotkey_lock_get();
            d[8] = hostfx_get_layer();
            d[9] = RGB_MATRIX_MAXIMUM_BRIGHTNESS;
            d[10] = RGB_MATRIX_CUSTOM_PAINT_ONLY;
            break;
        case HOSTFX_STATE: {
            HSV hsv = rgb_matrix_get_hsv();
            d[0]    = rgb_matrix_is_enabled();
            d[1]    = rgb_matrix_get_mode();
            d[2]    = hsv.h;
            d[3]    = hsv.s;
            d[4]    = hsv.v;
            d[5]    = rgb_matrix_get_speed();
            d[6]    = hostfx_frames_fresh();
            break;
        }
        case HOSTFX_HOTKEY_LOCK:
            d[0] = hostfx_hotkey_lock_get();
            break;
        case HOSTFX_LED_INFO: {
            uint8_t start = d[0];
            uint8_t count = 0;
            for (uint8_t i = 0; i < HOSTFX_LEDS_PER_INFO_MSG && start + i < RGB_MATRIX_LED_COUNT; i++) {
                uint8_t led       = start + i;
                d[2 + i * 3 + 0] = surface_led_x(led); // real position, even while joined
                d[2 + i * 3 + 1] = g_led_config.point[led].y;
                d[2 + i * 3 + 2] = g_led_config.flags[led];
                count++;
            }
            d[1] = count;
            break;
        }
        case HOSTFX_HOST_MODE:
            d[0] = host_requested && hostfx_host_mode_active();
            break;
        case HOSTFX_KEY_EVENTS: {
            uint8_t n = key_queue_len < HOSTFX_KEYS_PER_MSG ? key_queue_len : HOSTFX_KEYS_PER_MSG;
            d[0]      = n;
            memcpy(&d[1], key_queue, n);
            memmove(key_queue, key_queue + n, key_queue_len - n);
            key_queue_len -= n;
            break;
        }
        case HOSTFX_LAYER:
            d[0] = hostfx_get_layer();
            break;
        case HOSTFX_SPAN:
            surface_span_get(d);
            break;
        case HOSTFX_PAINT:
            surface_paint_get(d);
            break;
        case HOSTFX_PAINT_CTRL:
            surface_paint_ctrl_get(d);
            break;
        case HOSTFX_COMBOS:
            return hostfx_combos_get(d);
        default:
            break;
    }
    return true;
}

static bool handle_set(uint8_t value_id, uint8_t *d) {
    switch (value_id) {
        case HOSTFX_STATE: {
            bool    enabled = d[0];
            uint8_t mode    = d[1];
            bool    save    = d[6];
            if (mode == 0 || mode >= RGB_MATRIX_EFFECT_MAX) return false;
            // Picking a normal effect ends host-driven mode
            host_requested = false;
            if (save) {
                if (enabled) rgb_matrix_enable(); else rgb_matrix_disable();
                rgb_matrix_mode(mode);
                rgb_matrix_sethsv(d[2], d[3], d[4]);
                rgb_matrix_set_speed(d[5]);
            } else {
                if (enabled) rgb_matrix_enable_noeeprom(); else rgb_matrix_disable_noeeprom();
                rgb_matrix_mode_noeeprom(mode);
                rgb_matrix_sethsv_noeeprom(d[2], d[3], d[4]);
                rgb_matrix_set_speed_noeeprom(d[5]);
            }
            return true;
        }
        case HOSTFX_HOTKEY_LOCK:
            hostfx_hotkey_lock_set(d[0] != 0);
            return true;
        case HOSTFX_FRAME: {
            uint8_t start = d[0];
            uint8_t count = d[1];
            if (count > HOSTFX_LEDS_PER_FRAME_MSG) return false;
            for (uint8_t i = 0; i < count && start + i < RGB_MATRIX_LED_COUNT; i++) {
                memcpy(hostfx_frame[start + i], &d[2 + i * 3], 3);
            }
            if (host_requested) last_frame_timer = timer_read32();
            return true;
        }
        case HOSTFX_HOST_MODE:
            hostfx_host_mode_set(d[0] != 0);
            return true;
        case HOSTFX_LAYER:
            return hostfx_set_layer(d[0]);
        case HOSTFX_SPAN:
            user_config_load();
            user_config.span_on = d[0] != 0;
            user_config.span_x0 = d[1];
            user_config.span_x1 = d[2];
            surface_config_write(user_config.raw);
            surface_apply_span(user_config.span_on, d[1], d[2]);
            return true;
        case HOSTFX_CLOCK:
            surface_set_clock((uint32_t)d[0] | ((uint32_t)d[1] << 8) | ((uint32_t)d[2] << 16) | ((uint32_t)d[3] << 24));
            return true;
        case HOSTFX_PAINT:
            return surface_paint_set(d);
        case HOSTFX_PAINT_CTRL:
            return surface_paint_ctrl(d[0]);
        case HOSTFX_COMBOS:
            return hostfx_combos_set(d);
        default:
            return false;
    }
}

// VIA calls this for custom values on channels it does not handle itself.
void via_custom_value_command_kb(uint8_t *data, uint8_t length) {
    uint8_t *command_id = &data[0];
    uint8_t  channel_id = data[1];
    uint8_t  value_id   = data[2];
    uint8_t *payload    = &data[3];

    // Note: don't check length; Framework reports the RP2040 sometimes gets 31 bytes.
    (void)length;
    if (channel_id != id_custom_channel) {
        *command_id = id_unhandled;
        return;
    }
    switch (*command_id) {
        case id_custom_get_value:
            if (!handle_get(value_id, payload)) *command_id = id_unhandled;
            break;
        case id_custom_set_value:
            if (!handle_set(value_id, payload)) *command_id = id_unhandled;
            break;
        case id_custom_save:
            // Settings are saved immediately; nothing to do.
            break;
        default:
            *command_id = id_unhandled;
            break;
    }
}
