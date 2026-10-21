# ------------------------------------------------------------
# Corne v4.1 (standard, 46 tasti) - keymap "mia" per Vial
# ------------------------------------------------------------

# Vial (obbligatori)
VIA_ENABLE          = yes
VIAL_ENABLE         = yes
VIALRGB_ENABLE      = yes   # gestione RGB da Vial
ENCODER_MAP_ENABLE  = yes   # innocuo se non hai encoder

# Funzioni QMK
# (Vial le abilita gia' di default: le scrivo esplicite cosi'
#  sai cosa c'e' e puoi spegnerle mettendo "no")
CAPS_WORD_ENABLE    = yes   # CW_TOGG: maiuscole per una sola parola
REPEAT_KEY_ENABLE   = yes   # QK_REP: ripete l'ultimo tasto
KEY_OVERRIDE_ENABLE = yes   # es. Shift+Bksp = Del (si configura in Vial)
COMBO_ENABLE        = yes   # combo (si configurano in Vial)
TAP_DANCE_ENABLE    = yes   # tap dance (si configurano in Vial)
LAYER_LOCK_ENABLE   = yes   # QK_LLCK: blocca il layer corrente
QMK_SETTINGS        = yes   # tab "QMK Settings" in Vial (tapping term, ecc.)
MOUSEKEY_ENABLE     = yes   # muovere il mouse dalla tastiera
EXTRAKEY_ENABLE     = yes   # volume, media, luminosita' schermo
OS_DETECTION_ENABLE = yes   # riconosce Windows/macOS/Linux (per le accentate)

# Debounce "eager per tasto": invia subito la pressione e filtra dopo.
# Se noti lettere doppie, cancella questa riga e ricompila.
DEBOUNCE_TYPE       = sym_eager_pk

# Effetti RGB personalizzati (file rgb_matrix_user.inc)
RGB_MATRIX_CUSTOM_USER = yes

# Firmware piu' compatto
LTO_ENABLE          = yes