/*
 * Keyboard HUD: firmware side of the HUD protocol. See hud.h and
 * docs/PROTOCOL.md.
 */
#include QMK_KEYBOARD_H
#include <string.h>
#include "hud.h"
#ifdef VIA_ENABLE
#    include "via.h" // raw_hid_receive_kb(), id_unhandled
#endif
#ifdef DYNAMIC_KEYMAP_ENABLE
#    include "dynamic_keymap.h"
#endif
#ifdef SPLIT_KEYBOARD
#    include "split_util.h" // is_transport_connected()
#endif
#ifdef OS_DETECTION_ENABLE
#    include "os_detection.h"
#endif

#if !defined(VIA_ENABLE) || !defined(DYNAMIC_KEYMAP_ENABLE)
#    error "Keyboard HUD needs VIA (or Vial) with a dynamic keymap"
#endif

#if MATRIX_ROWS > 8 || MATRIX_COLS > 8
#    error "Keyboard HUD protocol v1 supports matrices up to 8x8"
#endif

#define HUD_CMD        0xF0
#define HUD_PROTOCOL   1
#define HUD_KEYMAP_MAX 12 // keycodes per GET_KEYMAP response

static uint8_t hud_toggle_count = 0; // +1 on every HUD_TOG press
static uint8_t keymap_version   = 0; // +1 when the effective keymap changes

/* ------------------------------------------------------------------
 * Default hooks
 * ------------------------------------------------------------------ */
__attribute__((weak)) uint16_t hud_toggle_keycode(void) {
    return 0;
}

__attribute__((weak)) bool hud_colemak_on(void) {
    return false;
}

__attribute__((weak)) uint8_t hud_os_mode(void) {
    return 0;
}

__attribute__((weak)) uint8_t hud_active_os(void) {
#ifdef OS_DETECTION_ENABLE
    switch (detected_host_os()) {
        case OS_WINDOWS:
            return 1;
        case OS_MACOS:
        case OS_IOS:
            return 2;
        case OS_LINUX:
            return 3;
        default:
            return 0;
    }
#else
    return 0;
#endif
}

__attribute__((weak)) bool hud_user_flag(void) {
    return false;
}

void hud_toggle_pressed(void) {
    hud_toggle_count++;
}

void hud_keymap_changed(void) {
    keymap_version++;
}

/* ------------------------------------------------------------------
 * Keymap watcher
 * Vial does not tell the keymap when a key changes: the master re-reads
 * the keymap a few keys at a time (8 per cycle, a few ms in all) and
 * hashes it. When the hash changes, the HUD re-reads the legends.
 * ------------------------------------------------------------------ */
#define HUD_SCAN_STEP 8
#define FNV_OFFSET    2166136261UL
#define FNV_PRIME     16777619UL

void hud_task(void) {
    if (!is_keyboard_master()) return;

    static uint16_t pos       = 0;
    static uint32_t hash      = FNV_OFFSET;
    static uint32_t last_hash = 0;
    static bool     first     = true; // the first pass only sets the baseline

    const uint16_t per_layer = MATRIX_ROWS * MATRIX_COLS;
    const uint16_t total     = dynamic_keymap_get_layer_count() * per_layer;

    for (uint8_t i = 0; i < HUD_SCAN_STEP && pos < total; i++, pos++) {
        uint8_t  layer = pos / per_layer;
        uint8_t  rest  = pos % per_layer;
        uint16_t kc    = dynamic_keymap_get_keycode(layer, rest / MATRIX_COLS, rest % MATRIX_COLS);
        hash           = (hash ^ (kc & 0xFF)) * FNV_PRIME;
        hash           = (hash ^ (kc >> 8)) * FNV_PRIME;
    }
    if (pos < total) return;

    if (!first && hash != last_hash) keymap_version++;
    first     = false;
    last_hash = hash;
    pos       = 0;
    hash      = FNV_OFFSET;
}

/* ------------------------------------------------------------------
 * Raw HID commands
 * ------------------------------------------------------------------ */
enum { HUD_GET_INFO = 0x01, HUD_GET_STATE = 0x02, HUD_GET_KEYMAP = 0x03 };
enum { HUD_OK = 0, HUD_ERR_CMD = 1, HUD_ERR_ARG = 2 };

enum {
    HUD_CAP_COLEMAK    = 1 << 0,
    HUD_CAP_CAPS_WORD  = 1 << 1,
    HUD_CAP_OS_DETECT  = 1 << 2,
    HUD_CAP_HUD_TOG    = 1 << 3,
    HUD_CAP_LAYER_LOCK = 1 << 4,
    HUD_CAP_SPLIT      = 1 << 5,
};

enum {
    HUD_FLAG_CAPS_LOCK = 1 << 0,
    HUD_FLAG_CAPS_WORD = 1 << 1,
    HUD_FLAG_COLEMAK   = 1 << 2,
    HUD_FLAG_USER      = 1 << 3,
    HUD_FLAG_NUM_LOCK  = 1 << 4,
    HUD_FLAG_SLAVE_OK  = 1 << 5,
};

static void put16(uint8_t *p, uint16_t v) {
    p[0] = v & 0xFF;
    p[1] = v >> 8;
}

static void put32(uint8_t *p, uint32_t v) {
    put16(p, v & 0xFFFF);
    put16(p + 2, v >> 16);
}

static void hud_get_info(uint8_t *data) {
    uint16_t caps = 0;
#ifdef HUD_HAS_COLEMAK
    caps |= HUD_CAP_COLEMAK;
#endif
#ifdef CAPS_WORD_ENABLE
    caps |= HUD_CAP_CAPS_WORD;
#endif
#ifdef OS_DETECTION_ENABLE
    caps |= HUD_CAP_OS_DETECT;
#endif
#ifdef LAYER_LOCK_ENABLE
    caps |= HUD_CAP_LAYER_LOCK;
#endif
#ifdef SPLIT_KEYBOARD
    caps |= HUD_CAP_SPLIT;
#endif
    const uint16_t toggle = hud_toggle_keycode();
    if (toggle) caps |= HUD_CAP_HUD_TOG;

    data[4] = HUD_PROTOCOL;
    data[5] = MATRIX_ROWS;
    data[6] = MATRIX_COLS;
    data[7] = dynamic_keymap_get_layer_count();
    put16(&data[8], caps);
    put16(&data[10], toggle);
    data[12] = HUD_KEYMAP_MAX;
    data[13] = keymap_version;
}

static void hud_get_state(uint8_t *data) {
    put32(&data[4], (uint32_t)layer_state);
    data[8] = (uint8_t)default_layer_state;
#ifdef LAYER_LOCK_ENABLE
    for (uint8_t l = 0; l < 8; l++) {
        if (is_layer_locked(l)) data[9] |= 1 << l;
    }
#endif
    // on a split keyboard the master also has the rows of the other half
    for (uint8_t r = 0; r < MATRIX_ROWS; r++) data[10 + r] = (uint8_t)matrix_get_row(r);

    data[18] = get_mods();
    data[19] = get_weak_mods();
    data[20] = get_oneshot_mods();
    data[21] = get_oneshot_locked_mods();
    data[22] = get_oneshot_layer_state() ? get_oneshot_layer() : 0xFF;

    const led_t leds  = host_keyboard_led_state();
    uint8_t     flags = 0;
    if (leds.caps_lock) flags |= HUD_FLAG_CAPS_LOCK;
    if (leds.num_lock) flags |= HUD_FLAG_NUM_LOCK;
#ifdef CAPS_WORD_ENABLE
    if (is_caps_word_on()) flags |= HUD_FLAG_CAPS_WORD;
#endif
    if (hud_colemak_on()) flags |= HUD_FLAG_COLEMAK;
    if (hud_user_flag()) flags |= HUD_FLAG_USER;
#ifdef SPLIT_KEYBOARD
    if (is_transport_connected()) flags |= HUD_FLAG_SLAVE_OK;
#endif
    data[23] = flags;

#ifdef OS_DETECTION_ENABLE
    data[24] = detected_host_os();
#endif
    data[25] = hud_os_mode();
    data[26] = hud_active_os();
    data[27] = hud_toggle_count;
    data[28] = keymap_version;
}

// EFFECTIVE keycodes (after any remap in keymap_key_to_keycode()),
// in row x columns + column order
static void hud_get_keymap(uint8_t *data, const uint8_t *args) {
    const uint8_t  layer  = args[0];
    const uint16_t offset = args[1] | (args[2] << 8);
    const uint8_t  count  = args[3];

    memcpy(&data[4], args, 4); // echo of the request
    if (layer >= dynamic_keymap_get_layer_count() || count == 0 || count > HUD_KEYMAP_MAX || offset + count > MATRIX_ROWS * MATRIX_COLS) {
        data[2] = HUD_ERR_ARG;
        return;
    }
    for (uint8_t i = 0; i < count; i++) {
        uint16_t pos = offset + i;
        uint16_t kc  = keymap_key_to_keycode(layer, (keypos_t){.row = pos / MATRIX_COLS, .col = pos % MATRIX_COLS});
        put16(&data[8 + 2 * i], kc);
    }
}

// Called by VIA for the commands it does not know. VIA then sends the
// same buffer back: the answer goes in "data" (no raw_hid_send()).
void raw_hid_receive_kb(uint8_t *data, uint8_t length) {
    if (data[0] != HUD_CMD || length < 32) {
        data[0] = id_unhandled; // as the default implementation
        return;
    }
    // data[1] = subcommand and data[3] = the HUD's sequence stay as they are
    uint8_t args[4];
    memcpy(args, &data[4], sizeof(args));
    memset(&data[4], 0, length - 4);
    data[2] = HUD_OK;

    switch (data[1]) {
        case HUD_GET_INFO:   hud_get_info(data); break;
        case HUD_GET_STATE:  hud_get_state(data); break;
        case HUD_GET_KEYMAP: hud_get_keymap(data, args); break;
        default:             data[2] = HUD_ERR_CMD; break;
    }
}
