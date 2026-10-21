/*
 * Corne v4.1 standard (46 tasti) - keymap "mia"
 *
 * Indice del file
 *   1. Layer e tasti personalizzati
 *   2. PALETTE: tutti i colori in un unico posto
 *   3. Sistema operativo (Windows / Linux / macOS)
 *   4. Accentate
 *   5. Tasti "intelligenti"
 *   6. Colemak-DH
 *   7. Sincronizzazione tra le due meta'
 *   8. LED: indicatori, animazione del cambio layout
 *   9. Gestione dei tasti e ciclo principale
 *  10. Layout
 *
 * Il layout in fondo e' quello di PARTENZA: dopo il primo flash i tasti
 * si cambiano da Vial senza ricompilare.
 */

#include QMK_KEYBOARD_H
#include "os_detection.h"
#include "keymap_introspection.h" // keycode_at_keymap_location() e simili
#include "transactions.h"         // comunicazione tra le due meta'
#include <string.h>
#ifdef VIAL_ENABLE
#    include "vial.h"
#endif
#ifdef RGB_MATRIX_ENABLE
#    include <lib/lib8tion/lib8tion.h> // scale8()
#endif

/* ==================================================================
 * 1. LAYER E TASTI PERSONALIZZATI
 * ================================================================== */
enum layers {
    _BASE,  // lettere (QWERTY o Colemak-DH)
    _SYM,   // simboli e numeri          (tieni Fn1)
    _NAV,   // navigazione, mod, appunti (tieni Fn2)
    _SYS,   // tasti F, media, RGB       (Fn1 + Fn2)
    _DEBUG, // debug VS Code, bloccabile (angolo alto sx)
    _ACC,   // accentate, one-shot       (tasto interno dx, poi la vocale)
};

// Compaiono in Vial nella tab "User". NON cambiare l'ordine: Vial li
// identifica per posizione. I nuovi tasti vanno aggiunti IN FONDO.
enum custom_keycodes {
    ACC_A = QK_KB_0, // à
    ACC_EG,          // è
    ACC_EA,          // é
    ACC_I,           // ì
    ACC_O,           // ò
    ACC_U,           // ù
    OS_AUTO,         // sistema operativo: rilevamento automatico
    OS_WIN,          // forza Windows
    OS_MAC,          // forza macOS
    OS_LNX,          // forza Linux
    SM_WORDL,        // parola precedente
    SM_WORDR,        // parola successiva
    SM_HOME,         // inizio riga
    SM_END,          // fine riga
    SM_REDO,         // ripeti azione
    SM_RUN,          // VS Code: esegui senza debug
    CMK_TOG,         // alterna QWERTY / Colemak-DH
    RGB_FUNZ,        // torna all'effetto RGB "Funzioni"
};

/* ==================================================================
 * 2. PALETTE
 *
 * Ogni colore e' {tonalita', saturazione}:
 *   tonalita' 0-255:   0 rosso   21 arancione   43 giallo   85 verde
 *                    128 azzurro 170 blu        191 viola  213 magenta
 *                    234 rosa
 *   saturazione 0-255: 255 colore pieno, ~120 pastello, 0 bianco
 * La luminosita' e' sempre quella scelta in Vial.
 * ================================================================== */
typedef struct {
    uint8_t hue;
    uint8_t sat;
} color_t;

// Colore di tutta la tastiera mentre il layer e' attivo.
// enabled = false: il layer non cambia i colori (resta l'effetto).
static const struct {
    bool    enabled;
    color_t color;
} layer_colors[] = {
    [_BASE]  = {false, {0, 0}},
    [_SYM]   = {true,  {128, 255}}, // azzurro
    [_NAV]   = {true,  {85, 255}},  // verde
    [_SYS]   = {false, {0, 0}},     // nessuno: cosi' vedi l'effetto mentre lo cambi
    [_DEBUG] = {true,  {0, 255}},   // rosso
    [_ACC]   = {true,  {43, 255}},  // giallo
};

// Spie sui due tasti Shift
static const color_t COLOR_CAPS_LOCK = {0, 0};     // bianco
static const color_t COLOR_CAPS_WORD = {170, 255}; // blu

// Indicatore fisso del layout sui tasti degli indici (layer base)
static const color_t COLOR_IND_QWERTY  = {0, 0};     // bianco
static const color_t COLOR_IND_COLEMAK = {191, 255}; // viola

// Animazione quando cambi layout
static const color_t COLOR_ANIM_QWERTY  = {128, 255}; // azzurro
static const color_t COLOR_ANIM_COLEMAK = {21, 255};  // arancione

// Luminosita' minima dei colori (sotto i LED perdono il colore)
#define MIN_BRIGHTNESS 40

/* ==================================================================
 * 3. SISTEMA OPERATIVO
 * Riconosciuto al collegamento. Su macOS Ctrl e Cmd si scambiano da
 * soli. Se il rilevamento sbaglia, forza il sistema dal layer 3
 * (la scelta forzata si azzera scollegando la tastiera).
 * ================================================================== */
enum { MODE_AUTO, MODE_WIN, MODE_MAC, MODE_LNX };
static uint8_t os_mode = MODE_AUTO;

static uint8_t active_os(void) {
    if (os_mode != MODE_AUTO) return os_mode;
    switch (detected_host_os()) {
        case OS_MACOS:
        case OS_IOS:
            return MODE_MAC;
        case OS_LINUX:
            return MODE_LNX;
        default:
            return MODE_WIN;
    }
}

static void apply_os_settings(void) {
    bool mac = (active_os() == MODE_MAC);
    if (keymap_config.swap_lctl_lgui != mac || keymap_config.swap_rctl_rgui != mac) {
        clear_keyboard(); // evita modificatori incastrati durante il cambio
        keymap_config.swap_lctl_lgui = mac; // solo in RAM
        keymap_config.swap_rctl_rgui = mac;
    }
}

bool process_detected_host_os_user(os_variant_t os) {
    (void)os;
    apply_os_settings();
    return true;
}

/* ==================================================================
 * 4. ACCENTATE (senza installare layout)
 *   Windows: Alt+codice sul tastierino   macOS: Option+accento
 *   Linux:   Ctrl+Shift+U + codice esadecimale (GNOME/IBus)
 * ================================================================== */
static const struct {
    uint8_t lower, upper; // codici Latin-1 di minuscola e maiuscola
    uint8_t mac_dead;     // macOS: tasto da premere con Option
    uint8_t mac_letter;   // macOS: vocale da premere dopo
} accents[] = {
    {0xE0, 0xC0, KC_GRV, KC_A}, // à À
    {0xE8, 0xC8, KC_GRV, KC_E}, // è È
    {0xE9, 0xC9, KC_E,   KC_E}, // é É
    {0xEC, 0xCC, KC_GRV, KC_I}, // ì Ì
    {0xF2, 0xD2, KC_GRV, KC_O}, // ò Ò
    {0xF9, 0xD9, KC_GRV, KC_U}, // ù Ù
};

static const uint16_t keypad_digits[10] = {
    KC_KP_0, KC_KP_1, KC_KP_2, KC_KP_3, KC_KP_4, KC_KP_5, KC_KP_6, KC_KP_7, KC_KP_8, KC_KP_9,
};

static void send_windows_alt_code(uint8_t code) {
    bool numlock_off = !host_keyboard_led_state().num_lock; // serve Bloc Num
    if (numlock_off) {
        tap_code(KC_NUM_LOCK);
        wait_ms(10);
    }
    register_code(KC_LALT);
    tap_code(KC_KP_0);
    tap_code(keypad_digits[code / 100]);
    tap_code(keypad_digits[(code / 10) % 10]);
    tap_code(keypad_digits[code % 10]);
    unregister_code(KC_LALT);
    if (numlock_off) tap_code(KC_NUM_LOCK);
}

static void tap_hex_digit(uint8_t n) {
    if (n == 0) {
        tap_code(KC_0);
    } else if (n < 10) {
        tap_code(KC_1 + n - 1);
    } else {
        tap_code(KC_A + n - 10);
    }
}

static void send_accent(uint8_t index) {
    const uint8_t mods  = get_mods();
    const bool    upper = ((mods | get_oneshot_mods()) & MOD_MASK_SHIFT) || is_caps_word_on() || host_keyboard_led_state().caps_lock;
    const uint8_t code  = upper ? accents[index].upper : accents[index].lower;

    clear_mods(); // i modificatori disturberebbero la sequenza
    clear_weak_mods();
    clear_oneshot_mods();
    send_keyboard_report();

    switch (active_os()) {
        case MODE_MAC:
            tap_code16(LALT(accents[index].mac_dead));
            if (upper) {
                tap_code16(LSFT(accents[index].mac_letter));
            } else {
                tap_code(accents[index].mac_letter);
            }
            break;
        case MODE_LNX:
            tap_code16(LCTL(LSFT(KC_U)));
            tap_hex_digit(code >> 4);
            tap_hex_digit(code & 0x0F);
            tap_code(KC_SPC);
            break;
        default:
            send_windows_alt_code(code);
            break;
    }

    set_mods(mods);
    send_keyboard_report();
}

/* ==================================================================
 * 5. TASTI "INTELLIGENTI"
 * Stessa funzione, combinazione giusta per ogni sistema. Si ripetono
 * se tenuti premuti e funzionano con Shift per selezionare.
 * ================================================================== */
static uint8_t smart_mods = 0;
static uint8_t smart_key  = KC_NO;

static void smart_release(void) {
    if (smart_key == KC_NO) return;
    unregister_code(smart_key);
    unregister_mods(smart_mods);
    smart_key  = KC_NO;
    smart_mods = 0;
}

static void smart_press(uint8_t mods, uint8_t key) {
    smart_release();
    smart_mods = mods & ~get_mods(); // non staccare al rilascio i mod che tieni tu
    smart_key  = key;
    register_mods(smart_mods);
    register_code(key);
}

static void handle_smart_key(uint16_t keycode) {
    const uint8_t os  = active_os();
    const bool    mac = (os == MODE_MAC);
    const uint8_t CTL = MOD_BIT(KC_LCTL), ALT = MOD_BIT(KC_LALT);
    const uint8_t GUI = MOD_BIT(KC_LGUI), SFT = MOD_BIT(KC_LSFT);

    switch (keycode) {
        case SM_WORDL: smart_press(mac ? ALT : CTL, KC_LEFT); break;
        case SM_WORDR: smart_press(mac ? ALT : CTL, KC_RGHT); break;
        case SM_HOME:  mac ? smart_press(GUI, KC_LEFT) : smart_press(0, KC_HOME); break;
        case SM_END:   mac ? smart_press(GUI, KC_RGHT) : smart_press(0, KC_END); break;
        case SM_REDO:
            if (mac)                 smart_press(GUI | SFT, KC_Z);
            else if (os == MODE_LNX) smart_press(CTL | SFT, KC_Z);
            else                     smart_press(CTL, KC_Y);
            break;
        case SM_RUN: smart_press(CTL, KC_F5); break; // Ctrl+F5 anche su macOS
    }
}

/* ==================================================================
 * 6. COLEMAK-DH
 * Nessun layer in piu': con Colemak attivo, le lettere del layer BASE
 * e le vocali del layer ACCENTI vengono lette da un'altra posizione.
 * ================================================================== */
static bool colemak_on = false; // salvato in memoria (bit 0 della config utente)

typedef struct {
    uint8_t row, col;
} pos_t;

// Posizione fisica -> posizione QWERTY da cui leggere il tasto
static const struct {
    pos_t at, from;
} colemak_dh[] = {
    {{0, 3}, {1, 4}}, // E -> F
    {{0, 4}, {4, 1}}, // R -> P
    {{0, 5}, {2, 5}}, // T -> B
    {{4, 5}, {5, 4}}, // Y -> J
    {{4, 4}, {5, 2}}, // U -> L
    {{4, 3}, {4, 4}}, // I -> U
    {{4, 2}, {4, 5}}, // O -> Y
    {{4, 1}, {5, 1}}, // P -> ;
    {{1, 2}, {0, 4}}, // S -> R
    {{1, 3}, {1, 2}}, // D -> S
    {{1, 4}, {0, 5}}, // F -> T
    {{5, 5}, {6, 4}}, // H -> M
    {{5, 4}, {6, 5}}, // J -> N
    {{5, 3}, {0, 3}}, // K -> E
    {{5, 2}, {4, 3}}, // L -> I
    {{5, 1}, {4, 2}}, // ; -> O
    {{2, 4}, {1, 3}}, // V -> D
    {{2, 5}, {2, 4}}, // B -> V
    {{6, 5}, {5, 3}}, // N -> K
    {{6, 4}, {5, 5}}, // M -> H
};

// Posizione di ogni lettera sul layout QWERTY (indice 0 = A ... 25 = Z)
static const pos_t qwerty_pos[26] = {
    {1, 1}, // A
    {2, 5}, // B
    {2, 3}, // C
    {1, 3}, // D
    {0, 3}, // E
    {1, 4}, // F
    {1, 5}, // G
    {5, 5}, // H
    {4, 3}, // I
    {5, 4}, // J
    {5, 3}, // K
    {5, 2}, // L
    {6, 4}, // M
    {6, 5}, // N
    {4, 2}, // O
    {4, 1}, // P
    {0, 1}, // Q
    {0, 4}, // R
    {1, 2}, // S
    {0, 5}, // T
    {4, 4}, // U
    {2, 4}, // V
    {0, 2}, // W
    {2, 2}, // X
    {4, 5}, // Y
    {2, 1}, // Z
};

// Sostituisce la funzione di QMK che traduce "tasto fisico" -> keycode.
// Identica all'originale, salvo il remap Colemak.
uint16_t keymap_key_to_keycode(uint8_t layer, keypos_t key) {
#ifdef VIAL_ENABLE
    if (vial_unlock_in_progress) return KC_NO;
    if (key.row == VIAL_MATRIX_MAGIC && key.col == VIAL_MATRIX_MAGIC) return g_vial_magic_keycode_override;
#endif
    if (key.row < MATRIX_ROWS && key.col < MATRIX_COLS) {
        uint8_t row = key.row, col = key.col;
        if (colemak_on && (layer == _BASE || layer == _ACC)) {
            for (uint8_t i = 0; i < ARRAY_SIZE(colemak_dh); i++) {
                if (colemak_dh[i].at.row == row && colemak_dh[i].at.col == col) {
                    row = colemak_dh[i].from.row;
                    col = colemak_dh[i].from.col;
                    break;
                }
            }
        }
        return keycode_at_keymap_location(layer, row, col);
    }
#ifdef ENCODER_MAP_ENABLE
    if (key.row == KEYLOC_ENCODER_CW && key.col < NUM_ENCODERS) return keycode_at_encodermap_location(layer, key.col, true);
    if (key.row == KEYLOC_ENCODER_CCW && key.col < NUM_ENCODERS) return keycode_at_encodermap_location(layer, key.col, false);
#endif
    return KC_NO;
}

/* ==================================================================
 * 7. SINCRONIZZAZIONE TRA LE META'
 * La meta' collegata al cavo ("master") decide tutto e invia all'altra
 * cio' che serve ai LED. L'orologio dell'animazione e' gia'
 * sincronizzato da QMK tra le due meta'.
 * ================================================================== */
// Animazione del cambio layout: la parola si "scrive" da sola sui
// tasti QWERTY, una lettera alla volta.
#define ANIM_WORD_QWERTY  "QWERTY"
#define ANIM_WORD_COLEMAK "COLEMAK"
#define ANIM_STEP_MS    180 // ritardo tra una lettera e la successiva
#define ANIM_FADEIN_MS  200 // tempo di accensione di ogni lettera
#define ANIM_HOLD_MS    700 // parola completa accesa, prima di spegnersi
#define ANIM_FADEOUT_MS 500 // spegnimento finale

static const char *anim_word(bool colemak) {
    return colemak ? ANIM_WORD_COLEMAK : ANIM_WORD_QWERTY;
}

static uint32_t anim_total_ms(bool colemak) {
    uint8_t len = strlen(anim_word(colemak));
    return (uint32_t)(len - 1) * ANIM_STEP_MS + ANIM_FADEIN_MS + ANIM_HOLD_MS + ANIM_FADEOUT_MS;
}

typedef struct {
    bool     colemak;
    bool     caps_lock;
    bool     caps_word;
    bool     borrowed;   // LED accesi "in prestito" (RGB spento in Vial)
    bool     anim;       // animazione del cambio layout in corso
    uint32_t anim_start; // inizio animazione (orologio sincronizzato)
} shared_state_t;

static shared_state_t local_state  = {0}; // stato deciso dal master
static shared_state_t remote_state = {0}; // copia ricevuta dallo slave

static const shared_state_t *state(void) {
    return is_keyboard_master() ? &local_state : &remote_state;
}

static void receive_state(uint8_t in_len, const void *in_data, uint8_t out_len, void *out_data) {
    (void)out_len;
    (void)out_data;
    if (in_len == sizeof(shared_state_t)) memcpy(&remote_state, in_data, sizeof(shared_state_t));
}

static bool anim_running(void) {
    const shared_state_t *s = state();
    return s->anim && sync_timer_elapsed32(s->anim_start) < anim_total_ms(s->colemak);
}

static void sync_to_slave(void) {
    static shared_state_t last_sent    = {0};
    static uint32_t       last_ok      = 0;
    static uint32_t       last_attempt = 0;

    bool changed = memcmp(&local_state, &last_sent, sizeof(local_state)) != 0;
    // invia quando cambia qualcosa (al massimo ogni 20 ms) e comunque ogni 500 ms
    if ((changed && timer_elapsed32(last_attempt) > 20) || timer_elapsed32(last_ok) > 500) {
        last_attempt = timer_read32();
        if (transaction_rpc_send(USER_SYNC_STATE, sizeof(local_state), &local_state)) {
            last_sent = local_state;
            last_ok   = timer_read32();
        }
    }
}

/* ==================================================================
 * 8. LED
 *
 * Priorita' (dalla piu' alta):
 *   a. animazione del cambio layout: solo le lettere, il resto spento
 *   b. colore del layer attivo (se il layer ne ha uno)
 *   c. effetto RGB scelto, piu' gli indicatori:
 *      - layout sui tasti degli indici (bianco QWERTY / viola Colemak)
 *      - Caps Lock / Caps Word sui tasti Shift
 *
 * LED "in prestito": se in Vial l'RGB e' spento ma serve mostrare
 * qualcosa, la tastiera accende i LED senza salvare nulla e lascia
 * neri tutti i tasti tranne gli indicatori.
 * ================================================================== */
#ifdef RGB_MATRIX_ENABLE

static const pos_t KEY_INDEX_L = {1, 4}, KEY_INDEX_R = {5, 4}; // F e J fisici
static const pos_t KEY_SHIFT_L = {2, 0}, KEY_SHIFT_R = {6, 0};

static bool layer_has_color(uint8_t layer) {
    return layer < ARRAY_SIZE(layer_colors) && layer_colors[layer].enabled;
}

static rgb_t to_rgb(color_t c, uint8_t level) {
    uint8_t v = rgb_matrix_get_val();
    if (v < MIN_BRIGHTNESS) v = MIN_BRIGHTNESS;
    hsv_t hsv = {c.hue, c.sat, scale8(v, level)};
    return hsv_to_rgb(hsv);
}

static void fill(rgb_t rgb, uint8_t led_min, uint8_t led_max) {
    for (uint8_t i = led_min; i < led_max; i++) rgb_matrix_set_color(i, rgb.r, rgb.g, rgb.b);
}

// Colora il LED del tasto in posizione p, se e' su questa meta'
static void set_key(pos_t p, rgb_t rgb, uint8_t led_min, uint8_t led_max) {
    uint8_t led = g_led_config.matrix_co[p.row][p.col];
    if (led != NO_LED && led >= led_min && led < led_max) rgb_matrix_set_color(led, rgb.r, rgb.g, rgb.b);
}

// La parola del nuovo layout ("QWERTY" o "COLEMAK") si accende una
// lettera alla volta, con dissolvenza, sui tasti del layout QWERTY.
// Tutti gli altri tasti restano spenti.
static void draw_layout_animation(const shared_state_t *s, uint8_t led_min, uint8_t led_max) {
    const uint32_t t     = sync_timer_elapsed32(s->anim_start);
    const uint32_t total = anim_total_ms(s->colemak);
    const char    *word  = anim_word(s->colemak);
    const color_t  color = s->colemak ? COLOR_ANIM_COLEMAK : COLOR_ANIM_QWERTY;

    fill((rgb_t){0, 0, 0}, led_min, led_max);

    uint8_t fade_out = 255; // spegnimento finale, uguale per tutte le lettere
    if (t > total - ANIM_FADEOUT_MS) {
        fade_out = 255 - (uint8_t)((t - (total - ANIM_FADEOUT_MS)) * 255 / ANIM_FADEOUT_MS);
    }

    for (uint8_t i = 0; word[i] != '\0'; i++) {
        uint32_t start = (uint32_t)i * ANIM_STEP_MS;
        if (t < start) break; // le lettere successive non sono ancora partite
        uint32_t since = t - start;
        uint8_t  level = (since >= ANIM_FADEIN_MS) ? 255 : (uint8_t)(since * 255 / ANIM_FADEIN_MS);
        set_key(qwerty_pos[word[i] - 'A'], to_rgb(color, scale8(level, fade_out)), led_min, led_max);
    }
}

bool rgb_matrix_indicators_advanced_user(uint8_t led_min, uint8_t led_max) {
    const shared_state_t *s = state();

    // a. animazione del cambio layout
    if (anim_running()) {
        draw_layout_animation(s, led_min, led_max);
        return false;
    }

    // b. colore del layer
    uint8_t layer = get_highest_layer(layer_state);
    if (layer_has_color(layer)) {
        fill(to_rgb(layer_colors[layer].color, 255), led_min, led_max);
        return false;
    }

    // c. effetto + indicatori
    if (s->borrowed) fill((rgb_t){0, 0, 0}, led_min, led_max);

    if (s->colemak || !s->borrowed) {
        rgb_t ind = to_rgb(s->colemak ? COLOR_IND_COLEMAK : COLOR_IND_QWERTY, 255);
        set_key(KEY_INDEX_L, ind, led_min, led_max);
        set_key(KEY_INDEX_R, ind, led_min, led_max);
    }

    if (s->caps_lock || s->caps_word) {
        rgb_t caps = to_rgb(s->caps_word ? COLOR_CAPS_WORD : COLOR_CAPS_LOCK, 255);
        set_key(KEY_SHIFT_L, caps, led_min, led_max);
        set_key(KEY_SHIFT_R, caps, led_min, led_max);
    }
    return false;
}

// Solo master: accende/spegne i LED "in prestito" quando serve
static void manage_borrowed_leds(void) {
    const shared_state_t *s = &local_state;
    bool need = anim_running() || layer_has_color(get_highest_layer(layer_state)) || s->caps_lock || s->caps_word || s->colemak;

    if (need && !rgb_matrix_is_enabled()) {
        rgb_matrix_enable_noeeprom();
        local_state.borrowed = true;
    } else if (!need && local_state.borrowed) {
        rgb_matrix_disable_noeeprom();
        local_state.borrowed = false;
    }
}

// Tasto RGB acceso/spento: decide in base a cio' che vedi
static void rgb_toggle_pressed(void) {
    if (local_state.borrowed || !rgb_matrix_is_enabled()) {
        local_state.borrowed = false;
        rgb_matrix_enable(); // acceso per scelta tua (salvato)
    } else {
        rgb_matrix_disable(); // spento (salvato); gli indicatori restano
    }
}

static void rgb_funzioni_pressed(void) {
    local_state.borrowed = false;
    rgb_matrix_enable();
    rgb_matrix_mode(RGB_MATRIX_CUSTOM_funzioni);
}

#else
static void manage_borrowed_leds(void) {}
static void rgb_toggle_pressed(void) {}
static void rgb_funzioni_pressed(void) {}
#endif // RGB_MATRIX_ENABLE

/* ==================================================================
 * 9. GESTIONE DEI TASTI E CICLO PRINCIPALE
 * ================================================================== */
bool process_record_user(uint16_t keycode, keyrecord_t *record) {
    const bool pressed = record->event.pressed;

    switch (keycode) {
        case QK_UNDERGLOW_TOGGLE:
        case QK_RGB_MATRIX_TOGGLE:
            if (pressed) rgb_toggle_pressed();
            return false;

        case RGB_FUNZ:
            if (pressed) rgb_funzioni_pressed();
            return false;

        case ACC_A ... ACC_U:
            if (pressed) send_accent(keycode - ACC_A);
            return false;

        case OS_AUTO ... OS_LNX:
            if (pressed) {
                os_mode = MODE_AUTO + (keycode - OS_AUTO);
                apply_os_settings();
            }
            return false;

        case SM_WORDL ... SM_RUN:
            if (pressed) {
                handle_smart_key(keycode);
            } else {
                smart_release();
            }
            return false;

        case CMK_TOG:
            if (pressed) {
                colemak_on   = !colemak_on;
                uint32_t cfg = eeconfig_read_user();
                eeconfig_update_user(colemak_on ? (cfg | 1) : (cfg & ~1UL));
                local_state.anim       = true; // parte l'animazione
                local_state.anim_start = sync_timer_read32();
            }
            return false;
    }
    return true;
}

// Caps Word: come lo standard, ma le accentate non lo interrompono
bool caps_word_press_user(uint16_t keycode) {
    switch (keycode) {
        case KC_A ... KC_Z:
        case KC_MINS:
            add_weak_mods(MOD_BIT(KC_LSFT));
            return true;
        case KC_1 ... KC_0:
        case KC_BSPC:
        case KC_DEL:
        case KC_UNDS:
        case ACC_A ... ACC_U:
            return true;
        default:
            return false;
    }
}

void keyboard_post_init_user(void) {
    colemak_on = eeconfig_read_user() & 1; // ultimo layout scelto
    transaction_register_rpc(USER_SYNC_STATE, receive_state);
}

// Eseguita di continuo: qui il master aggiorna lo stato e lo invia
void housekeeping_task_user(void) {
    if (!is_keyboard_master()) return;

    local_state.colemak   = colemak_on;
    local_state.caps_lock = host_keyboard_led_state().caps_lock;
    local_state.caps_word = is_caps_word_on();
    if (local_state.anim && !anim_running()) local_state.anim = false;

    manage_borrowed_leds();
    sync_to_slave();
}

/* ==================================================================
 * 10. LAYOUT
 * _______ = trasparente (usa il tasto del layer sotto)
 * XXXXXXX = nessuna funzione
 * ================================================================== */
const uint16_t PROGMEM keymaps[][MATRIX_ROWS][MATRIX_COLS] = {

  /* 0 BASE
   * Angolo alto sx = entra/esce dal layer DEBUG
   * Colonna interna: Esc | LGui      RAlt | Accenti (one-shot)
   * Pollici: LAlt Fn1 Spazio         Invio Fn2 Repeat
   */
  [_BASE] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
     TG(_DEBUG),   KC_Q,    KC_W,    KC_E,    KC_R,    KC_T,  KC_ESC,    KC_RALT,    KC_Y,    KC_U,    KC_I,    KC_O,    KC_P, KC_BSPC,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
  LCTL_T(KC_TAB),  KC_A,    KC_S,    KC_D,    KC_F,    KC_G, KC_LGUI,  OSL(_ACC),    KC_H,    KC_J,    KC_K,    KC_L, KC_SCLN, KC_QUOT,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       KC_LSFT,    KC_Z,    KC_X,    KC_C,    KC_V,    KC_B,                         KC_N,    KC_M, KC_COMM,  KC_DOT, KC_SLSH, KC_RSFT,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           KC_LALT, TL_LOWR,  KC_SPC,     KC_ENT, TL_UPPR,  QK_REP
                                       //`--------------------------'  `--------------------------'
  ),

  /* 1 SYM: simboli a sinistra, tastierino a destra */
  [_SYM] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
        KC_GRV, KC_EXLM,   KC_AT, KC_HASH,  KC_DLR, KC_PERC, KC_BSLS,    XXXXXXX, KC_ASTR,    KC_7,    KC_8,    KC_9, KC_PLUS, _______,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
       _______, KC_LCBR, KC_RCBR, KC_LPRN, KC_RPRN,  KC_EQL, KC_PIPE,    XXXXXXX, KC_MINS,    KC_4,    KC_5,    KC_6, KC_UNDS, _______,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       _______, KC_LBRC, KC_RBRC, KC_CIRC, KC_AMPR, KC_TILD,                      KC_COMM,    KC_1,    KC_2,    KC_3,  KC_DOT, _______,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           _______, _______, XXXXXXX,    _______, _______,    KC_0
                                       //`--------------------------'  `--------------------------'
  ),

  /* 2 NAV
   * sx: Gui Alt Ctrl Shift su A S D F, Caps Word su G,
   *     Undo Taglia Copia Incolla Redo su Z X C V B
   * dx: frecce su HJKL, inizio/fine riga e pagina su/giu' sopra,
   *     parola precedente/successiva su N M
   */
  [_NAV] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
       _______, XXXXXXX, XXXXXXX, XXXXXXX, XXXXXXX, XXXXXXX, XXXXXXX,    _______, SM_HOME, KC_PGDN, KC_PGUP,  SM_END, XXXXXXX, _______,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
       _______, KC_LGUI, KC_LALT, KC_LCTL, KC_LSFT, CW_TOGG, XXXXXXX,    _______, KC_LEFT, KC_DOWN,   KC_UP, KC_RGHT, XXXXXXX, XXXXXXX,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       _______, C(KC_Z), C(KC_X), C(KC_C), C(KC_V), SM_REDO,                     SM_WORDL, SM_WORDR, XXXXXXX, XXXXXXX, XXXXXXX, _______,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           _______, _______, XXXXXXX,    _______, _______, _______
                                       //`--------------------------'  `--------------------------'
  ),

  /* 3 SYS: tasti F, media, RGB, sistema
   * Sistema operativo: Q = automatico, Z = Windows,
   *                    interno sx alto = macOS, interno sx medio = Linux
   * P = alterna QWERTY / Colemak-DH
   * RGB: O = acceso/spento, L/. = effetto succ./prec., ;// = luminosita',
   *      ' = torna all'effetto "Funzioni"
   * Su questo layer i LED NON cambiano colore: vedi l'effetto corrente.
   */
  [_SYS] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
       QK_BOOT, OS_AUTO,   KC_F9,  KC_F10,  KC_F11,  KC_F12,  OS_MAC,    XXXXXXX, KC_MUTE, KC_MPLY, KC_MSTP, RGB_TOG, CMK_TOG,  QK_RBT,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
       _______, KC_CAPS,   KC_F5,   KC_F6,   KC_F7,   KC_F8,  OS_LNX,    XXXXXXX, KC_VOLU, KC_BRIU, KC_MNXT, RGB_MOD, RGB_VAI,RGB_FUNZ,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       _______,  OS_WIN,   KC_F1,   KC_F2,   KC_F3,   KC_F4,                      KC_VOLD, KC_BRID, KC_MPRV,RGB_RMOD, RGB_VAD, XXXXXXX,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           _______, _______, XXXXXXX,    XXXXXXX, _______, XXXXXXX
                                       //`--------------------------'  `--------------------------'
  ),

  /* 4 DEBUG (bloccabile): tutto sulla mano sinistra, la destra resta
   * normale (mouse/trackball). Esci premendo di nuovo l'angolo alto sx.
   *
   *  riga alta : Restart  RunNoDebug  Breakpoint  GoToDef
   *  home row  : Stop  Continue  StepOver  StepInto  StepOut
   *  riga bassa: References  PeekDef  Rename  QuickFix
   */
  [_DEBUG] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
       _______, XXXXXXX,C(S(KC_F5)), SM_RUN,  KC_F9,  KC_F12, _______,    _______, _______, _______, _______, _______, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
       _______,S(KC_F5),   KC_F5,  KC_F10,  KC_F11,S(KC_F11),_______,    _______, _______, _______, _______, _______, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       _______, XXXXXXX,S(KC_F12),A(KC_F12), KC_F2,C(KC_DOT),                    _______, _______, _______, _______, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           _______, _______, _______,    _______, _______, _______
                                       //`--------------------------'  `--------------------------'
  ),

  /* 5 ACC (one-shot): vocali accentate, il resto e' trasparente */
  [_ACC] = LAYOUT_split_3x6_3_ex2(
  //,--------------------------------------------------------------.  ,--------------------------------------------------------------.
       _______, _______, _______,  ACC_EG,  ACC_EA, _______, _______,    _______, _______,   ACC_U,   ACC_I,   ACC_O, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------|  |--------+--------+--------+--------+--------+--------+--------|
       _______,   ACC_A, _______, _______, _______, _______, _______,    _______, _______, _______, _______, _______, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------'  `--------+--------+--------+--------+--------+--------+--------|
       _______, _______, _______, _______, _______, _______,                      _______, _______, _______, _______, _______, _______,
  //|--------+--------+--------+--------+--------+--------+--------.  ,--------+--------+--------+--------+--------+--------+--------|
                                           _______, _______, _______,    _______, _______, _______
                                       //`--------------------------'  `--------------------------'
  )
};

#ifdef ENCODER_MAP_ENABLE
// Usato solo se la tua scheda ha gli encoder rotativi (opzionali).
// Deve avere lo stesso numero di layer di "keymaps" (6).
const uint16_t PROGMEM encoder_map[][NUM_ENCODERS][NUM_DIRECTIONS] = {
  [_BASE]  = { ENCODER_CCW_CW(KC_VOLD, KC_VOLU), ENCODER_CCW_CW(KC_VOLD, KC_VOLU), ENCODER_CCW_CW(KC_VOLD, KC_VOLU), ENCODER_CCW_CW(KC_VOLD, KC_VOLU), },
  [_SYM]   = { ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), },
  [_NAV]   = { ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), },
  [_SYS]   = { ENCODER_CCW_CW(RGB_RMOD, RGB_MOD), ENCODER_CCW_CW(RGB_VAD, RGB_VAI), ENCODER_CCW_CW(RGB_RMOD, RGB_MOD), ENCODER_CCW_CW(RGB_VAD, RGB_VAI), },
  [_DEBUG] = { ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), },
  [_ACC]   = { ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), ENCODER_CCW_CW(_______, _______), },
};
#endif