# Macropad → Resolume mapping sheet

The macropad has two layers. Switch between them by pressing the **two top
corner keys at the same time**. The top corner keys glow **cyan on layer A**
and **orange on layer B**, and the whole pad flashes that color when you
switch. The layer is remembered after unplugging. You can also switch it in
the app.

* **Layer A: MIDI.** The pad is a USB MIDI device. Each key sends a MIDI note
  on channel 1 at full velocity (note on when pressed, note off when released).
* **Layer B: numpad.** Framework's numpad layout. Change any key (to a key,
  a key combination or "open an app") on the app's **Key mapping** page.

Which layer should I use? MIDI (A) is the most reliable in Resolume, and the
mapping works even when Resolume isn't the active window. Keys (B) work in any
program that accepts keyboard shortcuts, but only while that program is the
focused window.

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

## Layer B: numpad

The numpad's big keys (+, Enter, 0) take two of the macropad's squares.

| Col 1 | Col 2 | Col 3 | Col 4 |
|---|---|---|---|
| Esc | Calculator | = | Backspace |
| Num Lock | / | * | − |
| 7 | 8 | 9 | + |
| 4 | 5 | 6 | + |
| 1 | 2 | 3 | Enter |
| 0 | 0 | . | Enter |

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
| 1 | 1 | 48 | C3 | Esc | | top corner: press both = switch layer A/B |
| 1 | 2 | 49 | C#3 | Calculator | |  |
| 1 | 3 | 50 | D3 | = | |  |
| 1 | 4 | 51 | D#3 | Backspace | | top corner: press both = switch layer A/B |
| 2 | 1 | 52 | E3 | Num Lock | |  |
| 2 | 2 | 53 | F3 | / | |  |
| 2 | 3 | 54 | F#3 | * | |  |
| 2 | 4 | 55 | G3 | − | |  |
| 3 | 1 | 56 | G#3 | 7 | |  |
| 3 | 2 | 57 | A3 | 8 | |  |
| 3 | 3 | 58 | A#3 | 9 | |  |
| 3 | 4 | 59 | B3 | + | |  |
| 4 | 1 | 60 | C4 | 4 | |  |
| 4 | 2 | 61 | C#4 | 5 | |  |
| 4 | 3 | 62 | D4 | 6 | |  |
| 4 | 4 | 63 | D#4 | + | |  |
| 5 | 1 | 64 | E4 | 1 | |  |
| 5 | 2 | 65 | F4 | 2 | |  |
| 5 | 3 | 66 | F#4 | 3 | |  |
| 5 | 4 | 67 | G4 | Enter | |  |
| 6 | 1 | 68 | G#4 | 0 | | bottom corner: press both = lighting controls |
| 6 | 2 | 69 | A4 | 0 | |  |
| 6 | 3 | 70 | A#4 | . | |  |
| 6 | 4 | 71 | B4 | Enter | | bottom corner: press both = lighting controls |

> **About the corner keys:** each corner key still works alone, but it waits
> 40 ms to check whether its partner is pressed too. If you need perfect timing
> (for example, beat-matched clip triggers), map your most timing-critical
> actions to the other keys.

## Mapping MIDI in Resolume (layer A)

1. Plug in / install the macropad and make sure it is on **layer A**
   (the pad flashes cyan when you switch to it).
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

1. Switch the pad to **layer B** (the pad flashes orange). Give the keys you
   want to use their own shortcuts first, on the app's **Key mapping** page:
   a key combination that Resolume doesn't use yet works best.
2. Open the **Shortcuts** menu and choose the **Keyboard** mapping mode
   (Composition or Application, as above).
3. Click a control, then press the pad key. Resolume stores the key with any
   Ctrl/Shift/Alt held, so Ctrl+F13 and Shift+F13 are different shortcuts.
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
