// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

// MI_C ... MI_B1 note keycodes need the "advanced" MIDI implementation
#define MIDI_ADVANCED

// Both corner keys must go down within this many ms to count as the combo.
// Short, so single corner presses are only delayed a tiny bit.
#define COMBO_TERM 40
