// Copyright 2026 kingof9x
// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

// MI_C ... MI_B1 note keycodes need the "advanced" MIDI implementation
#define MIDI_ADVANCED

// Both corner keys must go down within this many ms to count as the combo.
// Short, so single corner presses are only delayed a tiny bit.
#define COMBO_TERM 40

// Combos match on key position, not on what the keys send, so they keep working
// after the corner keys are changed in the app or VIA. Layer 2 is Framework's
// factory test layer: every key sends a different letter (corners: A, D, U, X).
#define COMBO_ONLY_FROM_LAYER 2
// Each combo can be turned off from the app (combo_should_trigger in keymap.c).
#define COMBO_SHOULD_TRIGGER

