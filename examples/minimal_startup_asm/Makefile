PREFIX ?= arm-none-eabi-
CC = $(PREFIX)gcc
LD = $(PREFIX)ld

TARGET = startup

all: build/$(TARGET).elf

build:
	mkdir -p build

build/$(TARGET).elf: build/startup.o | build
	$(LD) -Tlink.ld $< -o $@

build/startup.o: startup.S | build
	$(CC) -c -mthumb -mcpu=cortex-m4 $< -o $@

clean:
	rm -f build/startup.o build/$(TARGET).elf build/$(TARGET).fae build/$(TARGET).gdbinit

realclean: clean
	rm -rf build

.PHONY: all clean realclean
