use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::io::Write;
use std::process::{Command, Output};

use anyhow::Result;
use tempfile::NamedTempFile;

use crate::common::{get_bytes_text_suffix, get_word_from_slice};
use crate::constants;
use crate::elf::ArmCodeMode;

pub const CLI_OPTION_DISASSEMBLE_CRT0: &str = "-Dcrt0";
pub const CLI_OPTION_DISASSEMBLE_CRT0_LONG: &str = "--Dcrt0";
pub const CLI_OPTION_HEXDUMP_CRT0: &str = "-Hcrt0";
pub const CLI_OPTION_HEXDUMP_CRT0_LONG: &str = "--Hcrt0";
pub const CLI_OPTION_HEXDUMP_RELOCATIONS: &str = "-Hreloc";
pub const CLI_OPTION_HEXDUMP_RELOCATIONS_LONG: &str = "--Hreloc";
pub const CLI_OPTION_HEXDUMP_ROM_RAM: &str = "-Hromram";
pub const CLI_OPTION_HEXDUMP_ROM_RAM_LONG: &str = "--Hromram";
pub const CLI_OPTION_DISASSEMBLE_TEXT: &str = "-Drom";
pub const CLI_OPTION_DISASSEMBLE_TEXT_LONG: &str = "--Drom";
pub const CLI_OPTION_HEXDUMP_TEXT: &str = "-Hrom";
pub const CLI_OPTION_HEXDUMP_TEXT_LONG: &str = "--Hrom";
pub const CLI_OPTION_HEXDUMP_GOT: &str = "-Hgot";
pub const CLI_OPTION_HEXDUMP_GOT_LONG: &str = "--Hgot";
pub const CLI_OPTION_VERBOSE: &str = "-v";
pub const CLI_OPTION_HELP: &str = "-h";
pub const CLI_OPTION_HELP_LONG: &str = "--help";

const CLI_OPTIONS: [(&str, &str); 15] = [
    (CLI_OPTION_DISASSEMBLE_CRT0, "Disassemble startup code"),
    (CLI_OPTION_DISASSEMBLE_CRT0_LONG, "Disassemble startup code"),
    (CLI_OPTION_HEXDUMP_CRT0, "Hexdump startup code"),
    (CLI_OPTION_HEXDUMP_CRT0_LONG, "Hexdump startup code"),
    (CLI_OPTION_HEXDUMP_RELOCATIONS, "Hexdump relocation table"),
    (
        CLI_OPTION_HEXDUMP_RELOCATIONS_LONG,
        "Hexdump relocation table",
    ),
    (CLI_OPTION_HEXDUMP_ROM_RAM, "Hexdump .rom.ram"),
    (CLI_OPTION_HEXDUMP_ROM_RAM_LONG, "Hexdump .rom.ram"),
    (CLI_OPTION_DISASSEMBLE_TEXT, "Disassemble .rom"),
    (CLI_OPTION_DISASSEMBLE_TEXT_LONG, "Disassemble .rom"),
    (CLI_OPTION_HEXDUMP_TEXT, "Hexdump .rom"),
    (CLI_OPTION_HEXDUMP_TEXT_LONG, "Hexdump .rom"),
    (CLI_OPTION_HEXDUMP_GOT, "Hexdump .got"),
    (CLI_OPTION_HEXDUMP_GOT_LONG, "Hexdump .got"),
    (CLI_OPTION_VERBOSE, "Enable all dumps and disassembly views"),
];

fn usage(argv0: &str) {
    println!("Usage:");
    println!(
        "  {argv0} [OPTIONS] <file.fae>\n  {argv0} {help_long}",
        help_long = CLI_OPTION_HELP_LONG
    );
    println!();
    println!("Inspect and validate a .fae file.");
    println!();
    println!("Options:");
    for (k, v) in CLI_OPTIONS {
        println!("  {k:<12} {v}");
    }
    println!("  {help:<12} Show this help", help = CLI_OPTION_HELP);
    println!(
        "  {help_long:<12} Show this help",
        help_long = CLI_OPTION_HELP_LONG
    );
}

fn usage_error(argv0: &str, message: &str) -> Result<BTreeSet<String>, i32> {
    eprintln!("error: {message}");
    usage(argv0);
    Err(1)
}

fn run_command(mut cmd: Command, tool_name: &str, argv0: &str) -> Output {
    cmd.output().unwrap_or_else(|e| {
        if e.kind() == ErrorKind::NotFound {
            die(
                argv0,
                &format!(
                    "{} not found in PATH (required external tool: {})",
                    tool_name, tool_name
                ),
            )
        } else {
            die(argv0, &format!("failed to launch {}: {}", tool_name, e))
        }
    })
}

fn parse_options(args: &[String]) -> Result<BTreeSet<String>, i32> {
    if args.len() < 2 {
        return usage_error(&args[0], "missing input file");
    }
    if args.len() == 2 && (args[1] == CLI_OPTION_HELP || args[1] == CLI_OPTION_HELP_LONG) {
        usage(&args[0]);
        return Err(0);
    }
    if args.last().is_some_and(|x| x.starts_with('-')) {
        return usage_error(&args[0], "missing input file");
    }

    let mut options = BTreeSet::new();
    for arg in args.iter().skip(1).take(args.len() - 2) {
        if CLI_OPTIONS.iter().any(|(k, _)| *k == arg.as_str()) {
            if arg == CLI_OPTION_VERBOSE {
                for (k, _) in CLI_OPTIONS {
                    if k != CLI_OPTION_VERBOSE {
                        options.insert(k.to_string());
                    }
                }
            } else {
                options.insert(arg.clone());
            }
        } else {
            return usage_error(&args[0], &format!("unrecognized option: {}", arg));
        }
    }

    Ok(options)
}

fn has_option(options: &BTreeSet<String>, short: &str, long: &str) -> bool {
    options.contains(short) || options.contains(long)
}

fn die(argv0: &str, message: &str) -> ! {
    eprintln!("\x1b[91;1m{}: {}\x1b[0m", argv0, message);
    std::process::exit(1)
}

fn die_on_invalid_file(argv0: &str, filename: &str, message: &str) -> ! {
    die(argv0, &format!("Invalid file {} : {}", filename, message));
}

fn check_less_than_binary_size(
    argv0: &str,
    filename: &str,
    label: &str,
    value: usize,
    binary_size: usize,
) {
    if value >= binary_size {
        die_on_invalid_file(
            argv0,
            filename,
            &format!(
                "{} ({}) is greater than or equal to binary size",
                label, value
            ),
        );
    }
}

fn check_value_from_file(
    argv0: &str,
    filename: &str,
    label: &str,
    value: usize,
    binary_size: usize,
) {
    check_less_than_binary_size(argv0, filename, label, value, binary_size);
}

fn print_bytesize(label: &str, value: usize) {
    println!("{} : {} {}", label, value, get_bytes_text_suffix(value));
}

fn print_chunk(chunk_name: &str, chunk_start: usize, chunk_end: usize, chunk_size: usize) {
    println!(
        "- {} : [start : @{} {} , end : @{} {}] (size : {} {})",
        chunk_name,
        chunk_start,
        get_bytes_text_suffix(chunk_start),
        chunk_end,
        get_bytes_text_suffix(chunk_end),
        chunk_size,
        get_bytes_text_suffix(chunk_size)
    );
}

fn print_disassembly(
    argv0: &str,
    label: &str,
    fae_memory: &[u8],
    start: usize,
    end: usize,
    mode: ArmCodeMode,
) {
    let end = end.min(fae_memory.len());
    let start = start.min(end);

    let mut file = NamedTempFile::new().unwrap_or_else(|e| die(argv0, &format!("{}", e)));
    file.write_all(&fae_memory[start..end])
        .unwrap_or_else(|e| die(argv0, &format!("{}", e)));

    let mut cmd = Command::new(constants::OBJDUMP);
    cmd.arg("-b")
        .arg("binary")
        .arg("-marm")
        .arg(format!("--endian={}", constants::ENDIANNESS))
        .arg("-D");
    if mode == ArmCodeMode::Thumb {
        cmd.arg("-Mforce-thumb");
    }
    cmd.arg(file.path());
    let result = run_command(cmd, constants::OBJDUMP, argv0);

    if !result.status.success() {
        die(argv0, &format!("Failed to disassemble {}", label));
    }
    print!("{}", String::from_utf8_lossy(&result.stdout));
}

fn print_hexdump_line(
    line_number: usize,
    data: &[u8],
    start_index_in_bytes: usize,
    bytescount: usize,
) {
    let line_number_string = format!("{:08x}", line_number);

    let (this_bytes_count, these_bytes, a, b) = if bytescount > 16 {
        let these_bytes = &data[start_index_in_bytes..start_index_in_bytes + 16];
        (
            16,
            these_bytes,
            hex_space(&these_bytes[0..8]),
            hex_space(&these_bytes[8..16]),
        )
    } else if bytescount > 8 {
        let these_bytes = &data[start_index_in_bytes..start_index_in_bytes + bytescount];
        let mut b = hex_space(&these_bytes[8..bytescount]);
        while b.len() < 23 {
            b.push(' ');
        }
        (bytescount, these_bytes, hex_space(&these_bytes[0..8]), b)
    } else {
        let these_bytes = &data[start_index_in_bytes..start_index_in_bytes + bytescount];
        (
            bytescount,
            these_bytes,
            hex_space(these_bytes),
            "                                   ".to_string(),
        )
    };

    let mut c = String::from("|");
    for b in these_bytes.iter().take(this_bytes_count) {
        let ch = *b as char;
        if ch.is_ascii() && !ch.is_ascii_control() {
            c.push(ch);
        } else {
            c.push('.');
        }
    }
    while c.len() < 17 {
        c.push(' ');
    }
    c.push('|');

    println!("{}  {}  {}  {}", line_number_string, a, b, c);
}

fn hex_space(data: &[u8]) -> String {
    let mut out = String::new();
    for (i, b) in data.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn print_hexdump(fae_memoryview: &[u8]) {
    let mut offset = 0usize;
    while offset < fae_memoryview.len() {
        let remaining = fae_memoryview.len() - offset;
        let count = remaining.min(16);
        print_hexdump_line(offset, fae_memoryview, offset, count);
        offset += count;
    }
}

fn checked_end(start: usize, size: usize) -> Option<usize> {
    size.checked_sub(1).and_then(|v| start.checked_add(v))
}

fn checked_add_or_die(argv0: &str, filename: &str, label: &str, a: usize, b: usize) -> usize {
    a.checked_add(b)
        .unwrap_or_else(|| die_on_invalid_file(argv0, filename, &format!("{} overflow", label)))
}

struct IntegrityLayout {
    format: FormatInfo,
    binary_size: usize,
    startup_code_size: usize,
    relocation_entries_start: usize,
    relocation_entries_count: usize,
    relocation_entries_size: usize,
    payload_padding_size: usize,
    partition_offset: usize,
    entrypoint_value: usize,
    entrypoint_offset: usize,
    entrypoint_mode: ArmCodeMode,
    rom_size: usize,
    got_size: usize,
    rom_ram_size: usize,
    ram_size: usize,
    rom_start: usize,
    got_start: usize,
    rom_ram_start: usize,
}

#[derive(Debug, Clone)]
enum FormatInfo {
    Facade12,
    Fae1 {
        isa: fae_core::fae1::IsaDescriptor,
        abi: fae_core::fae1::AbiDescriptor,
        extra_words: Vec<u32>,
        crc: u32,
        footer_size: usize,
    },
}

struct XipfsFooterFields {
    footer_size: usize,
    startup_code_size: usize,
    entrypoint_value: usize,
    rom_size: usize,
    got_size: usize,
    rom_ram_size: usize,
    ram_size: usize,
    format: FormatInfo,
}

fn decode_xipfs_footer(argv0: &str, filename: &str, image: &[u8]) -> XipfsFooterFields {
    let magic = get_word_from_slice(image, -4)
        .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")));
    if magic == constants::MAGIC_NUMBER_AND_VERSION {
        return XipfsFooterFields {
            footer_size: constants::FOOTER_BYTESIZE,
            startup_code_size: get_word_from_slice(image, constants::FOOTER_STARTUP_CODE_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            entrypoint_value: get_word_from_slice(image, constants::FOOTER_ENTRYPOINT_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            rom_size: get_word_from_slice(image, constants::FOOTER_ROM_SIZE_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            got_size: get_word_from_slice(image, constants::FOOTER_GOT_SIZE_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            rom_ram_size: get_word_from_slice(image, constants::FOOTER_ROM_RAM_SIZE_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            ram_size: get_word_from_slice(image, constants::FOOTER_RAM_SIZE_OFFSET)
                .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{e}")))
                as usize,
            format: FormatInfo::Facade12,
        };
    }
    if magic != fae_core::fae1::MAGIC_AND_VERSION {
        if (constants::MAGIC_NUMBER..=(constants::MAGIC_NUMBER | 0xFF)).contains(&magic) {
            die_on_invalid_file(
                argv0,
                filename,
                &format!("version not supported: 0x{magic:08X}"),
            );
        }
        die_on_invalid_file(
            argv0,
            filename,
            &format!("unsupported FAE magic/version: 0x{magic:08X}"),
        );
    }

    let decoded = fae_core::fae1::decode(image).unwrap_or_else(|e| {
        die_on_invalid_file(argv0, filename, &format!("invalid FAE 1.0 footer: {e}"))
    });
    let profile = fae_core::fae1::XipfsProfile::decode(&decoded.footer).unwrap_or_else(|e| {
        die_on_invalid_file(
            argv0,
            filename,
            &format!("unsupported FAE 1.0 ABI for XiPFS inspection: {e}"),
        )
    });
    XipfsFooterFields {
        footer_size: decoded.footer_size,
        startup_code_size: profile.startup_code_size as usize,
        entrypoint_value: profile.entrypoint_offset as usize,
        rom_size: profile.rom_size as usize,
        got_size: profile.got_size as usize,
        rom_ram_size: profile.rom_ram_size as usize,
        ram_size: profile.ram_size as usize,
        format: FormatInfo::Fae1 {
            isa: decoded.footer.isa,
            abi: decoded.footer.abi,
            extra_words: decoded.footer.extra_words,
            crc: decoded.stored_crc,
            footer_size: decoded.footer_size,
        },
    }
}

fn integrity_check(argv0: &str, filename: &str, fae_bytearray: &[u8]) -> IntegrityLayout {
    let binary_size = fae_bytearray.len();
    if binary_size == 0 {
        die_on_invalid_file(argv0, filename, "Empty file");
    }

    if binary_size <= constants::MINIMAL_BYTESIZE {
        die_on_invalid_file(
            argv0,
            filename,
            &format!(
                "Binary size is less than or equal to minimal bytesize ({})",
                constants::MINIMAL_BYTESIZE
            ),
        );
    }

    if !binary_size.is_multiple_of(constants::PADDING_MPU_ALIGNMENT) {
        die_on_invalid_file(
            argv0,
            filename,
            &format!(
                "binary size ({}) is not a multiple of {}",
                binary_size,
                constants::PADDING_MPU_ALIGNMENT
            ),
        );
    }

    let footer = decode_xipfs_footer(argv0, filename, fae_bytearray);
    let startup_code_size = footer.startup_code_size;
    check_value_from_file(
        argv0,
        filename,
        "startup_code_size",
        startup_code_size,
        binary_size,
    );
    if startup_code_size == 0 {
        die_on_invalid_file(argv0, filename, "startup_code_size is equal to 0");
    }

    let binary_size_from_file = get_word_from_slice(fae_bytearray, startup_code_size as isize)
        .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{}", e)))
        as usize;
    if binary_size_from_file != binary_size {
        die_on_invalid_file(
            argv0,
            filename,
            &format!(
                "binary_size_from_file ({}) is different from binary size",
                binary_size_from_file
            ),
        );
    }

    let relocation_entries_start = checked_add_or_die(
        argv0,
        filename,
        "relocation count word offset",
        startup_code_size,
        constants::BINARY_SIZE_BYTESIZE,
    );
    check_less_than_binary_size(
        argv0,
        filename,
        "relocation count word offset",
        relocation_entries_start,
        binary_size,
    );

    let relocation_entries_count =
        get_word_from_slice(fae_bytearray, relocation_entries_start as isize)
            .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{}", e)))
            as usize;
    let relocation_entries_size = relocation_entries_count.checked_mul(4).unwrap_or_else(|| {
        die_on_invalid_file(argv0, filename, "relocation entries size overflow")
    });
    let relocation_table_size = constants::RELOCATION_ENTRIES_COUNT_BYTESIZE
        .checked_add(relocation_entries_size)
        .unwrap_or_else(|| die_on_invalid_file(argv0, filename, "relocation table size overflow"));
    let payload_padding_offset = checked_add_or_die(
        argv0,
        filename,
        "payload padding offset",
        relocation_entries_start,
        relocation_table_size,
    );
    check_less_than_binary_size(
        argv0,
        filename,
        "payload padding offset",
        payload_padding_offset,
        binary_size,
    );
    let payload_padding_size = get_word_from_slice(fae_bytearray, payload_padding_offset as isize)
        .unwrap_or_else(|e| die_on_invalid_file(argv0, filename, &format!("{}", e)))
        as usize;
    let payload_padding_start = checked_add_or_die(
        argv0,
        filename,
        "payload padding start",
        payload_padding_offset,
        constants::PAYLOAD_PADDING_BYTESIZE,
    );
    let partition_offset = checked_add_or_die(
        argv0,
        filename,
        "partition offset",
        payload_padding_start,
        payload_padding_size,
    );

    let footer_start = binary_size
        .checked_sub(footer.footer_size)
        .unwrap_or_else(|| die_on_invalid_file(argv0, filename, "binary size too small"));
    if partition_offset > footer_start {
        die_on_invalid_file(argv0, filename, "partition offset points inside footer");
    }
    if fae_bytearray[payload_padding_start..partition_offset]
        .iter()
        .any(|b| *b != constants::TEXT_ALIGNMENT_PADDING_VALUE)
    {
        die_on_invalid_file(
            argv0,
            filename,
            "payload alignment padding contains non-0x00 bytes",
        );
    }

    let entrypoint_value = footer.entrypoint_value;
    let entrypoint_mode = ArmCodeMode::from_symbol_value(entrypoint_value as u32);
    let entrypoint_offset = entrypoint_value & !1usize;
    check_value_from_file(
        argv0,
        filename,
        "Entrypoint offset",
        entrypoint_offset,
        binary_size,
    );

    let rom_size = footer.rom_size;
    check_value_from_file(argv0, filename, "rom_size", rom_size, binary_size);

    let got_size = footer.got_size;
    check_value_from_file(argv0, filename, "got size", got_size, binary_size);

    let rom_ram_size = footer.rom_ram_size;
    check_value_from_file(argv0, filename, "rom_ram_size", rom_ram_size, binary_size);

    let ram_size = footer.ram_size;

    let rom_start = partition_offset;
    let got_start = checked_add_or_die(argv0, filename, ".got start", rom_start, rom_size);
    let rom_ram_start = checked_add_or_die(argv0, filename, ".rom.ram start", got_start, got_size);
    let payload_end_exclusive =
        checked_add_or_die(argv0, filename, "payload end", rom_ram_start, rom_ram_size);

    if payload_end_exclusive > footer_start {
        die_on_invalid_file(
            argv0,
            filename,
            "section sizes overlap the final padding/footer trailer",
        );
    }

    if fae_bytearray[payload_end_exclusive..footer_start]
        .iter()
        .any(|b| *b != constants::PADDING_VALUE)
    {
        die_on_invalid_file(
            argv0,
            filename,
            "padding before footer contains non-0xFF bytes",
        );
    }

    let startup_only = rom_size == 0 && got_size == 0 && rom_ram_size == 0 && entrypoint_value == 0;
    if !startup_only && rom_size == 0 {
        die_on_invalid_file(argv0, filename, ".rom section is empty");
    }
    if !startup_only && entrypoint_offset >= rom_size {
        die_on_invalid_file(
            argv0,
            filename,
            "Entrypoint offset is out of .rom aka text section",
        );
    }

    IntegrityLayout {
        format: footer.format,
        binary_size,
        startup_code_size,
        relocation_entries_start,
        relocation_entries_count,
        relocation_entries_size,
        payload_padding_size,
        partition_offset,
        entrypoint_value,
        entrypoint_offset,
        entrypoint_mode,
        rom_size,
        got_size,
        rom_ram_size,
        ram_size,
        rom_start,
        got_start,
        rom_ram_start,
    }
}

pub fn run(args: &[String]) -> Result<(), i32> {
    let options = parse_options(args)?;
    let filename = args.last().expect("filename arg");

    let fae_bytearray = match fs::read(filename) {
        Ok(v) => v,
        Err(e) => die(&args[0], &format!("{}", e)),
    };

    let layout = integrity_check(&args[0], filename, &fae_bytearray);
    let binary_size = layout.binary_size;
    print_bytesize("- Binary size", binary_size);

    match &layout.format {
        FormatInfo::Facade12 => {
            println!("- Format : legacy FAE 0xFACADE12");
            println!(
                "- Magic Number And Version : {:#x}",
                constants::MAGIC_NUMBER_AND_VERSION
            );
            println!("- Footer size : {} bytes", constants::FOOTER_BYTESIZE);
        }
        FormatInfo::Fae1 {
            isa,
            abi,
            extra_words,
            crc,
            footer_size,
        } => {
            println!("- Format : FAE 1.0 (0xFAEC0D10)");
            println!("- Footer size : {footer_size} bytes");
            println!(
                "- ISA : family=0x{:02X}, subgroup=0x{:02X}, inline=0x{:03X}, extra_words={}",
                isa.family, isa.subgroup, isa.inline_bits, isa.extra_word_count
            );
            println!(
                "- ABI : XiPFS family=0x{:07X}, extra_words={}",
                abi.family, abi.extra_word_count
            );
            println!("- Generic extra fields : {extra_words:08X?}");
            println!("- CRC-32/ISO-HDLC : 0x{crc:08X} (OK)");
        }
    }

    let startup_code_size = layout.startup_code_size;

    print_chunk("startup code", 0, startup_code_size - 1, startup_code_size);
    if has_option(
        &options,
        CLI_OPTION_DISASSEMBLE_CRT0,
        CLI_OPTION_DISASSEMBLE_CRT0_LONG,
    ) {
        print_disassembly(
            &args[0],
            "startup code",
            &fae_bytearray,
            0,
            startup_code_size.saturating_sub(1),
            ArmCodeMode::Thumb,
        );
    }
    if has_option(
        &options,
        CLI_OPTION_HEXDUMP_CRT0,
        CLI_OPTION_HEXDUMP_CRT0_LONG,
    ) {
        print_hexdump(&fae_bytearray[0..startup_code_size]);
    }

    let relocation_entries_start = layout.relocation_entries_start;
    let relocation_entries_count = layout.relocation_entries_count;
    let relocation_entries_size = layout.relocation_entries_size;

    if relocation_entries_size > 0 {
        let relocation_entries_data_start =
            relocation_entries_start + constants::RELOCATION_ENTRIES_COUNT_BYTESIZE;
        let relocation_entries_end =
            checked_end(relocation_entries_data_start, relocation_entries_size).unwrap_or_else(
                || die_on_invalid_file(&args[0], filename, "relocation entries end overflow"),
            );
        print_chunk(
            "Relocation entries",
            relocation_entries_data_start,
            relocation_entries_end,
            relocation_entries_size,
        );
        println!("\t- Count : {}", relocation_entries_count);
        if has_option(
            &options,
            CLI_OPTION_HEXDUMP_RELOCATIONS,
            CLI_OPTION_HEXDUMP_RELOCATIONS_LONG,
        ) {
            print_hexdump(
                &fae_bytearray[relocation_entries_data_start
                    ..relocation_entries_data_start + relocation_entries_size],
            );
        }
    } else {
        println!("- Relocation entries : none.");
    }

    let partition_offset = layout.partition_offset;
    if layout.payload_padding_size > 0 {
        print_bytesize("- Payload alignment padding", layout.payload_padding_size);
    }
    print_bytesize("- Offset to partition", partition_offset);

    let entrypoint_value = layout.entrypoint_value;
    let entrypoint_mode = layout.entrypoint_mode;
    let entrypoint_offset = layout.entrypoint_offset;
    print_bytesize("- Entrypoint value", entrypoint_value);
    println!("- Entrypoint mode : {}", entrypoint_mode.as_str());
    print_bytesize("- Entrypoint offset in .rom", entrypoint_offset);
    let rom_size = layout.rom_size;
    let got_size = layout.got_size;
    let rom_ram_size = layout.rom_ram_size;
    let rom_start = layout.rom_start;
    let got_start = layout.got_start;
    let rom_ram_start = layout.rom_ram_start;

    if rom_size > 0 {
        let rom_end = checked_end(rom_start, rom_size)
            .unwrap_or_else(|| die_on_invalid_file(&args[0], filename, ".rom end overflow"));

        print_chunk(".rom", rom_start, rom_end, rom_size);
        if has_option(
            &options,
            CLI_OPTION_DISASSEMBLE_TEXT,
            CLI_OPTION_DISASSEMBLE_TEXT_LONG,
        ) {
            print_disassembly(
                &args[0],
                ".rom",
                &fae_bytearray,
                rom_start,
                rom_end,
                entrypoint_mode,
            );
        }
        if has_option(
            &options,
            CLI_OPTION_HEXDUMP_TEXT,
            CLI_OPTION_HEXDUMP_TEXT_LONG,
        ) {
            print_hexdump(&fae_bytearray[rom_start..rom_end + 1]);
        }
    } else {
        println!("- .rom : none.");
    }

    if got_size > 0 {
        let got_end = checked_end(got_start, got_size)
            .unwrap_or_else(|| die_on_invalid_file(&args[0], filename, ".got end overflow"));
        print_chunk(".got", got_start, got_end, got_size);
        if has_option(
            &options,
            CLI_OPTION_HEXDUMP_GOT,
            CLI_OPTION_HEXDUMP_GOT_LONG,
        ) {
            print_hexdump(&fae_bytearray[got_start..got_end + 1]);
        }
    } else {
        println!(".got : none.");
    }

    if rom_ram_size > 0 {
        let rom_ram_end = checked_end(rom_ram_start, rom_ram_size)
            .unwrap_or_else(|| die_on_invalid_file(&args[0], filename, ".rom.ram end overflow"));

        print_chunk(".rom.ram", rom_ram_start, rom_ram_end, rom_ram_size);
        if has_option(
            &options,
            CLI_OPTION_HEXDUMP_ROM_RAM,
            CLI_OPTION_HEXDUMP_ROM_RAM_LONG,
        ) {
            print_hexdump(&fae_bytearray[rom_ram_start..rom_ram_end + 1]);
        }
    } else {
        println!("- .rom.ram : none.");
    }

    let ram_size = layout.ram_size;
    print_bytesize("- writable RAM (.data + .bss)", ram_size);

    println!("- Integrity check : OK");

    Ok(())
}
