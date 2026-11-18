# Framework 16 keyboard + macropad customization

Custom lighting and macropad setup for the **Framework Laptop 16** with the
**RGB ANSI keyboard** and the **RGB macropad**, for use with Resolume. Nothing
needs to be installed on your PC to build it: GitHub builds the firmware and
the Windows app.

| Folder | What it is |
|---|---|
| [`firmware/`](firmware/) | Custom QMK firmware (`f16custom`) for the keyboard and the macropad |
| [`app/`](app/) | **Framework 16 Lights**, a Windows app to control both devices' lighting |
| [`resolume/`](resolume/MAPPING.md) | Key-by-key macropad sheet and how to map it in Resolume |

## What you get

**Both devices**
* All 44 of Framework's built-in lighting effects, plus a **"PC controlled"**
  effect the app can draw on and an **"Off, painted keys only"** effect.
* **Join into one device:** the keyboard and macropad run an effect as one
  surface, so a sweep starts on the keyboard and finishes at the far edge of
  the macropad. This keeps working with the app closed.
* **Painted keys:** give any key its own color. Painted keys can still react
  to the running effect. They're saved on the device.
* The app can read and set the effect, speed, brightness and color.
* **Hotkey lock** (toggle in the app, remembered by the device): Fn + lighting
  keys do nothing, so a stray key press can't change the lights during a show.
* VIA and Framework's `qmk_hid` tool still work.

**Keyboard:** same keys and Fn layer as Framework's official firmware.

**Macropad** (details in [`resolume/MAPPING.md`](resolume/MAPPING.md))
* **Layer A:** 24 MIDI notes (notes 48–71). The pad shows up as a USB MIDI device.
* **Layer B:** F13–F24, Ctrl+F13–F20 and Shift+F13–F16.
* Press the **two top corner keys together** to switch layers. The corners
  glow cyan (A) or orange (B).
* Press the **two bottom corner keys together** for the lighting controls
  overlay.

**App** (tabs: Show, Effects, Macropad, Firmware)
* A live picture of your keyboard and side module, lit with the same effect
  formulas the firmware uses. Nothing changes on the hardware until you press
  **Send to devices**.
* **Your setup:** keyboard Clear RGB, Black RGB, or Black with white backlight;
  side module None, RGB macropad, or Numpad, on the left or right.
* **Show:** try any effect across both devices as one surface, and paint keys
  with your own colors.
* **Effects:** a separate effect for each device, or **Join into one device**.
  Full color picker (any color the LEDs can make), speed and brightness.
* **Macropad:** switch layers and see what each key sends.
* **Firmware:** restart a device into flashing mode.
* App-drawn effects (wave, rainbow sweep, keypress ripple), hotkey lock, and
  dark or light mode following Windows.

> **Only for the ANSI (US) keyboard and the RGB macropad.** Don't flash the
> keyboard file onto an ISO/JIS keyboard or the numpad. If your ANSI keyboard
> has the white (non-RGB) backlight, the firmware still works for typing, but
> the color features do nothing.

---

## 1. Download the files

The builds come from GitHub Actions:

1. Open this repo's **Actions** tab.
2. Pick the latest green run of **Firmware**. At the bottom, under
   *Artifacts*, download **firmware-uf2**. Do the same in the **Windows app**
   workflow and download **f16-lights-windows**. (You must be signed in to
   GitHub to download artifacts.)
3. Unzip both. You get:
   * `framework_ansi_f16custom.uf2`: keyboard firmware
   * `framework_macropad_f16custom.uf2`: macropad firmware
   * `f16-lights.exe`: the app

If a version tag (for example `v0.1.0`) has been pushed, the same files are
also attached to the matching page under **Releases**.

## 2. Flash the firmware

Each device must first restart into **flashing mode**. In that mode a USB
drive called **RPI-RP2** appears in File Explorer. While a device is in
flashing mode it doesn't type, so use the touchpad (and an external keyboard
if you need one).

**Enter flashing mode, easy way (works on Framework's official firmware too):**
open `f16-lights.exe` → **Firmware update** → *Restart Keyboard into flashing
mode* (or Macropad) → confirm.

**Enter flashing mode, manual way:**
* **Keyboard:** hold **both Alt keys** (left Alt and right Alt) while you
  slide the keyboard module in.
* **Macropad:** hold the key in **row 5, column 2** and the key in **row 4,
  column 3** while you slide the macropad in. Those are the "2" and "6" keys
  on Framework's numpad layout. Rows count from the top, columns from the
  left.

**Then:**
1. Drag the matching `.uf2` file onto the **RPI-RP2** drive.
2. The drive disappears by itself and the device restarts with the new
   firmware. This takes about 5 seconds. Don't unplug it during the copy.
3. Do the other device the same way.

Flashing resets the device's saved settings (lighting, VIA key changes). That
is expected.

## 3. Go back to Framework's official firmware

1. Download the official files from Framework's releases:
   <https://github.com/FrameworkComputer/qmk_firmware/releases>. Use
   `framework_ansi_default.uf2` for the keyboard and
   `framework_macropad_default.uf2` for the macropad. This project is built on
   their **v0.3.1**.
2. Flash them exactly as in step 2 (the app's flashing-mode button also works
   on official firmware).

## 4. Use the app

1. Double-click `f16-lights.exe`. The first time, Windows may say *"Windows
   protected your PC"* because the app isn't signed. Click **More info → Run
   anyway**.
2. At the top, both devices should show **connected (f16custom firmware)**.
   *"found, but not running the f16custom firmware"* means you still need to
   flash that device.
3. Pick **Your setup** at the top: which keyboard you have, which side module
   (the app detects a macropad by itself) and which side it's on. The app
   remembers it.
4. The tabs:
   * **Show:** pick an effect (all 44 built-in effects, "Off, painted keys
     only", or the three app-drawn ones). Set color, speed and brightness. In
     the preview, effects run across the keyboard and side module as one
     surface. Under **Key colors**, pick a color and click or drag across keys
     to paint them. **Painted keys still react to the effect** makes them
     brighten and dim with the effect. Press **Send to devices** to try it on
     the hardware, or **Save as default** to keep it after a restart.
   * **Effects:** an effect, color, speed and brightness for each device.
     **Join into one device** makes one card drive both, running as one
     surface. **Copy to** copies one device's settings to the other.
   * **Macropad:** switch layer A (MIDI) or B (F-keys); click a macropad key in
     the picture to see what it sends.
   * **Firmware:** restart a device into flashing mode.
   * **Hotkey lock** (top right): Fn + lighting keys and the macropad's lighting
     overlay do nothing until you switch it off again.
5. When you close the app, devices running an app-drawn effect go back to
   their saved effect. Built-in effects, joined surfaces and painted keys keep
   running without the app.

Black keyboards with a white backlight and the numpad have one white light for
all keys, so they can only change brightness. The app shows them in plain
white.

Try the app without the hardware: run `f16-lights.exe --demo` from a command
prompt to use two pretend devices.

The app needs no install and no admin rights. You can keep the `.exe`
anywhere.

## 5. Map the macropad in Resolume

See [`resolume/MAPPING.md`](resolume/MAPPING.md) for the key-by-key sheet and
step-by-step MIDI and keyboard mapping.

---

## For tinkerers

* **Building:** done by GitHub Actions on every push and pull request
  (`.github/workflows/`). The firmware workflow runs
  [`firmware/build.sh`](firmware/build.sh) on Ubuntu (Python 3.11 + ARM GCC).
  The app workflow runs the unit tests and builds the `.exe` on a Windows
  machine.
* **How the app talks to the devices:** see
  [`firmware/PROTOCOL.md`](firmware/PROTOCOL.md).
* **Code layout:**
  * `firmware/users/f16custom/`: code shared by both devices (protocol,
    PC-controlled effect, hotkey lock).
  * `firmware/keymaps/ansi/f16custom/`: keyboard keymap (Framework's default
    layout).
  * `firmware/keymaps/macropad/f16custom/`: macropad layers, combos and layer
    LEDs.
  * `app/core/`: effect maths and message encoding, with unit tests.
  * `app/gui/`: the Windows app (egui + hidapi).
* **VIA:** still works. If you remap a macropad **corner key** in VIA, the
  two-key layer switch stops working for that layer, because the combos match
  the original keys.

Licensed GPL-2.0-or-later, like QMK.
