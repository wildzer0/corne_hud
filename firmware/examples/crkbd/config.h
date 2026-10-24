// Keyboard HUD example: the stock Vial keymap of the Corne (crkbd/rev1)
// with the HUD module. SPDX-License-Identifier: GPL-2.0-or-later
#pragma once

// Generate your own UID with vial-qmk's util/vial_generate_keyboard_uid.py
#define VIAL_KEYBOARD_UID {0xD0, 0xDB, 0xED, 0xAA, 0xBD, 0xE0, 0x81, 0xCF}
#define VIAL_UNLOCK_COMBO_ROWS {0, 0}
#define VIAL_UNLOCK_COMBO_COLS {0, 1}

#undef DYNAMIC_KEYMAP_LAYER_COUNT
#define DYNAMIC_KEYMAP_LAYER_COUNT 4
#define TAPPING_TERM 180

#define MASTER_LEFT
#define USE_SERIAL_PD2
