pub const SUFFIX: &str = ".fae";
pub const GDBINIT_SUFFIX: &str = ".gdbinit";

pub const BINUTILS_PREFIX: &str = "arm-none-eabi-";
pub const OBJCOPY: &str = "arm-none-eabi-objcopy";
pub const OBJDUMP: &str = "arm-none-eabi-objdump";

pub const ENDIANNESS: &str = "little";

pub const OBJCOPY_ARGS_BASE: [&str; 3] = [
    "--input-target=elf32-littlearm",
    "--output-target=binary",
    "",
];

pub const MAGIC_NUMBER: u32 = 0xFACADE00;
pub const VERSION: u32 = 0x12;
pub const MAGIC_NUMBER_AND_VERSION: u32 = MAGIC_NUMBER | VERSION;

pub const STARTUP_SECTION_NAME: &str = "._start";
pub const STARTUP_SYMBOL_NAME: &str = "_start";
pub const DEFAULT_RUNTIME_ENTRY_POINT: &str = "start";
pub const DEFAULT_STARTUP_ALONE_RAM_SIZE: u32 = 1024;

pub const DEFAULT_PROFILE: &str = "release";

pub const EXPORTED_SYMBOL_ROM_RAM_SIZE: &str = "__rom_ram_size";
pub const EXPORTED_SYMBOL_ROM_SIZE: &str = "__rom_size";
pub const EXPORTED_SYMBOL_GOT_SIZE: &str = "__got_size";
pub const EXPORTED_SYMBOL_RAM_SIZE: &str = "__ram_size";

pub const ROM_SECTION_NAME: &str = ".rom";
pub const GOT_SECTION_NAME: &str = ".got";
pub const ROM_RAM_SECTION_NAME: &str = ".rom.ram";

pub const EXPORTED_SIZE_SYMBOLS: [&str; 4] = [
    EXPORTED_SYMBOL_ROM_RAM_SIZE,
    EXPORTED_SYMBOL_ROM_SIZE,
    EXPORTED_SYMBOL_GOT_SIZE,
    EXPORTED_SYMBOL_RAM_SIZE,
];

pub const EXPORTED_RELOCATION_TABLES: [&str; 1] = [".rel.rom.ram"];
pub const PARTITION_NAME: &str = "partition.fae";

pub const MAKE: &str = "make";

pub const BINARY_SIZE_BYTESIZE: usize = 4;
pub const PAYLOAD_PADDING_BYTESIZE: usize = 4;
pub const RELOCATION_ENTRIES_COUNT_BYTESIZE: usize = 4;
pub const STARTUP_CODE_SIZE_BYTESIZE: usize = 4;
pub const ENTRY_POINT_BYTESIZE: usize = 4;
pub const ROM_RAM_SIZE_BYTESIZE: usize = 4;
pub const ROM_SIZE_BYTESIZE: usize = 4;
pub const GOT_SIZE_BYTESIZE: usize = 4;
pub const RAM_SIZE_BYTESIZE: usize = 4;
pub const MAGIC_NUMBER_AND_VERSION_BYTESIZE: usize = 4;

pub const FOOTER_BYTESIZE: usize = RAM_SIZE_BYTESIZE
    + GOT_SIZE_BYTESIZE
    + ROM_SIZE_BYTESIZE
    + ROM_RAM_SIZE_BYTESIZE
    + ENTRY_POINT_BYTESIZE
    + STARTUP_CODE_SIZE_BYTESIZE
    + MAGIC_NUMBER_AND_VERSION_BYTESIZE;

pub const FOOTER_RAM_SIZE_OFFSET: isize = -28;
pub const FOOTER_GOT_SIZE_OFFSET: isize = -24;
pub const FOOTER_ROM_SIZE_OFFSET: isize = -20;
pub const FOOTER_ROM_RAM_SIZE_OFFSET: isize = -16;
pub const FOOTER_ENTRYPOINT_OFFSET: isize = -12;
pub const FOOTER_STARTUP_CODE_OFFSET: isize = -8;
pub const FOOTER_CRT0_OFFSET: isize = FOOTER_STARTUP_CODE_OFFSET;
pub const FOOTER_MAGIC_NUMBER_AND_VERSION_OFFSET: isize = -4;

pub const MINIMAL_BYTESIZE: usize = BINARY_SIZE_BYTESIZE
    + RELOCATION_ENTRIES_COUNT_BYTESIZE
    + PAYLOAD_PADDING_BYTESIZE
    + FOOTER_BYTESIZE;

pub const PADDING_VALUE: u8 = 0xFF;
pub const TEXT_ALIGNMENT_PADDING_VALUE: u8 = 0x00;
pub const SECTION_ALIGNMENT: usize = 4;
pub const PADDING_MPU_ALIGNMENT: usize = 32;

pub const R_ARM_ABS32: u32 = 2;
pub const R_ARM_REL32: u32 = 3;
pub const R_ARM_SBREL32: u32 = 9;
pub const R_ARM_GOT_BREL: u32 = 26;
pub const R_ARM_MOVW_PREL_NC: u32 = 45;
pub const R_ARM_MOVT_PREL: u32 = 46;
pub const R_ARM_THM_MOVW_PREL_NC: u32 = 49;
pub const R_ARM_THM_MOVT_PREL: u32 = 50;
pub const R_ARM_MOVW_BREL_NC: u32 = 85;
pub const R_ARM_MOVT_BREL: u32 = 86;
pub const R_ARM_THM_MOVW_BREL_NC: u32 = 87;
pub const R_ARM_THM_MOVT_BREL: u32 = 88;
