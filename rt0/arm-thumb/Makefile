PREFIX ?= arm-none-eabi-
CC = $(PREFIX)gcc
LD = $(PREFIX)ld

TARGET = crt0

all: $(TARGET).elf

$(TARGET).elf: build/rt0.o
	$(LD) -Tlink.ld $< -o $@

build:
	mkdir -p build

build/rt0.o: core.s rt0.s | build
	$(CC) -c -mthumb -mcpu=cortex-m4 -I../.. core.s -o $@

clean:
	rm -f build/rt0.o $(TARGET).elf $(TARGET).fae $(TARGET).gdbinit

realclean: clean
	rm -rf build

.PHONY: all clean realclean
