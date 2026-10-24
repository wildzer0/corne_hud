/*
 * Keyboard HUD: firmware side of the HUD protocol (Raw HID).
 *
 * The app polls the keyboard with command 0xF0 on the VIA/Vial Raw HID
 * interface; this module answers with layers, pressed keys, modifiers,
 * Caps Word and the host OS. It only answers: it never sends reports on
 * its own (so it does not disturb Vial) and never writes to EEPROM.
 * Wire format: docs/PROTOCOL.md.
 *
 * Integration (see firmware/README.md):
 *   1. rules.mk:  include $(dir $(lastword $(MAKEFILE_LIST)))hud/hud.mk
 *   2. keymap.c:  #include "hud.h"
 *   3. housekeeping_task_user():  hud_task();
 *   4. optional: a HUD_TOG custom keycode, see hud_toggle_keycode().
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>

/* Call from housekeeping_task_user(). Does nothing on the split slave. */
void hud_task(void);

/* Call when the HUD_TOG key is pressed (key down only). */
void hud_toggle_pressed(void);

/* Call when the effective keymap changes without Vial noticing (e.g. a
 * remap in keymap_key_to_keycode() switched on or off): the HUD re-reads
 * the legends. Changes made with Vial are detected on their own. */
void hud_keymap_changed(void);

/*
 * Optional hooks: define them in the keymap to override the defaults.
 */

/* Keycode of the HUD_TOG custom key, or 0 if there is none (default).
 * With a HUD_TOG key the HUD can be shown/hidden from the keyboard and
 * click-through can be enabled. */
uint16_t hud_toggle_keycode(void);

/* Return true while the keymap applies Colemak (default: false). Define
 * HUD_HAS_COLEMAK in config.h so that the HUD shows the QWERTY/Colemak
 * indicator. */
bool hud_colemak_on(void);

/* OS mode chosen by the user: 0 auto, 1 Windows, 2 macOS, 3 Linux
 * (default: 0). */
uint8_t hud_os_mode(void);

/* OS the keymap acts on: 1 Windows, 2 macOS, 3 Linux, 0 unknown
 * (default: from QMK OS detection, if enabled). */
uint8_t hud_active_os(void);

/* A free status bit for the keymap (flag b3 of GET_STATE, default: false). */
bool hud_user_flag(void);
