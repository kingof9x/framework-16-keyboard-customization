# Macropad → Resolume mapping sheet

The macropad has two layers. Switch between them by pressing the **two top
corner keys at the same time**. The top corner keys glow **cyan on layer A**
and **orange on layer B**, and the whole pad flashes that colour when you
switch. The layer is remembered after unplugging. You can also switch it in
the app.

* **Layer A: MIDI.** The pad is a USB MIDI device. Each key sends a MIDI note
  on channel 1 at full velocity (note on when pressed, note off when released).
* **Layer B: keyboard keys.** F13–F24, plus Ctrl/Shift versions. No normal
  program uses these keys, so they never clash with typing.

Which layer should I use? MIDI (A) is the most reliable in Resolume, and the
mapping works even when Resolume isn't the active window. Keys (B) work in any
program that accepts keyboard shortcuts, but only while Resolume is the focused
window.

## Layer A: MIDI notes

Rows go from top (1) to bottom (6), seen from above with the pad installed.
Numbers are MIDI note numbers. Programs name notes differently: the name in
brackets uses "middle C = C4 = 60". Some programs, Ableton for example, call
note 60 "C3", one octave lower. Resolume shows what it received when you map,
so you don't need to work this out.

| Col 1 | Col 2 | Col 3 | Col 4 |
|---|---|---|---|
| 48 (C3) | 49 (C#3) | 50 (D3) | 51 (D#3) |
| 52 (E3) | 53 (F3) | 54 (F#3) | 55 (G3) |
| 56 (G#3) | 57 (A3) | 58 (A#3) | 59 (B3) |
| 60 (C4) | 61 (C#4) | 62 (D4) | 63 (D#4) |
| 64 (E4) | 65 (F4) | 66 (F#4) | 67 (G4) |
| 68 (G#4) | 69 (A4) | 70 (A#4) | 71 (B4) |

## Layer B: keys

| Col 1 | Col 2 | Col 3 | Col 4 |
|---|---|---|---|
| F13 | F14 | F15 | F16 |
| F17 | F18 | F19 | F20 |
| F21 | F22 | F23 | F24 |
| Ctrl+F13 | Ctrl+F14 | Ctrl+F15 | Ctrl+F16 |
| Ctrl+F17 | Ctrl+F18 | Ctrl+F19 | Ctrl+F20 |
| Shift+F13 | Shift+F14 | Shift+F15 | Shift+F16 |

## Lighting controls (both corners at the bottom)

Pressing the two **bottom** corner keys together turns an overlay on or off
(the bottom corners glow white while it's on):

| Col 1 | Col 2 | Col 3 | Col 4 |
|---|---|---|---|
| *(layer key)* | Lights on/off | Next effect | *(layer key)* |
| Hue + | Saturation + | Brightness + | Speed + |
| Hue − | Saturation − | Brightness − | Speed − |
| Previous effect | – | – | – |
| – | – | – | – |
| *(exit)* | – | – | *(exit)* |

With **Hotkey lock** on in the app, these lighting keys (and Fn + lighting
keys on the keyboard) do nothing.

## Full key list, with space for your own notes

Print this table or copy it into a spreadsheet and fill in the "Resolume
action" column as you map.

| Row | Col | Layer A note | Name | Layer B key | Resolume action | Notes |
|---|---|---|---|---|---|---|
| 1 | 1 | 48 | C3 | F13 | | top corner: press both = switch layer A/B |
| 1 | 2 | 49 | C#3 | F14 | |  |
| 1 | 3 | 50 | D3 | F15 | |  |
| 1 | 4 | 51 | D#3 | F16 | | top corner: press both = switch layer A/B |
| 2 | 1 | 52 | E3 | F17 | |  |
| 2 | 2 | 53 | F3 | F18 | |  |
| 2 | 3 | 54 | F#3 | F19 | |  |
| 2 | 4 | 55 | G3 | F20 | |  |
| 3 | 1 | 56 | G#3 | F21 | |  |
| 3 | 2 | 57 | A3 | F22 | |  |
| 3 | 3 | 58 | A#3 | F23 | |  |
| 3 | 4 | 59 | B3 | F24 | |  |
| 4 | 1 | 60 | C4 | Ctrl+F13 | |  |
| 4 | 2 | 61 | C#4 | Ctrl+F14 | |  |
| 4 | 3 | 62 | D4 | Ctrl+F15 | |  |
| 4 | 4 | 63 | D#4 | Ctrl+F16 | |  |
| 5 | 1 | 64 | E4 | Ctrl+F17 | |  |
| 5 | 2 | 65 | F4 | Ctrl+F18 | |  |
| 5 | 3 | 66 | F#4 | Ctrl+F19 | |  |
| 5 | 4 | 67 | G4 | Ctrl+F20 | |  |
| 6 | 1 | 68 | G#4 | Shift+F13 | | bottom corner: press both = lighting controls |
| 6 | 2 | 69 | A4 | Shift+F14 | |  |
| 6 | 3 | 70 | A#4 | Shift+F15 | |  |
| 6 | 4 | 71 | B4 | Shift+F16 | | bottom corner: press both = lighting controls |

> **About the corner keys:** each corner key still works alone, but it waits
> 40 ms to check whether its partner is pressed too. If you need perfect timing
> (for example, beat-matched clip triggers), map your most timing-critical
> actions to the other keys.

## Mapping MIDI in Resolume (layer A)

1. Plug in / install the macropad and make sure it is on **layer A**
   (top corners cyan).
2. In Resolume open **Preferences → MIDI**. The macropad appears as a MIDI
   input with a name like *Laptop 16 RGB Macropad*. Turn its **Input** on.
3. Open the **Shortcuts** menu and choose the **MIDI** mapping mode. Pick
   *Composition* for mappings saved with this show, or *Application* for
   mappings that apply to every show (good for layer/column controls).
   Mappable controls turn highlighted.
4. Click the thing you want to control: a clip, a column, a layer's bypass
   button, an effect's on/off, and so on.
5. Press the pad key. Resolume shows the note it learned next to the control.
6. Repeat for the other keys, then leave the mapping mode from the same
   **Shortcuts** menu.

Tips:
* The pad sends **notes**, not knobs/faders (CC). Notes are good for
  triggering clips, columns, buttons and toggles. In the shortcut panel you can
  set a note to *toggle* or *momentary* where Resolume offers that choice.
* To control a slider with buttons, map two keys to the slider and set one to
  move up and the other down, if Resolume offers it for that control. Otherwise
  use a keyboard/MIDI controller with knobs for sliders.

## Mapping keys in Resolume (layer B)

1. Switch the pad to **layer B** (top corners orange).
2. Open the **Shortcuts** menu and choose the **Keyboard** mapping mode
   (Composition or Application, as above).
3. Click a control, then press the pad key. Resolume stores the key with any
   Ctrl/Shift, so F13, Ctrl+F13 and Shift+F13 are three different shortcuts.
4. Leave the mapping mode from the **Shortcuts** menu.

If Resolume ignores a key in mapping mode, use MIDI (layer A) for that action.
Some programs don't recognise F13–F24. MIDI doesn't have this problem.

## A starting point (optional)

One way to lay out a 4-deck show. Change it to suit your set:

| Row | Layer A (MIDI) idea |
|---|---|
| 1 | Trigger columns 1–4 |
| 2 | Layer 1: clips 1–4 |
| 3 | Layer 2: clips 1–4 |
| 4 | Layer 3: clips 1–4 |
| 5 | Bypass layers 1–4 |
| 6 | Effect toggles (strobe, blur, invert, mirror) |
