#pragma once

// ------------------------------------------------------------
// Identita' Vial: stesso UID del firmware Vial ufficiale della
// Corne v4 (foostan), cosi' Vial la riconosce come prima.
// ------------------------------------------------------------
#define VIAL_KEYBOARD_UID {0x89, 0x36, 0x2A, 0xC7, 0xFA, 0xD8, 0x89, 0x45}

// Combo di sblocco Vial (per funzioni "sensibili" come il bootloader
// da Vial): tieni premuti insieme i due tasti in alto a sinistra.
#define VIAL_UNLOCK_COMBO_ROWS {0, 0}
#define VIAL_UNLOCK_COMBO_COLS {0, 1}

// 6 layer: base, simboli, nav, sistema, debug, accentate
#undef DYNAMIC_KEYMAP_LAYER_COUNT
#define DYNAMIC_KEYMAP_LAYER_COUNT 6

// ------------------------------------------------------------
// Split: condivide con l'altra meta' il layer attivo e lo stato
// di Caps Lock, cosi' i colori compaiono su entrambe le meta'.
// ------------------------------------------------------------
#define SPLIT_LAYER_STATE_ENABLE
#define SPLIT_LED_STATE_ENABLE
// Comunica all'altra meta' anche i tasti premuti: serve agli effetti
// RGB reattivi (es. "Funzioni") per illuminarsi su entrambe le meta'.
#define SPLIT_TRANSPORT_MIRROR
// Canale per inviare all'altra meta' lo stato di Colemak e Caps Word
#define SPLIT_TRANSACTION_IDS_USER USER_SYNC_STATE

// ------------------------------------------------------------
// Mod-tap (Tab = tocco / Ctrl = tenuto)
// ------------------------------------------------------------
// Valore iniziale del Tapping Term. Con Vial le opzioni Tap-Hold
// (Tapping Term, Permissive Hold, Retro Tapping) si gestiscono da
// Vial -> QMK Settings -> Tap-Hold e restano salvate in memoria.
#define TAPPING_TERM 200
// NB: NON attivare CHORDAL_HOLD o FLOW_TAP: con Ctrl sulla mano
// sinistra romperebbero Ctrl+C/V/S/Z.

// ------------------------------------------------------------
// Caps Word
// ------------------------------------------------------------
// Premere i due Shift insieme attiva Caps Word.
#define BOTH_SHIFTS_TURNS_ON_CAPS_WORD
// Si spegne da solo dopo 5 secondi di inattivita'.
#define CAPS_WORD_IDLE_TIMEOUT 5000

// ------------------------------------------------------------
// RGB
// ------------------------------------------------------------
// Spegne i LED dopo 10 minuti senza usare la tastiera (in ms);
// si riaccendono al primo tasto.
#define RGB_MATRIX_TIMEOUT 600000
// Luminosita' massima (0-255). Di fabbrica la Corne v4.1 e' a 50.
// Non superare ~120: le due meta' sono alimentate dallo stesso cavo.
#undef RGB_MATRIX_MAXIMUM_BRIGHTNESS
#define RGB_MATRIX_MAXIMUM_BRIGHTNESS 120
// Effetto predefinito: "Funzioni" (usato quando la memoria viene azzerata)
#define RGB_MATRIX_DEFAULT_MODE RGB_MATRIX_CUSTOM_funzioni
// Tiene traccia dei tasti premuti per gli effetti reattivi
#ifndef RGB_MATRIX_KEYPRESSES
#    define RGB_MATRIX_KEYPRESSES
#endif

// Il numero di combo, tap dance e key override configurabili da Vial
// viene calcolato in automatico in base alla memoria disponibile.

// Keyboard HUD: questa keymap ha il remap Colemak (hud_colemak_on)
#define HUD_HAS_COLEMAK
