# Keyboard HUD firmware module

`hud/` adds the HUD protocol ([docs/PROTOCOL.md](../docs/PROTOCOL.md)) to any QMK keymap built with [vial-qmk](https://github.com/vial-kb/vial-qmk). It answers the HUD's requests on the Raw HID interface Vial already uses: it never sends reports on its own (so it does not disturb Vial) and never writes to EEPROM.

**Requirements**: Vial (or VIA with a dynamic keymap), a key matrix of at most 8 rows × 8 columns, at most 32 layers (Layer Lock is reported for layers 0–7). The build stops with a clear error otherwise. The module costs under 1 KB of flash.

## Adding it to your keymap

1. Copy the `hud/` folder into your keymap folder, next to `rules.mk`.
2. `rules.mk`:
   ```make
   include $(dir $(lastword $(MAKEFILE_LIST)))hud/hud.mk
   ```
3. `keymap.c`:
   ```c
   #include "hud.h"

   void housekeeping_task_user(void) {
       hud_task();
   }
   ```
   If you already have a `housekeeping_task_user()`, add the call to it.
4. Optional, recommended: a **`HUD_TOG`** key. It shows/hides the HUD with a tap and toggles click-through with a double tap, so no global shortcut is needed. Add it **at the end** of your custom keycodes (Vial identifies them by position), both in `keymap.c` and in `customKeycodes` of `vial.json`:
   ```c
   enum custom_keycodes {
       // ... your keys ...
       HUD_TOG,
   };

   uint16_t hud_toggle_keycode(void) {
       return HUD_TOG;
   }

   bool process_record_user(uint16_t keycode, keyrecord_t *record) {
       switch (keycode) {
           case HUD_TOG:
               if (record->event.pressed) hud_toggle_pressed();
               return false;
       }
       return true;
   }
   ```
   ```json
   "customKeycodes": [
       {"name": "HUD_TOG", "title": "Keyboard HUD: show/hide (double tap: click-through)", "shortName": "HUD"}
   ]
   ```
   After flashing, assign `HUD_TOG` from Vial (*User* tab) wherever you like.

Then build and flash as usual. On a split keyboard, flash **both** halves: the master answers for the whole keyboard.

## What the HUD shows, and where it comes from

| HUD | Source | Notes |
|---|---|---|
| Layers, locked layers, one-shot layer | `layer_state`, `default_layer_state`, Layer Lock, `get_oneshot_layer()` | Layer Lock needs `LAYER_LOCK_ENABLE` |
| Pressed keys | `matrix_get_row()` | on a split keyboard the master has both halves |
| Legends | `keymap_key_to_keycode()` | the keycodes as the firmware resolves them, remaps included |
| Modifiers | `get_mods()`, weak, one-shot, locked one-shot mods | |
| Caps Lock, Num Lock | host LED state | |
| Caps Word | `is_caps_word_on()` | needs `CAPS_WORD_ENABLE` |
| OS | `detected_host_os()` | needs `OS_DETECTION_ENABLE`, or the hooks below |
| Other half disconnected | `is_transport_connected()` | split keyboards |
| Keymap changes made with Vial | the module re-reads the keymap a few keys per cycle and hashes it | a full pass takes a few ms |

## Optional hooks

The module defines these functions as weak: define them in your keymap to override the defaults.

| Hook | Default | Use it for |
|---|---|---|
| `uint16_t hud_toggle_keycode(void)` | `0` (no HUD_TOG key) | the keycode of your `HUD_TOG` key |
| `bool hud_colemak_on(void)` | `false` | a QWERTY/Colemak toggle in `keymap_key_to_keycode()`. Also `#define HUD_HAS_COLEMAK` in `config.h`, so that the HUD shows the QWERTY/Colemak-DH chip |
| `uint8_t hud_os_mode(void)` | `0` | an OS chosen by the user: 0 auto, 1 Windows, 2 macOS, 3 Linux |
| `uint8_t hud_active_os(void)` | from OS detection | the OS your keymap acts on: 1 Windows, 2 macOS, 3 Linux, 0 unknown (the HUD hides the OS chip) |
| `bool hud_user_flag(void)` | `false` | a free status bit (flag b3 of `GET_STATE`) |

And two calls:

- `hud_toggle_pressed()`: from `process_record_user()` when `HUD_TOG` goes down;
- `hud_keymap_changed()`: when the effective keymap changes without Vial, e.g. after a Colemak toggle, so that the HUD re-reads the legends at once.

## Example

[`examples/crkbd/`](examples/crkbd/) is the stock Vial keymap of the Corne (`crkbd/rev1`, Pro Micro) with the module and a `HUD_TOG` key on the Adjust layer. It includes the module from `../../hud/`; in your own keymap, copy `hud/` next to `rules.mk` as above.

```sh
# once: link the example into the vial-qmk tree
ln -s "$PWD/firmware/examples/crkbd" ~/vial-qmk/keyboards/crkbd/keymaps/hud_example

cd ~/vial-qmk
qmk compile -kb crkbd/rev1 -km hud_example
```

Generate your own `VIAL_KEYBOARD_UID` for a keymap you publish (`python3 util/vial_generate_keyboard_uid.py` in vial-qmk).

## Testing without the app

`tools/hud_probe.py` speaks the protocol from the command line (with Vial closed):

```sh
pip install hidapi
python3 firmware/tools/hud_probe.py info
python3 firmware/tools/hud_probe.py state --watch
python3 firmware/tools/hud_probe.py keymap --raw
```
