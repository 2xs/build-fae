PREFIX ?= arm-none-eabi-
CC = $(PREFIX)gcc
AR = $(PREFIX)ar
MESON ?= meson
NINJA ?= ninja
GIT ?= git

CPU ?= cortex-m33
TARGET = faeulibc
HEAP_SIZE ?= 1024
NO_HEAP ?= false

ifeq ($(HEAP_SIZE),0)
NO_HEAP := true
endif

ifeq ($(NO_HEAP),true)
HEAP_SIZE_EFFECTIVE := 0
else
HEAP_SIZE_EFFECTIVE := $(HEAP_SIZE)
endif

BUILD_DIR = build
PICOLIBC_DIR = picolibc
PICOLIBC_BUILD = $(BUILD_DIR)/picolibc
PICOLIBC_CROSS = fae/meson-cross-arm-none-eabi.txt
PICOLIBC_URL ?= https://github.com/picolibc/picolibc.git
PICOLIBC_TAG ?= 1.8.11
PICOLIBC_LIB = $(shell find $(PICOLIBC_BUILD) -name libc.a -print -quit 2>/dev/null)
PICOLIBC_STAMP = $(PICOLIBC_BUILD)/build.ninja

APP_SRCS = \
	app/main.c \
	fae/start.c \
	fae/hs_backend.c \
	fae/stdio_glue.c \
	fae/syscalls_min.c

APP_OBJS = $(patsubst %.c,$(BUILD_DIR)/%.o,$(APP_SRCS))

CFLAGS = -Wall -Wextra -Werror
CFLAGS += -mthumb
CFLAGS += -mcpu=$(CPU)
CFLAGS += -mfloat-abi=soft
CFLAGS += -msingle-pic-base
CFLAGS += -mpic-register=r9
CFLAGS += -mno-pic-data-is-text-relative
CFLAGS += -fPIC
CFLAGS += -ffreestanding
CFLAGS += -ffunction-sections
CFLAGS += -fdata-sections
CFLAGS += -Os
CFLAGS += -I$(PICOLIBC_BUILD)
CFLAGS += -I$(PICOLIBC_DIR)/libc/include
CFLAGS += -I.

LDFLAGS = -nostartfiles -nostdlib -nolibc
LDFLAGS += -Tfae/link.ld
LDFLAGS += -Wl,-q
LDFLAGS += -Wl,--gc-sections
LDFLAGS += -Wl,--defsym=__heap_size=$(HEAP_SIZE_EFFECTIVE)
ifeq ($(shell $(PREFIX)ld --help | grep -q 'warn-rwx-segments'; echo $$?), 0)
LDFLAGS += -Wl,--no-warn-rwx-segments
endif

all: $(BUILD_DIR)/$(TARGET).elf

help:
	@echo "Targets:"
	@echo "  all              build picolibc then link $(TARGET).elf"
	@echo "  fetch-picolibc   init submodule or clone picolibc at $(PICOLIBC_TAG)"
	@echo "  picolibc         configure and build the vendored picolibc"
	@echo "  fae              package the ELF into a .fae using build_fae"
	@echo "  clean            remove local build products"
	@echo
	@echo "Variables:"
	@echo "  HEAP_SIZE        requested heap size in bytes ; default: $(HEAP_SIZE)"
	@echo "  NO_HEAP          disable linker-provided heap (default: $(NO_HEAP))"

check-tools:
	$(CC) -v >/dev/null
	$(AR) -v >/dev/null
	$(MESON) -v >/dev/null
	$(NINJA) -v >/dev/null
	$(GIT) -v >/dev/null

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(BUILD_DIR)/fae:
	mkdir -p $(BUILD_DIR)/fae

$(BUILD_DIR)/app:
	mkdir -p $(BUILD_DIR)/app

fetch-picolibc: | check-tools
	@if test -f "$(PICOLIBC_DIR)/meson.build"; then \
		echo "Using existing $(PICOLIBC_DIR) checkout"; \
	elif test -f .gitmodules && $(GIT) config --file .gitmodules --get-regexp '^submodule\\..*\\.path$$' | awk '{print $$2}' | grep -qx '$(PICOLIBC_DIR)'; then \
		echo "Initializing picolibc submodule"; \
		$(GIT) submodule update --init --recursive $(PICOLIBC_DIR); \
	else \
		echo "Cloning picolibc $(PICOLIBC_TAG) into $(PICOLIBC_DIR)"; \
		$(GIT) clone --depth 1 --branch $(PICOLIBC_TAG) $(PICOLIBC_URL) $(PICOLIBC_DIR); \
	fi

$(PICOLIBC_STAMP): | check-tools $(BUILD_DIR) fetch-picolibc
	$(MESON) setup $(PICOLIBC_BUILD) $(PICOLIBC_DIR) \
		--cross-file $(PICOLIBC_CROSS) \
		-Dmultilib=false \
		-Dtests=false \
		-Dpicocrt=false \
		-Dpicocrt-lib=false \
		-Dposix-console=false \
		-Dstdio-locking=false \
		-Dprintf-small-ultoa=true \
		-Dio-long-long=false \
		-Dio-long-double=false \
		-Dminimal-io-long-long=false \
		-Dspecsdir=none

picolibc: $(PICOLIBC_STAMP)
	$(NINJA) -C $(PICOLIBC_BUILD)

$(BUILD_DIR)/%.o: %.c | $(BUILD_DIR) $(BUILD_DIR)/fae $(BUILD_DIR)/app picolibc
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/$(TARGET).elf: $(APP_OBJS)
	@test -n "$(PICOLIBC_LIB)" || (echo "picolibc libc.a not found under $(PICOLIBC_BUILD)"; exit 1)
	$(CC) $(CFLAGS) $(LDFLAGS) $^ $(PICOLIBC_LIB) -lgcc -o $@

fae: $(BUILD_DIR)/$(TARGET).elf
	cargo run --bin build_fae -- $<

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all help check-tools fetch-picolibc picolibc fae clean
