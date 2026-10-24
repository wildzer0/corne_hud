VIA_ENABLE          = yes
VIAL_ENABLE         = yes
LTO_ENABLE          = yes

# The Pro Micro (ATmega32U4) has 28 KB: keep the stock Vial feature set.
# On an RP2040 board you can also enable CAPS_WORD_ENABLE,
# LAYER_LOCK_ENABLE and OS_DETECTION_ENABLE: the HUD shows them.
RGBLIGHT_ENABLE     = yes
RGB_MATRIX_ENABLE   = no
MOUSEKEY_ENABLE     = no
OLED_ENABLE         = no
EXTRAKEY_ENABLE     = no
COMBO_ENABLE        = no
QMK_SETTINGS        = no
CAPS_WORD_ENABLE    = no
LAYER_LOCK_ENABLE   = no
REPEAT_KEY_ENABLE   = no

# Keyboard HUD module. In your own keymap, copy firmware/hud/ next to
# rules.mk and use: include $(dir $(lastword $(MAKEFILE_LIST)))hud/hud.mk
include $(dir $(lastword $(MAKEFILE_LIST)))../../hud/hud.mk
