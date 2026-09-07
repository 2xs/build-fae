PREFIX ?= arm-none-eabi-
CC = $(PREFIX)gcc
LD = $(PREFIX)gcc

CFLAGS = -Wall -Wextra -Werror
CFLAGS += -mthumb
CFLAGS += -mcpu=cortex-m33
CFLAGS += -mfloat-abi=soft
CFLAGS += -msingle-pic-base
CFLAGS += -mpic-register=r9
CFLAGS += -mno-pic-data-is-text-relative
CFLAGS += -fPIC
CFLAGS += -ffreestanding
CFLAGS += -Os

LDFLAGS = -nostartfiles -nodefaultlibs -nolibc -nostdlib
LDFLAGS += -Tlink.ld
LDFLAGS += -Wl,-q
ifeq ($(shell $(PREFIX)ld --help | grep -q 'warn-rwx-segments'; echo $$?), 0)
LDFLAGS += -Wl,--no-warn-rwx-segments
endif

TARGET = minimal

all: build/$(TARGET).elf

build:
	mkdir -p build

build/$(TARGET).elf: build/minimal.o | build
	$(LD) $(LDFLAGS) $< -o $@

build/minimal.o: minimal.c | build
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f build/minimal.o build/$(TARGET).elf build/$(TARGET).fae build/$(TARGET).gdbinit

realclean: clean
	rm -rf build

.PHONY: all clean realclean
