# Shared code for the f16custom keymaps (Framework Laptop 16 keyboard + RGB macropad)
SRC += f16custom.c surface.c

# Our host-driven RGB effect lives in rgb_matrix_user.inc next to this file.
RGB_MATRIX_CUSTOM_USER = yes

# CI build number (build.sh passes it; 0 = test build). Shown by the app and used by its update check.
F16_FIRMWARE_BUILD ?= 0
OPT_DEFS += -DF16_FIRMWARE_BUILD=$(F16_FIRMWARE_BUILD)
