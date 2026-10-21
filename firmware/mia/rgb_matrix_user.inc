/* ==================================================================
 * EFFETTO RGB PERSONALIZZATO "FUNZIONI"
 *
 * Ogni gruppo di tasti ha una tonalita' ben distinta dalle altre
 * (colori saturi, distanti tra loro sulla ruota dei colori):
 *     lettere        azzurro
 *     home row       verde   (A S D, ; L K e G H restano azzurre)
 *     F e J          bianco  (punti di riferimento per gli indici)
 *     modificatori   giallo  (Tab/Ctrl, Shift, LGui, RAlt)
 *     pollici        magenta (i 6 tasti dei pollici)
 *     speciali       rosso   (debug, Esc, accenti, Backspace)
 *
 * L'indicatore QWERTY / Colemak-DH e' gestito in keymap.c e funziona
 * con qualsiasi effetto (tasti degli indici: bianco / viola).
 *
 * Reattivo sobrio: il tasto premuto diventa bianco luminoso e torna
 * al suo colore in circa mezzo secondo.
 *
 * Per cambiare i colori modifica la tabella "group_colors".
 * Tonalita': 0 rosso, 21 arancio, 40 giallo, 85 verde, 128 azzurro,
 *            170 blu, 191 viola, 213 magenta
 * Saturazione: 255 colore pieno, 0 bianco.
 * ================================================================== */

RGB_MATRIX_EFFECT(funzioni)

#ifdef RGB_MATRIX_CUSTOM_EFFECT_IMPLS

// Durata del bagliore dopo la pressione, in millisecondi
#    define FUNZIONI_FADE_MS 450
// Luminosita' minima dell'effetto: sotto questa soglia i LED WS2812
// perdono il colore e tutto sembra uguale.
#    define FUNZIONI_MIN_VAL 45

enum funzioni_group {
    G_NONE,
    G_LETTER,
    G_HOME,
    G_ANCHOR,
    G_MOD,
    G_THUMB,
    G_SPECIAL,
};

typedef struct {
    uint8_t hue;
    uint8_t sat;
    uint8_t level; // luminosita' relativa (255 = luminosita' impostata)
} funzioni_color_t;

static const funzioni_color_t group_colors[] = {
    [G_NONE]    = {  0,   0,   0},
    [G_LETTER]  = {128, 255, 170}, // azzurro
    [G_HOME]    = { 85, 255, 230}, // verde
    [G_ANCHOR]  = {  0,   0, 255}, // bianco
    [G_MOD]     = { 40, 255, 230}, // giallo
    [G_THUMB]   = {213, 255, 230}, // magenta
    [G_SPECIAL] = {  0, 255, 255}, // rosso
};

// Gruppo di ogni tasto, per posizione nella matrice della Corne v4.1.
// Righe 0-3 = meta' sinistra, 4-7 = destra.
// Colonna 0 = esterna, 5 = ultima lettera verso il centro, 6 = colonna interna.
static uint8_t funzioni_group_for(uint8_t row, uint8_t col) {
    uint8_t r    = row % 4; // 0 alta, 1 home, 2 bassa, 3 pollici
    bool    left = row < 4;

    if (r == 3) return G_THUMB;

    if (col == 0) {
        if (r == 0) return G_SPECIAL;               // sx: debug   dx: Backspace
        if (r == 1) return left ? G_MOD : G_LETTER; // sx: Tab/Ctrl dx: '
        return G_MOD;                               // Shift
    }
    if (col == 6) {
        if (left) return (r == 0) ? G_SPECIAL : G_MOD; // Esc | LGui
        return (r == 0) ? G_MOD : G_SPECIAL;           // RAlt | accenti
    }
    if (r == 1) {
        if (col == 4) return G_ANCHOR;           // F e J
        if (col >= 1 && col <= 3) return G_HOME; // A S D  e  ; L K
        return G_LETTER;                         // G e H
    }
    return G_LETTER;
}

static uint8_t funzioni_led_group[RGB_MATRIX_LED_COUNT];
static bool    funzioni_map_ready = false;

static void funzioni_build_map(void) {
    funzioni_map_ready = true;
    for (uint8_t i = 0; i < RGB_MATRIX_LED_COUNT; i++) funzioni_led_group[i] = G_NONE;
    for (uint8_t row = 0; row < MATRIX_ROWS; row++) {
        for (uint8_t col = 0; col < MATRIX_COLS; col++) {
            uint8_t led = g_led_config.matrix_co[row][col];
            if (led != NO_LED && led < RGB_MATRIX_LED_COUNT) {
                funzioni_led_group[led] = funzioni_group_for(row, col);
            }
        }
    }
}

// Quanto e' "acceso" il bagliore del LED i: 255 appena premuto, 0 dopo FUNZIONI_FADE_MS
static uint8_t funzioni_glow(uint8_t i) {
#    ifdef RGB_MATRIX_KEYREACTIVE_ENABLED
    for (int8_t j = g_last_hit_tracker.count - 1; j >= 0; j--) {
        if (g_last_hit_tracker.index[j] == i) {
            uint16_t t = g_last_hit_tracker.tick[j];
            if (t >= FUNZIONI_FADE_MS) return 0;
            return 255 - (uint8_t)((uint32_t)t * 255 / FUNZIONI_FADE_MS);
        }
    }
#    endif
    return 0;
}

static bool funzioni(effect_params_t* params) {
    RGB_MATRIX_USE_LIMITS(led_min, led_max);
    if (params->init || !funzioni_map_ready) funzioni_build_map();

    uint8_t max_v = rgb_matrix_config.hsv.v; // luminosita' scelta da te
    if (max_v < FUNZIONI_MIN_VAL) max_v = FUNZIONI_MIN_VAL;

    for (uint8_t i = led_min; i < led_max; i++) {
        RGB_MATRIX_TEST_LED_FLAGS();
        const funzioni_color_t c = group_colors[funzioni_led_group[i]];

        uint8_t v = scale8(max_v, c.level);
        uint8_t s = c.sat;

        uint8_t glow = funzioni_glow(i);
        if (glow) {
            // verso il bianco, alla luminosita' piena
            v = v + scale8(max_v - v, glow);
            s = s - scale8(s, glow);
        }

        hsv_t hsv = {c.hue, s, v};
        rgb_t rgb = hsv_to_rgb(hsv);
        rgb_matrix_set_color(i, rgb.r, rgb.g, rgb.b);
    }
    return rgb_matrix_check_finished_leds(led_max);
}

#endif // RGB_MATRIX_CUSTOM_EFFECT_IMPLS
