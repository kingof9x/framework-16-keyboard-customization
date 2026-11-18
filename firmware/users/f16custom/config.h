// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

// How long (ms) the host-driven effect keeps showing the last frame after the
// PC app stops sending. After this the device falls back to the effect that is
// saved in EEPROM, so a crashed app never leaves the keyboard frozen.
#ifndef HOSTFX_TIMEOUT_MS
#    define HOSTFX_TIMEOUT_MS 3000
#endif

// EEPROM space: settings word + version + follow + 128 * (hue, sat, on)
// (see store_t in surface.c)
#define EECONFIG_USER_DATA_SIZE (4 + 2 + 128 * 3)

// Number of key presses buffered for the PC app between polls.
#ifndef HOSTFX_KEY_QUEUE
#    define HOSTFX_KEY_QUEUE 32
#endif
