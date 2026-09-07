use anyhow::{anyhow, bail, Context, Result};
use goblin::elf::header::EM_ARM;
use std::fs;
use std::path::{Path, PathBuf};

use crate::constants;
use crate::elf::{
    export_relocation_table, export_section_bytes, export_startup_code, export_symbol_value,
    export_symbols_to_struct, parse_elf, symbol_code_mode, validate_abs32_relocation_targets,
    validate_no_exec_relocations_to_runtime_writable_sections, ArmCodeMode,
};
use crate::fae_format::{
    compute_aligned_header_size_for_format, concatenate_and_pad_bytearray_for_format,
    padded_exported_symbols, remap_offset_to_padded_layout, DescriptorWords, ExportedSymbols,
    Fae1Abi, OutputFormat,
};
use crate::gdbinit::{generate_gdbinit, GdbMemoryLayout};

pub const CLI_OPTION_RT0: &str = "--rt0";
pub const CLI_OPTION_RT0_SHORT: &str = "-rt0";
pub const CLI_OPTION_FIRMWARE: &str = "--firmware";
pub const CLI_OPTION_STARTUP_ALONE: &str = "--startup-alone";
pub const CLI_OPTION_RUNTIME_ENTRY_POINT: &str = "--runtime-entry-point";
pub const CLI_OPTION_RAM_SIZE: &str = "--ram-size";
pub const CLI_OPTION_VERBOSE: &str = "-v";
pub const CLI_OPTION_VERBOSE_LONG: &str = "--verbose";
pub const CLI_OPTION_HELP: &str = "-h";
pub const CLI_OPTION_HELP_LONG: &str = "--help";
pub const CLI_OPTION_ALIGN_PAYLOAD: &str = "--align_payload";
pub const CLI_OPTION_ALIGN_SIZE: &str = "--align_size";
pub const CLI_OPTION_FACADE12: &str = "--facade12";
pub const CLI_OPTION_ABI: &str = "--abi";
pub const CLI_OPTION_ABI_WORD: &str = "--abi-word";
pub const CLI_OPTION_ISA: &str = "--isa";
pub const CLI_OPTION_ISA_WORD: &str = "--isa-word";
pub const CLI_OPTION_RUSTLET: &str = "--rustlet";
pub const CLI_OPTION_RUSTLET_SHORT: &str = "-rs";
pub const CLI_OPTION_SECURITY_DOMAIN: &str = "--securitydomain";
pub const CLI_OPTION_SECURITY_DOMAIN_SHORT: &str = "-sd";
pub const CLI_OPTION_STACK_SIZE: &str = "--stack-size";
pub const CLI_OPTION_ALLOW_RT0_MISMATCH: &str = "--allow-rt0-mismatch";
const DEFAULT_RUSTLET_STACK_SIZE: u32 = 2048;

#[derive(Debug, Clone)]
struct BuildOptions {
    startup_source: StartupSource,
    startup_selection: StartupSelection,
    mode: BuildMode,
    runtime_entry_point: String,
    ram_size: u32,
    align_payload: usize,
    align_size: usize,
    facade12: bool,
    isa: Option<DescriptorWords>,
    abi: BuildAbi,
    stack_size: u32,
    allow_rt0_mismatch: bool,
    verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuildAbi {
    Xipfs,
    Rustlet,
    RustletSecurityDomain,
    Explicit(DescriptorWords),
}

impl BuildAbi {
    fn uses_rustlet_layout(self) -> bool {
        match self {
            Self::Rustlet | Self::RustletSecurityDomain => true,
            Self::Explicit(words) => {
                fae_core::fae1::AbiDescriptor::decode(words.descriptor()).family
                    == fae_core::fae1::registry::abi::OXIDE_SE
            }
            Self::Xipfs => false,
        }
    }
}

#[derive(Debug, Clone)]
enum StartupSource {
    EmbeddedDefaultRuntime,
    EmbeddedFirmware(FirmwareBoard),
    ExplicitElf(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StartupKind {
    Thumb,
    ThumbV6M,
    Arm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FirmwareBoard {
    Mps2An385,
    Mps2An386,
    OlimexStm32H405,
    BL475eIot01a,
}

#[derive(Debug, Clone, Copy)]
enum StartupSelection {
    Auto,
    Explicit(StartupKind),
}

#[derive(Debug, Clone)]
enum BuildMode {
    Application { elf_filename: String },
    StartupAlone,
}

#[derive(Debug, Clone, Copy)]
enum EmbeddedStartup {
    Runtime(StartupKind),
    Firmware(FirmwareBoard),
}

#[derive(Debug, Clone, Copy)]
struct EmbeddedStartupSpec {
    output_stem: &'static str,
    symbol_filename: &'static str,
    generated_filename: &'static str,
    fae1_symbol_filename: &'static str,
    fae1_generated_filename: &'static str,
    rustlet_symbol_filename: Option<&'static str>,
    rustlet_generated_filename: Option<&'static str>,
}

#[derive(Debug, Clone)]
enum ResolvedStartup {
    Embedded {
        spec: EmbeddedStartupSpec,
        fae1: bool,
        rustlet: bool,
    },
    Path {
        elf_path: PathBuf,
    },
}

fn usage(argv0: &str) {
    println!("Usage:");
    println!(
        "  {argv0} [OPTIONS] <input-elf>\n  {argv0} [OPTIONS] {startup_alone}\n  {argv0} {help_long}",
        startup_alone = CLI_OPTION_STARTUP_ALONE,
        help_long = CLI_OPTION_HELP_LONG
    );
    println!();
    println!("Build a .fae file and a matching .gdbinit companion file.");
    println!();
    println!("Options:");
    println!(
        "  {:<32} Select a bundled RT0 or an explicit RT0 ELF",
        "-rt0, --rt0 <thumb|thumbv6m|arm|file.elf>"
    );
    println!(
        "  {:<32}",
        "--firmware <mps2-an385|mps2-an386|olimex-stm32-h405|b-l475e-iot01a>"
    );
    println!("                                 Select a bundled firmware startup");
    println!(
        "  {:<32} Runtime entry symbol in the input ELF",
        "--runtime-entry-point <symbol>"
    );
    println!(
        "                                 Default: {}",
        constants::DEFAULT_RUNTIME_ENTRY_POINT
    );
    println!(
        "  {:<32} Build a startup-only FAE",
        CLI_OPTION_STARTUP_ALONE
    );
    println!(
        "  {:<32} RAM size to store with {startup_alone}",
        "--ram-size <bytes>",
        startup_alone = CLI_OPTION_STARTUP_ALONE
    );
    println!(
        "                                 Default: {}",
        constants::DEFAULT_STARTUP_ALONE_RAM_SIZE
    );
    println!(
        "  {:<32} Align the payload partition offset",
        "--align_payload <bytes>"
    );
    println!("                                 Default: 0");
    println!(
        "  {:<32} Align the final FAE file size",
        "--align_size <bytes>"
    );
    println!(
        "                                 Default: {}",
        constants::PADDING_MPU_ALIGNMENT
    );
    println!(
        "  {:<32} Emit the legacy 0xFACADE12 footer",
        CLI_OPTION_FACADE12
    );
    println!(
        "  {:<32} Write an explicit FAE 1.0 ISA descriptor",
        "--isa <u32>"
    );
    println!(
        "  {:<32} Append one ISA descriptor word",
        "--isa-word <u32>"
    );
    println!(
        "  {:<32} Write an explicit FAE 1.0 ABI descriptor",
        "--abi <u32>"
    );
    println!(
        "  {:<32} Append one ABI descriptor word",
        "--abi-word <u32>"
    );
    println!(
        "  {:<32} Rustlet application profile alias",
        "-rs, --rustlet"
    );
    println!(
        "  {:<32} Rustlet Security Domain profile alias",
        "-sd, --securitydomain"
    );
    println!(
        "  {:<32} Rustlet stack requirement in bytes",
        "--stack-size <bytes>"
    );
    println!("                                 Default: {DEFAULT_RUSTLET_STACK_SIZE}");
    println!(
        "  {:<32} Downgrade an RT0/payload instruction-mode mismatch to a warning",
        CLI_OPTION_ALLOW_RT0_MISMATCH
    );
    println!(
        "  {:<32} Dump detailed relocation export logs",
        format!("{}, {}", CLI_OPTION_VERBOSE, CLI_OPTION_VERBOSE_LONG)
    );
    println!(
        "  {:<32} Show this help",
        format!("{}, {}", CLI_OPTION_HELP, CLI_OPTION_HELP_LONG)
    );
}

fn usage_error(argv0: &str, message: &str) -> Result<BuildOptions, i32> {
    eprintln!("error: {message}");
    usage(argv0);
    Err(1)
}

fn parse_firmware_board_arg(value: &str) -> Result<FirmwareBoard, &'static str> {
    parse_firmware_board(value).map_err(|_| {
        "expected 'mps2-an385', 'mps2-an386', 'olimex-stm32-h405', or 'b-l475e-iot01a'"
    })
}

impl StartupKind {
    fn embedded(self) -> EmbeddedStartup {
        EmbeddedStartup::Runtime(self)
    }
}

impl FirmwareBoard {
    fn embedded(self) -> EmbeddedStartup {
        EmbeddedStartup::Firmware(self)
    }

    fn flash_base(self) -> u32 {
        match self {
            Self::Mps2An385 | Self::Mps2An386 => 0x0000_0000,
            Self::OlimexStm32H405 | Self::BL475eIot01a => 0x0800_0000,
        }
    }

    fn ram_base(self) -> u32 {
        match self {
            Self::Mps2An385 | Self::Mps2An386 | Self::OlimexStm32H405 | Self::BL475eIot01a => {
                0x2000_0000
            }
        }
    }
}

impl EmbeddedStartup {
    fn spec(self) -> EmbeddedStartupSpec {
        match self {
            Self::Runtime(StartupKind::Thumb) => EmbeddedStartupSpec {
                output_stem: "rt0-thumb",
                symbol_filename: "build_fae-rt0-thumb.elf",
                generated_filename: "rt0-thumb.elf",
                fae1_symbol_filename: "build_fae-rt0-thumb-fae1.elf",
                fae1_generated_filename: "rt0-thumb-fae1.elf",
                rustlet_symbol_filename: Some("build_fae-rt0-thumb-rustlet.elf"),
                rustlet_generated_filename: Some("rt0-thumb-rustlet.elf"),
            },
            Self::Runtime(StartupKind::ThumbV6M) => EmbeddedStartupSpec {
                output_stem: "rt0-thumbv6m",
                symbol_filename: "build_fae-rt0-thumbv6m.elf",
                generated_filename: "rt0-thumbv6m.elf",
                fae1_symbol_filename: "build_fae-rt0-thumbv6m-fae1.elf",
                fae1_generated_filename: "rt0-thumbv6m-fae1.elf",
                rustlet_symbol_filename: Some("build_fae-rt0-thumbv6m-rustlet.elf"),
                rustlet_generated_filename: Some("rt0-thumbv6m-rustlet.elf"),
            },
            Self::Runtime(StartupKind::Arm) => EmbeddedStartupSpec {
                output_stem: "rt0-arm",
                symbol_filename: "build_fae-rt0-arm.elf",
                generated_filename: "rt0-arm.elf",
                fae1_symbol_filename: "build_fae-rt0-arm-fae1.elf",
                fae1_generated_filename: "rt0-arm-fae1.elf",
                rustlet_symbol_filename: Some("build_fae-rt0-arm-rustlet.elf"),
                rustlet_generated_filename: Some("rt0-arm-rustlet.elf"),
            },
            Self::Firmware(FirmwareBoard::Mps2An385) => EmbeddedStartupSpec {
                output_stem: "boot-mps2-an385",
                symbol_filename: "build_fae-boot-mps2-an385.elf",
                generated_filename: "boot-mps2-an385.elf",
                fae1_symbol_filename: "build_fae-boot-mps2-an385-fae1.elf",
                fae1_generated_filename: "boot-mps2-an385-fae1.elf",
                rustlet_symbol_filename: None,
                rustlet_generated_filename: None,
            },
            Self::Firmware(FirmwareBoard::Mps2An386) => EmbeddedStartupSpec {
                output_stem: "boot-mps2-an386",
                symbol_filename: "build_fae-boot-mps2-an386.elf",
                generated_filename: "boot-mps2-an386.elf",
                fae1_symbol_filename: "build_fae-boot-mps2-an386-fae1.elf",
                fae1_generated_filename: "boot-mps2-an386-fae1.elf",
                rustlet_symbol_filename: None,
                rustlet_generated_filename: None,
            },
            Self::Firmware(FirmwareBoard::OlimexStm32H405) => EmbeddedStartupSpec {
                output_stem: "boot-olimex-stm32-h405",
                symbol_filename: "build_fae-boot-olimex-stm32-h405.elf",
                generated_filename: "boot-olimex-stm32-h405.elf",
                fae1_symbol_filename: "build_fae-boot-olimex-stm32-h405-fae1.elf",
                fae1_generated_filename: "boot-olimex-stm32-h405-fae1.elf",
                rustlet_symbol_filename: None,
                rustlet_generated_filename: None,
            },
            Self::Firmware(FirmwareBoard::BL475eIot01a) => EmbeddedStartupSpec {
                output_stem: "boot-b-l475e-iot01a",
                symbol_filename: "build_fae-boot-b-l475e-iot01a.elf",
                generated_filename: "boot-b-l475e-iot01a.elf",
                fae1_symbol_filename: "build_fae-boot-b-l475e-iot01a-fae1.elf",
                fae1_generated_filename: "boot-b-l475e-iot01a-fae1.elf",
                rustlet_symbol_filename: None,
                rustlet_generated_filename: None,
            },
        }
    }
}

fn output_format(
    startup: &ResolvedStartup,
    payload_elf: &goblin::elf::Elf<'_>,
    runtime_entry_point: &str,
    facade12: bool,
    explicit_isa: Option<DescriptorWords>,
    abi: BuildAbi,
    stack_size: u32,
) -> Result<OutputFormat> {
    if facade12 {
        return Ok(OutputFormat::Facade12);
    }

    let isa_subgroup = match startup {
        ResolvedStartup::Embedded { spec, .. } if spec.generated_filename == "rt0-thumbv6m.elf" => {
            fae_core::fae1::registry::isa::arm::THUMB_V6M
        }
        _ => match symbol_code_mode(payload_elf, runtime_entry_point)? {
            ArmCodeMode::Arm => fae_core::fae1::registry::isa::arm::A32,
            ArmCodeMode::Thumb => fae_core::fae1::registry::isa::arm::THUMB_2,
        },
    };
    let inferred_isa = DescriptorWords::arm(isa_subgroup);
    let isa = if let Some(explicit) = explicit_isa {
        let requested = fae_core::fae1::IsaDescriptor::decode(explicit.descriptor());
        let inferred = fae_core::fae1::IsaDescriptor::decode(inferred_isa.descriptor());
        if requested.family != inferred.family || requested.subgroup != inferred.subgroup {
            eprintln!(
                "warning: explicit ISA descriptor 0x{:08X} disagrees with ELF/RT0 ISA 0x{:08X}; writing the explicit value",
                explicit.descriptor(),
                inferred_isa.descriptor()
            );
        }
        explicit
    } else {
        inferred_isa
    };
    let abi = match abi {
        BuildAbi::Xipfs => Fae1Abi::Xipfs,
        BuildAbi::Rustlet | BuildAbi::RustletSecurityDomain => Fae1Abi::Rustlet {
            kind: if abi == BuildAbi::Rustlet {
                fae_core::fae1::RustletProfileKind::Application
            } else {
                fae_core::fae1::RustletProfileKind::SecurityDomain
            },
            minimum_version: fae_core::fae1::AbiVersion {
                major: 1,
                minor: 0,
                patch: 0,
                revision: 0,
            },
            stack_size,
        },
        BuildAbi::Explicit(words) => Fae1Abi::Explicit(words),
    };
    Ok(OutputFormat::Fae1 { isa, abi })
}

impl ResolvedStartup {
    fn read_elf_bytes(&self) -> Result<Vec<u8>> {
        match self {
            Self::Embedded {
                spec,
                fae1,
                rustlet,
            } => fs::read(generated_embedded_elf_path(spec, *fae1, *rustlet)).with_context(|| {
                format!(
                    "cannot read generated embedded startup {}",
                    spec.generated_filename
                )
            }),
            Self::Path { elf_path } => {
                fs::read(elf_path).with_context(|| format!("cannot read {}", elf_path.display()))
            }
        }
    }

    fn symbol_elf_path(&self, output_dir: &Path, verbose: bool) -> Result<PathBuf> {
        match self {
            Self::Embedded {
                spec,
                fae1,
                rustlet,
            } => {
                let path = output_dir.join(if *rustlet {
                    spec.rustlet_symbol_filename
                        .expect("Rustlet startup filename")
                } else if *fae1 {
                    spec.fae1_symbol_filename
                } else {
                    spec.symbol_filename
                });
                fs::copy(generated_embedded_elf_path(spec, *fae1, *rustlet), &path)
                    .with_context(|| format!("cannot write {}", path.display()))?;
                if verbose {
                    println!("Embedded startup ELF symbol file: {}", path.display());
                }
                Ok(path)
            }
            Self::Path { elf_path } => {
                let path = absolute_path(&elf_path.to_string_lossy())?;
                Ok(path)
            }
        }
    }

    fn startup_alone_output_path(&self) -> Result<PathBuf> {
        match self {
            Self::Embedded { spec, .. } => Ok(std::env::current_dir()?.join(format!(
                "{}{}",
                spec.output_stem,
                constants::SUFFIX
            ))),
            Self::Path { elf_path } => absolute_path(&output_filename_from_input(elf_path)?),
        }
    }

    fn gdb_memory_layout(&self) -> GdbMemoryLayout {
        match self {
            Self::Embedded { spec, .. } => match spec.output_stem {
                "boot-mps2-an385" => GdbMemoryLayout::Fixed {
                    flash_base: FirmwareBoard::Mps2An385.flash_base(),
                    ram_base: FirmwareBoard::Mps2An385.ram_base(),
                },
                "boot-mps2-an386" => GdbMemoryLayout::Fixed {
                    flash_base: FirmwareBoard::Mps2An386.flash_base(),
                    ram_base: FirmwareBoard::Mps2An386.ram_base(),
                },
                "boot-olimex-stm32-h405" => GdbMemoryLayout::Fixed {
                    flash_base: FirmwareBoard::OlimexStm32H405.flash_base(),
                    ram_base: FirmwareBoard::OlimexStm32H405.ram_base(),
                },
                "boot-b-l475e-iot01a" => GdbMemoryLayout::Fixed {
                    flash_base: FirmwareBoard::BL475eIot01a.flash_base(),
                    ram_base: FirmwareBoard::BL475eIot01a.ram_base(),
                },
                _ => GdbMemoryLayout::Relocatable {
                    flash_base_hint: None,
                },
            },
            Self::Path { .. } => GdbMemoryLayout::Relocatable {
                flash_base_hint: None,
            },
        }
    }
}

fn generated_embedded_elf_path(spec: &EmbeddedStartupSpec, fae1: bool, rustlet: bool) -> PathBuf {
    PathBuf::from(env!("OUT_DIR"))
        .join("embedded-startups")
        .join(if rustlet {
            spec.rustlet_generated_filename
                .expect("Rustlet startup filename")
        } else if fae1 {
            spec.fae1_generated_filename
        } else {
            spec.generated_filename
        })
}

fn parse_startup_kind(value: &str) -> Result<StartupKind, i32> {
    match value.trim() {
        "thumb" => Ok(StartupKind::Thumb),
        "thumbv6m" => Ok(StartupKind::ThumbV6M),
        "arm" => Ok(StartupKind::Arm),
        _ => Err(1),
    }
}

fn parse_firmware_board(value: &str) -> Result<FirmwareBoard, i32> {
    match value.trim() {
        "mps2-an385" => Ok(FirmwareBoard::Mps2An385),
        "mps2-an386" => Ok(FirmwareBoard::Mps2An386),
        "olimex-stm32-h405" => Ok(FirmwareBoard::OlimexStm32H405),
        "b-l475e-iot01a" => Ok(FirmwareBoard::BL475eIot01a),
        _ => Err(1),
    }
}

fn detect_startup_kind(
    elf: &goblin::elf::Elf<'_>,
    runtime_entry_point: &str,
) -> Result<StartupKind> {
    if elf.header.e_machine != EM_ARM {
        bail!(
            "automatic startup selection only supports ARM ELF inputs for now; use {} thumb|arm",
            CLI_OPTION_RT0
        );
    }

    let entry_mode = symbol_code_mode(elf, runtime_entry_point)?;
    Ok(match entry_mode {
        ArmCodeMode::Thumb => StartupKind::Thumb,
        ArmCodeMode::Arm => StartupKind::Arm,
    })
}

fn absolute_path(path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() {
        Ok(p.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(p))
    }
}

fn export_partition(
    elf: &goblin::elf::Elf<'_>,
    elf_bytes: &[u8],
    raw_symbols: ExportedSymbols,
    padded_symbols: ExportedSymbols,
    relocation_offsets: &[u32],
    verbose: bool,
) -> Result<Vec<u8>> {
    let rom = export_section_bytes(
        elf,
        elf_bytes,
        constants::ROM_SECTION_NAME,
        raw_symbols.rom_size as usize,
        verbose,
    )?;
    let mut got = export_section_bytes(
        elf,
        elf_bytes,
        constants::GOT_SECTION_NAME,
        raw_symbols.got_size as usize,
        verbose,
    )?;
    let mut rom_ram = export_section_bytes(
        elf,
        elf_bytes,
        constants::ROM_RAM_SECTION_NAME,
        raw_symbols.rom_ram_size as usize,
        verbose,
    )?;

    if got.len() % 4 != 0 {
        bail!(".got size must be a multiple of 4 bytes");
    }

    let (got_words, remainder) = got.as_chunks_mut::<4>();
    debug_assert!(remainder.is_empty());
    for word in got_words {
        let raw = u32::from_le_bytes(*word);
        let mapped = remap_offset_to_padded_layout(raw, raw_symbols, padded_symbols)
            .ok_or_else(|| anyhow!("cannot remap .got entry offset {}", raw))?;
        *word = mapped.to_le_bytes();
    }

    let raw_rom_start = raw_symbols.rom_size;
    let raw_got_start = raw_rom_start + raw_symbols.got_size;
    let raw_rom_ram_end = raw_got_start + raw_symbols.rom_ram_size;
    for &raw_ptr_off in relocation_offsets {
        if raw_ptr_off < raw_got_start || raw_ptr_off >= raw_rom_ram_end {
            continue;
        }
        let local = (raw_ptr_off - raw_got_start) as usize;
        if local + 4 > rom_ram.len() {
            bail!("relocation entry {} extends beyond .rom.ram", raw_ptr_off);
        }
        let raw = u32::from_le_bytes([
            rom_ram[local],
            rom_ram[local + 1],
            rom_ram[local + 2],
            rom_ram[local + 3],
        ]);
        let mapped = remap_offset_to_padded_layout(raw, raw_symbols, padded_symbols)
            .ok_or_else(|| anyhow!("cannot remap .rom.ram offset {}", raw))?;
        rom_ram[local..local + 4].copy_from_slice(&mapped.to_le_bytes());
    }

    got.resize(
        padded_symbols.got_size as usize,
        constants::TEXT_ALIGNMENT_PADDING_VALUE,
    );
    rom_ram.resize(
        padded_symbols.rom_ram_size as usize,
        constants::TEXT_ALIGNMENT_PADDING_VALUE,
    );
    let mut rom = rom;
    rom.resize(
        padded_symbols.rom_size as usize,
        constants::TEXT_ALIGNMENT_PADDING_VALUE,
    );

    let mut partition = Vec::with_capacity(
        padded_symbols.rom_size as usize
            + padded_symbols.got_size as usize
            + padded_symbols.rom_ram_size as usize,
    );
    partition.extend_from_slice(&rom);
    partition.extend_from_slice(&got);
    partition.extend_from_slice(&rom_ram);
    if verbose {
        println!("Export partition : {} bytes", partition.len());
    }
    Ok(partition)
}

fn parse_args(args: &[String]) -> Result<BuildOptions, i32> {
    if args.len() == 2 && (args[1] == CLI_OPTION_HELP || args[1] == CLI_OPTION_HELP_LONG) {
        usage(&args[0]);
        return Err(0);
    }

    let mut startup_source = StartupSource::EmbeddedDefaultRuntime;
    let mut startup_selection = StartupSelection::Auto;
    let mut has_explicit_startup_source = false;
    let mut startup_alone = false;
    let mut runtime_entry_point = constants::DEFAULT_RUNTIME_ENTRY_POINT.to_string();
    let mut ram_size = constants::DEFAULT_STARTUP_ALONE_RAM_SIZE;
    let mut align_payload = 0usize;
    let mut align_size = constants::PADDING_MPU_ALIGNMENT;
    let mut facade12 = false;
    let mut isa_descriptor = None;
    let mut isa_words = Vec::new();
    let mut abi = BuildAbi::Xipfs;
    let mut abi_descriptor = None;
    let mut abi_words = Vec::new();
    let mut stack_size = DEFAULT_RUSTLET_STACK_SIZE;
    let mut allow_rt0_mismatch = false;
    let mut verbose = false;
    let mut positional = Vec::new();

    let mut i = 1usize;
    while i < args.len() {
        match args[i].as_str() {
            CLI_OPTION_RT0_SHORT | CLI_OPTION_RT0 => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--rt0 requires a value");
                }
                if has_explicit_startup_source {
                    return usage_error(
                        &args[0],
                        "--rt0 and --firmware are mutually exclusive and may only be specified once",
                    );
                }
                let value = args[i + 1].trim();
                if let Ok(kind) = parse_startup_kind(value) {
                    startup_selection = StartupSelection::Explicit(kind);
                } else if Path::new(value).is_file() {
                    startup_source = StartupSource::ExplicitElf(value.to_owned());
                } else {
                    return usage_error(
                        &args[0],
                        "--rt0 expects thumb, thumbv6m, arm, or an existing RT0 ELF file",
                    );
                }
                has_explicit_startup_source = true;
                i += 2;
            }
            arg if arg.starts_with("--rt0=") || arg.starts_with("-rt0=") => {
                let value = arg.split_once('=').map(|(_, value)| value).unwrap_or("");
                if has_explicit_startup_source {
                    return usage_error(
                        &args[0],
                        "--rt0 and --firmware are mutually exclusive and may only be specified once",
                    );
                }
                if let Ok(kind) = parse_startup_kind(value) {
                    startup_selection = StartupSelection::Explicit(kind);
                } else if Path::new(value).is_file() {
                    startup_source = StartupSource::ExplicitElf(value.to_owned());
                } else {
                    return usage_error(
                        &args[0],
                        "--rt0 expects thumb, thumbv6m, arm, or an existing RT0 ELF file",
                    );
                }
                has_explicit_startup_source = true;
                i += 1;
            }
            CLI_OPTION_FIRMWARE => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--firmware requires a value");
                }
                if has_explicit_startup_source {
                    return usage_error(
                        &args[0],
                        &format!(
                            "{} cannot be combined with {}",
                            CLI_OPTION_FIRMWARE, CLI_OPTION_RT0
                        ),
                    );
                }
                startup_source = StartupSource::EmbeddedFirmware(
                    parse_firmware_board_arg(&args[i + 1]).map_err(|msg| {
                        usage_error(&args[0], &format!("{} {}", CLI_OPTION_FIRMWARE, msg))
                            .err()
                            .unwrap()
                    })?,
                );
                has_explicit_startup_source = true;
                i += 2;
            }
            arg if arg.starts_with("--firmware=") => {
                if has_explicit_startup_source {
                    return usage_error(
                        &args[0],
                        &format!(
                            "{} cannot be combined with {}",
                            CLI_OPTION_FIRMWARE, CLI_OPTION_RT0
                        ),
                    );
                }
                startup_source = StartupSource::EmbeddedFirmware(
                    parse_firmware_board_arg(&arg["--firmware=".len()..]).map_err(|msg| {
                        usage_error(&args[0], &format!("{} {}", CLI_OPTION_FIRMWARE, msg))
                            .err()
                            .unwrap()
                    })?,
                );
                has_explicit_startup_source = true;
                i += 1;
            }
            CLI_OPTION_RUNTIME_ENTRY_POINT => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--runtime-entry-point requires a value");
                }
                runtime_entry_point = args[i + 1].trim().to_string();
                i += 2;
            }
            CLI_OPTION_STARTUP_ALONE => {
                startup_alone = true;
                i += 1;
            }
            CLI_OPTION_RAM_SIZE => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--ram-size requires a value");
                }
                ram_size = args[i + 1].trim().parse().map_err(|_| {
                    usage_error(&args[0], "--ram-size expects an integer")
                        .err()
                        .unwrap()
                })?;
                i += 2;
            }

            CLI_OPTION_ALIGN_PAYLOAD => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--align_payload requires a value");
                }
                align_payload = parse_alignment(&args[0], "--align_payload", &args[i + 1])?;
                i += 2;
            }
            arg if arg.starts_with("--align_payload=") => {
                align_payload = parse_alignment(
                    &args[0],
                    "--align_payload",
                    &arg["--align_payload=".len()..],
                )?;
                i += 1;
            }
            CLI_OPTION_ALIGN_SIZE => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--align_size requires a value");
                }
                align_size = parse_alignment(&args[0], "--align_size", &args[i + 1])?;
                i += 2;
            }
            arg if arg.starts_with("--align_size=") => {
                align_size =
                    parse_alignment(&args[0], "--align_size", &arg["--align_size=".len()..])?;
                i += 1;
            }
            CLI_OPTION_FACADE12 => {
                facade12 = true;
                i += 1;
            }
            CLI_OPTION_ISA => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--isa requires a value");
                }
                isa_descriptor = Some(parse_u32_arg(&args[0], "--isa", &args[i + 1])?);
                i += 2;
            }
            arg if arg.starts_with("--isa=") => {
                isa_descriptor = Some(parse_u32_arg(&args[0], "--isa", &arg["--isa=".len()..])?);
                i += 1;
            }
            CLI_OPTION_ISA_WORD => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--isa-word requires a value");
                }
                isa_words.push(parse_u32_arg(&args[0], "--isa-word", &args[i + 1])?);
                i += 2;
            }
            arg if arg.starts_with("--isa-word=") => {
                isa_words.push(parse_u32_arg(
                    &args[0],
                    "--isa-word",
                    &arg["--isa-word=".len()..],
                )?);
                i += 1;
            }
            CLI_OPTION_ABI => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--abi requires a value");
                }
                if abi != BuildAbi::Xipfs {
                    return usage_error(&args[0], "--abi cannot be combined with a profile alias");
                }
                abi_descriptor = Some(parse_u32_arg(&args[0], "--abi", &args[i + 1])?);
                i += 2;
            }
            arg if arg.starts_with("--abi=") => {
                if abi != BuildAbi::Xipfs {
                    return usage_error(&args[0], "--abi cannot be combined with a profile alias");
                }
                abi_descriptor = Some(parse_u32_arg(&args[0], "--abi", &arg["--abi=".len()..])?);
                i += 1;
            }
            CLI_OPTION_ABI_WORD => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--abi-word requires a value");
                }
                abi_words.push(parse_u32_arg(&args[0], "--abi-word", &args[i + 1])?);
                i += 2;
            }
            arg if arg.starts_with("--abi-word=") => {
                abi_words.push(parse_u32_arg(
                    &args[0],
                    "--abi-word",
                    &arg["--abi-word=".len()..],
                )?);
                i += 1;
            }
            CLI_OPTION_RUSTLET | CLI_OPTION_RUSTLET_SHORT => {
                if abi_descriptor.is_some() || abi != BuildAbi::Xipfs {
                    return usage_error(
                        &args[0],
                        "--rustlet cannot be combined with another ABI profile",
                    );
                }
                abi = BuildAbi::Rustlet;
                i += 1;
            }
            CLI_OPTION_SECURITY_DOMAIN | CLI_OPTION_SECURITY_DOMAIN_SHORT => {
                if abi_descriptor.is_some() || abi != BuildAbi::Xipfs {
                    return usage_error(
                        &args[0],
                        "--securitydomain cannot be combined with another ABI profile",
                    );
                }
                abi = BuildAbi::RustletSecurityDomain;
                i += 1;
            }
            CLI_OPTION_STACK_SIZE => {
                if i + 1 >= args.len() {
                    return usage_error(&args[0], "--stack-size requires a value");
                }
                stack_size = args[i + 1].parse().map_err(|_| {
                    usage_error(&args[0], "--stack-size expects an integer")
                        .err()
                        .unwrap()
                })?;
                i += 2;
            }
            CLI_OPTION_ALLOW_RT0_MISMATCH => {
                allow_rt0_mismatch = true;
                i += 1;
            }
            CLI_OPTION_VERBOSE | CLI_OPTION_VERBOSE_LONG => {
                verbose = true;
                i += 1;
            }
            arg if arg.starts_with('-') => {
                return usage_error(&args[0], &format!("unrecognized option: {arg}"));
            }
            arg => {
                positional.push(arg.to_string());
                i += 1;
            }
        }
    }

    let isa = match isa_descriptor {
        Some(descriptor) => Some(descriptor_words(&args[0], "ISA", descriptor, &isa_words)?),
        None if isa_words.is_empty() => None,
        None => return usage_error(&args[0], "--isa-word requires --isa"),
    };
    if let Some(descriptor) = abi_descriptor {
        abi = BuildAbi::Explicit(descriptor_words(&args[0], "ABI", descriptor, &abi_words)?);
    } else if !abi_words.is_empty() {
        return usage_error(&args[0], "--abi-word requires --abi");
    }

    if runtime_entry_point.is_empty() {
        return usage_error(&args[0], "runtime entry point cannot be empty");
    }
    if facade12 && (abi != BuildAbi::Xipfs || isa.is_some()) {
        return usage_error(
            &args[0],
            "--facade12 cannot be combined with FAE 1.0 ISA/ABI options",
        );
    }
    if startup_alone && abi != BuildAbi::Xipfs {
        return usage_error(&args[0], "Rustlet ABI profiles require an input ELF");
    }

    if matches!(startup_source, StartupSource::EmbeddedFirmware(_))
        && !matches!(startup_selection, StartupSelection::Auto)
    {
        return usage_error(
            &args[0],
            &format!(
                "{} cannot be combined with {}",
                CLI_OPTION_FIRMWARE, CLI_OPTION_RT0
            ),
        );
    }

    if startup_alone {
        if !positional.is_empty() {
            return usage_error(
                &args[0],
                &format!(
                    "{} cannot be used with an input ELF",
                    CLI_OPTION_STARTUP_ALONE
                ),
            );
        }
        if runtime_entry_point != constants::DEFAULT_RUNTIME_ENTRY_POINT {
            return usage_error(
                &args[0],
                &format!(
                    "{} cannot be combined with {}",
                    CLI_OPTION_STARTUP_ALONE, CLI_OPTION_RUNTIME_ENTRY_POINT
                ),
            );
        }
    } else {
        if positional.len() != 1 {
            return usage_error(&args[0], "expected exactly one input ELF");
        }
        if ram_size != constants::DEFAULT_STARTUP_ALONE_RAM_SIZE {
            return usage_error(
                &args[0],
                &format!(
                    "{} requires {}",
                    CLI_OPTION_RAM_SIZE, CLI_OPTION_STARTUP_ALONE
                ),
            );
        }
    }

    let mode = if startup_alone {
        BuildMode::StartupAlone
    } else {
        BuildMode::Application {
            elf_filename: positional[0].trim().to_string(),
        }
    };

    Ok(BuildOptions {
        startup_source,
        startup_selection,
        mode,
        runtime_entry_point,
        ram_size,
        align_payload,
        align_size,
        facade12,
        isa,
        abi,
        stack_size,
        allow_rt0_mismatch,
        verbose,
    })
}

fn parse_u32_arg(argv0: &str, option: &str, value: &str) -> Result<u32, i32> {
    let compact = value.trim().replace('_', "");
    let parsed = compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
        .map(|hex| u32::from_str_radix(hex, 16))
        .unwrap_or_else(|| compact.parse::<u32>());
    parsed.map_err(|_| {
        usage_error(
            argv0,
            &format!("{option} expects a 32-bit decimal or 0x-prefixed value"),
        )
        .err()
        .unwrap()
    })
}

fn descriptor_words(
    argv0: &str,
    kind: &str,
    descriptor: u32,
    words: &[u32],
) -> Result<DescriptorWords, i32> {
    DescriptorWords::new(descriptor, words).map_err(|err| {
        usage_error(argv0, &format!("invalid {kind} descriptor: {err}"))
            .err()
            .unwrap()
    })
}
fn parse_alignment(argv0: &str, option: &str, value: &str) -> Result<usize, i32> {
    let align = value.trim().parse::<usize>().map_err(|_| {
        usage_error(argv0, &format!("{option} expects an integer"))
            .err()
            .unwrap()
    })?;
    if align > 1 && !align.is_power_of_two() {
        return Err(
            usage_error(argv0, &format!("{option} expects 0, 1, or a power of two"))
                .err()
                .unwrap(),
        );
    }
    Ok(align)
}

fn resolve_startup_elf_path(source: &StartupSource) -> Result<PathBuf> {
    match source {
        StartupSource::EmbeddedDefaultRuntime | StartupSource::EmbeddedFirmware(_) => {
            bail!("internal error: embedded startup should not resolve via a source tree path")
        }
        StartupSource::ExplicitElf(path) => absolute_path(path),
    }
}

fn resolve_startup(
    source: &StartupSource,
    selection: StartupSelection,
    elf: Option<&goblin::elf::Elf<'_>>,
    runtime_entry_point: &str,
    facade12: bool,
    abi: BuildAbi,
) -> Result<ResolvedStartup> {
    match source {
        StartupSource::EmbeddedDefaultRuntime => {
            let kind = match selection {
                StartupSelection::Auto => match elf {
                    Some(elf) => detect_startup_kind(elf, runtime_entry_point)?,
                    None => StartupKind::Thumb,
                },
                StartupSelection::Explicit(kind) => kind,
            };
            Ok(ResolvedStartup::Embedded {
                spec: kind.embedded().spec(),
                fae1: !facade12,
                rustlet: !facade12 && abi.uses_rustlet_layout(),
            })
        }
        StartupSource::EmbeddedFirmware(board) => Ok(ResolvedStartup::Embedded {
            spec: board.embedded().spec(),
            fae1: !facade12,
            rustlet: false,
        }),
        StartupSource::ExplicitElf(_) => {
            match selection {
                StartupSelection::Auto => {
                    if let Some(elf) = elf {
                        let _ = detect_startup_kind(elf, runtime_entry_point)?;
                    }
                }
                StartupSelection::Explicit(_) => {}
            }
            Ok(ResolvedStartup::Path {
                elf_path: resolve_startup_elf_path(source)?,
            })
        }
    }
}

fn output_filename_from_input(path: &Path) -> Result<String> {
    let mut output = path.to_path_buf();
    if let Some(stem) = path.file_stem() {
        output.set_file_name(format!("{}{}", stem.to_string_lossy(), constants::SUFFIX));
        return Ok(output.to_string_lossy().into_owned());
    }
    bail!("invalid input path {}", path.display())
}

fn gdbinit_filename_from_fae(path: &Path) -> Result<PathBuf> {
    let mut output = path.to_path_buf();
    if let Some(stem) = path.file_stem() {
        output.set_file_name(format!(
            "{}{}",
            stem.to_string_lossy(),
            constants::GDBINIT_SUFFIX
        ));
        return Ok(output);
    }
    bail!("invalid output path {}", path.display())
}

pub fn run(args: &[String]) -> Result<(), i32> {
    let options = parse_args(args)?;
    if let Err(e) = run_inner(&options) {
        eprintln!("\x1b[91;1m{}: {}\x1b[0m", args[0], e);
        return Err(1);
    }
    Ok(())
}

fn run_inner(options: &BuildOptions) -> Result<()> {
    match &options.mode {
        BuildMode::Application { elf_filename } => run_application_build(elf_filename, options),
        BuildMode::StartupAlone => {
            let startup = resolve_startup(
                &options.startup_source,
                options.startup_selection,
                None,
                &options.runtime_entry_point,
                options.facade12,
                options.abi,
            )?;
            let startup_elf_bytes = startup.read_elf_bytes()?;
            let startup_elf = parse_elf(&startup_elf_bytes)?;
            let startup_code =
                export_startup_code(&startup_elf, &startup_elf_bytes, options.verbose)?;
            run_startup_alone_build(
                &startup,
                &startup_elf,
                &startup_code,
                options.ram_size,
                options.align_payload,
                options.align_size,
                options.facade12,
                options.verbose,
            )
        }
    }
}
fn run_application_build(elf_filename: &str, options: &BuildOptions) -> Result<()> {
    let output_filename = output_filename_from_input(Path::new(elf_filename))?;
    let gdbinit_filename = gdbinit_filename_from_fae(Path::new(&output_filename))?;

    let elf_bytes =
        fs::read(elf_filename).with_context(|| format!("cannot read {}", elf_filename))?;
    let elf = parse_elf(&elf_bytes)?;
    let startup = resolve_startup(
        &options.startup_source,
        options.startup_selection,
        Some(&elf),
        &options.runtime_entry_point,
        options.facade12,
        options.abi,
    )?;
    let startup_elf_bytes = startup.read_elf_bytes()?;
    let startup_elf = parse_elf(&startup_elf_bytes)?;
    let startup_code = export_startup_code(&startup_elf, &startup_elf_bytes, options.verbose)?;
    validate_abs32_relocation_targets(&elf, &elf_bytes)?;
    validate_no_exec_relocations_to_runtime_writable_sections(&elf, &elf_bytes)?;

    validate_rt0_payload_compatibility(
        &startup_elf,
        &elf,
        &options.runtime_entry_point,
        options.allow_rt0_mismatch,
    )?;
    let raw_exported_symbols =
        export_symbols_to_struct(&elf, &options.runtime_entry_point, options.verbose)?;
    let mut exported_symbols = padded_exported_symbols(raw_exported_symbols);
    let start_mode_bit = raw_exported_symbols.start & 1;
    exported_symbols.start = remap_offset_to_padded_layout(
        raw_exported_symbols.start & !1,
        raw_exported_symbols,
        exported_symbols,
    )
    .unwrap_or(raw_exported_symbols.start & !1)
        | start_mode_bit;

    let mut relocation_bytearray = Vec::new();
    let mut writable_relocation_offsets = Vec::new();
    for name in constants::EXPORTED_RELOCATION_TABLES {
        let (chunk, raw_offsets) = export_relocation_table(
            &elf,
            &elf_bytes,
            name,
            raw_exported_symbols,
            exported_symbols,
            options.verbose,
        )?;
        relocation_bytearray.extend_from_slice(&chunk);
        writable_relocation_offsets.extend_from_slice(&raw_offsets);
    }
    let format = output_format(
        &startup,
        &elf,
        &options.runtime_entry_point,
        options.facade12,
        options.isa,
        options.abi,
        options.stack_size,
    )?;
    let metadata_size = compute_aligned_header_size_for_format(
        &startup_code,
        &relocation_bytearray,
        options.align_payload,
        format,
    );

    let partition_bytearray = export_partition(
        &elf,
        &elf_bytes,
        raw_exported_symbols,
        exported_symbols,
        &writable_relocation_offsets,
        options.verbose,
    )?;
    if options.verbose {
        println!("Offset to partition : {} bytes", metadata_size);
    }

    let array_of_bytes = concatenate_and_pad_bytearray_for_format(
        &startup_code,
        exported_symbols,
        &relocation_bytearray,
        options.align_payload,
        options.align_size,
        &partition_bytearray,
        format,
    )?;

    let output_dir = Path::new(&output_filename)
        .parent()
        .unwrap_or(Path::new("."));
    let startup_symbol_path = startup.symbol_elf_path(output_dir, options.verbose)?;
    generate_gdbinit(
        Some(elf_filename),
        &startup_symbol_path,
        &gdbinit_filename,
        metadata_size,
        Some(exported_symbols),
        exported_symbols.ram_size,
        startup.gdb_memory_layout(),
        options.verbose,
    )?;

    fs::write(&output_filename, array_of_bytes)
        .with_context(|| format!("cannot write {}", output_filename))?;
    if options.verbose {
        println!(
            "{} has been generated.",
            absolute_path(&output_filename)?.display()
        );
    } else {
        println!("Required RAM : {} bytes", exported_symbols.ram_size);
        println!(
            "Total flash image : {} bytes",
            metadata_size + partition_bytearray.len()
        );
    }

    let mut debug_elf = absolute_path(elf_filename)?;
    debug_elf.set_file_name("debug_elf");
    let status = std::process::Command::new("arm-none-eabi-objcopy")
        .arg("--rename-section")
        .arg(".rom=.text,alloc,load,readonly,code,contents")
        .arg(absolute_path(elf_filename)?)
        .arg(&debug_elf)
        .status()?;

    if !status.success() {
        eprintln!("objcopy failed with status: {}", status);
        std::process::exit(1);
    }

    println!(
        "Replaced .rom with .text for gdb in {}",
        debug_elf.display()
    );

    Ok(())
}

fn validate_rt0_payload_compatibility(
    rt0_elf: &goblin::elf::Elf<'_>,
    payload_elf: &goblin::elf::Elf<'_>,
    runtime_entry_point: &str,
    allow_mismatch: bool,
) -> Result<()> {
    if rt0_elf.header.e_machine != payload_elf.header.e_machine {
        bail!(
            "RT0 ELF machine {} is incompatible with payload ELF machine {}",
            rt0_elf.header.e_machine,
            payload_elf.header.e_machine
        );
    }
    if rt0_elf.header.e_machine != EM_ARM {
        bail!("RT0/payload compatibility validation currently supports ARM ELF only");
    }

    let rt0_mode = symbol_code_mode(rt0_elf, constants::STARTUP_SYMBOL_NAME)?;
    let payload_entry = export_symbol_value(payload_elf, runtime_entry_point)?;
    let payload_mode = ArmCodeMode::from_symbol_value(payload_entry);
    if rt0_mode != payload_mode {
        let message = format!(
            "RT0 entry {} uses {} instructions but payload entry {} uses {} instructions",
            constants::STARTUP_SYMBOL_NAME,
            rt0_mode.as_str(),
            runtime_entry_point,
            payload_mode.as_str()
        );
        if allow_mismatch {
            eprintln!("warning: {message}; producing the FAE because {CLI_OPTION_ALLOW_RT0_MISMATCH} was specified");
        } else {
            bail!("{message}; use {CLI_OPTION_ALLOW_RT0_MISMATCH} to downgrade this error to a warning and produce the FAE anyway");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_startup_alone_build(
    startup: &ResolvedStartup,
    startup_elf: &goblin::elf::Elf<'_>,
    startup_code: &[u8],
    ram_size: u32,
    align_payload: usize,
    align_size: usize,
    facade12: bool,
    verbose: bool,
) -> Result<()> {
    let output_filename = startup.startup_alone_output_path()?;
    let gdbinit_filename = gdbinit_filename_from_fae(&output_filename)?;
    let relocation_bytearray = (0u32).to_le_bytes().to_vec();
    let exported_symbols = ExportedSymbols {
        start: 0,
        rom_ram_size: 0,
        rom_size: 0,
        got_size: 0,
        ram_size,
    };
    let format = output_format(
        startup,
        startup_elf,
        constants::DEFAULT_RUNTIME_ENTRY_POINT,
        facade12,
        None,
        BuildAbi::Xipfs,
        DEFAULT_RUSTLET_STACK_SIZE,
    )?;
    let metadata_size = compute_aligned_header_size_for_format(
        startup_code,
        &relocation_bytearray,
        align_payload,
        format,
    );
    let array_of_bytes = concatenate_and_pad_bytearray_for_format(
        startup_code,
        exported_symbols,
        &relocation_bytearray,
        align_payload,
        align_size,
        &[],
        format,
    )?;

    let output_dir = output_filename.parent().unwrap_or(Path::new("."));
    let startup_symbol_path = startup.symbol_elf_path(output_dir, verbose)?;
    generate_gdbinit(
        None,
        &startup_symbol_path,
        &gdbinit_filename,
        metadata_size,
        None,
        ram_size,
        startup.gdb_memory_layout(),
        verbose,
    )?;

    fs::write(&output_filename, array_of_bytes)
        .with_context(|| format!("cannot write {}", output_filename.display()))?;
    if verbose {
        println!("{} has been generated.", output_filename.display());
    } else {
        println!("Required RAM : {} bytes", ram_size);
        println!("Total flash image : {} bytes", metadata_size);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rt0_option_accepts_an_embedded_kind_or_existing_elf() {
        for option in [CLI_OPTION_RT0_SHORT, CLI_OPTION_RT0] {
            let path = std::env::temp_dir().join(format!(
                "fae-build-rt0-option-{}-{option}.elf",
                std::process::id()
            ));
            fs::write(&path, []).expect("create RT0 option fixture");
            let args = vec![
                "build_fae".to_owned(),
                option.to_owned(),
                path.to_string_lossy().into_owned(),
                "payload.elf".to_owned(),
            ];
            let options = parse_args(&args).expect("explicit RT0 option must parse");

            assert!(matches!(
                options.startup_source,
                StartupSource::ExplicitElf(ref parsed) if parsed == path.to_string_lossy().as_ref()
            ));
            assert!(matches!(
                options.mode,
                BuildMode::Application { ref elf_filename } if elf_filename == "payload.elf"
            ));
            fs::remove_file(path).expect("remove RT0 option fixture");
        }

        let args = vec![
            "build_fae".to_owned(),
            CLI_OPTION_RT0.to_owned(),
            "thumbv6m".to_owned(),
            "payload.elf".to_owned(),
        ];
        let options = parse_args(&args).expect("embedded RT0 kind must parse");
        assert!(matches!(
            options.startup_selection,
            StartupSelection::Explicit(StartupKind::ThumbV6M)
        ));
    }

    #[test]
    fn rt0_option_rejects_an_unknown_kind_or_missing_file() {
        let args = vec![
            "build_fae".to_owned(),
            CLI_OPTION_RT0.to_owned(),
            "not-an-rt0".to_owned(),
            "payload.elf".to_owned(),
        ];
        assert!(parse_args(&args).is_err());
    }
}
