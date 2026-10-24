# Keyboard HUD module. In the keymap's rules.mk:
#   include $(dir $(lastword $(MAKEFILE_LIST)))hud/hud.mk
# Needs VIA (Vial enables it) and a dynamic keymap.
HUD_DIR := $(patsubst %/,%,$(dir $(lastword $(MAKEFILE_LIST))))
SRC   += $(HUD_DIR)/hud.c
VPATH += $(HUD_DIR)
