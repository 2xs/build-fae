TARGET = thumbv7em-none-eabi
PROFILE = dev
CARGO_OUT_DIR = $(if $(filter dev,$(PROFILE)),debug,$(PROFILE))
ELF = target/$(TARGET)/$(CARGO_OUT_DIR)/minimal_rust_print_CStrBuf

all: $(ELF)

$(ELF):
	cargo build --profile=$(PROFILE) --target $(TARGET)

clean:
	cargo clean

.PHONY: all clean
