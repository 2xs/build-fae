use std::collections::BTreeMap;
use std::fs;

use goblin::elf::section_header::{SHT_REL, SHT_RELA};

use fae_core::constants;
use fae_elf::elf::parse_elf;

#[derive(Debug, Clone)]
struct RelocationRequest {
    relocation_section_name: String,
    relocation_kind: &'static str,
    entry_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelocationFilter {
    Any,
    ArmAbs32Only,
}

fn usage(argv0: &str) {
    println!("Usage:");
    println!("  {argv0} <file.elf>");
    println!("  {argv0} --abs32-only <file.elf>");
    println!("  {argv0} --help");
    println!();
    println!("List ELF sections that are targeted by relocation sections.");
}

fn usage_error(argv0: &str, message: &str) -> Result<(), i32> {
    eprintln!("error: {message}");
    usage(argv0);
    Err(1)
}

fn count_matching_relocations(
    elf_bytes: &[u8],
    section_offset: u64,
    section_size: u64,
    entry_size: u64,
    filter: RelocationFilter,
) -> Result<usize, i32> {
    let entry_size = entry_size as usize;
    if entry_size == 0 {
        return Ok(0);
    }

    let start = section_offset as usize;
    let size = section_size as usize;
    let end = start.checked_add(size).ok_or_else(|| {
        eprintln!("check_elf_rel: relocation section end overflow");
        1
    })?;
    if end > elf_bytes.len() {
        eprintln!("check_elf_rel: relocation section extends beyond file bounds");
        return Err(1);
    }

    let mut matches = 0usize;
    for off in (start..end).step_by(entry_size) {
        if off + 8 > elf_bytes.len() {
            eprintln!("check_elf_rel: truncated relocation entry");
            return Err(1);
        }
        let r_info = u32::from_le_bytes([
            elf_bytes[off + 4],
            elf_bytes[off + 5],
            elf_bytes[off + 6],
            elf_bytes[off + 7],
        ]);
        let r_type = r_info & 0xff;
        let keep = match filter {
            RelocationFilter::Any => true,
            RelocationFilter::ArmAbs32Only => r_type == constants::R_ARM_ABS32,
        };
        if keep {
            matches += 1;
        }
    }

    Ok(matches)
}

pub fn run(args: &[String]) -> Result<(), i32> {
    let (filter, dump_entries, filename) = match args {
        [argv0, help] if help == "-h" || help == "--help" => {
            usage(&args[0]);
            return Ok(());
        }
        [_argv0, filename] => (RelocationFilter::Any, false, filename.as_str()),
        [_argv0, option, filename] if option == "--abs32-only" => {
            (RelocationFilter::ArmAbs32Only, false, filename.as_str())
        }
        [_argv0, option, filename] if option == "--dump-abs32" => {
            (RelocationFilter::ArmAbs32Only, true, filename.as_str())
        }
        [argv0, ..] => {
            return usage_error(
                argv0,
                "expected <file.elf>, --abs32-only <file.elf>, or --dump-abs32 <file.elf>",
            );
        }
        [] => return Err(1),
    };

    let elf_bytes = fs::read(filename).map_err(|e| {
        eprintln!("{}: {}", args[0], e);
        1
    })?;
    let elf = parse_elf(&elf_bytes).map_err(|e| {
        eprintln!("{}: {}", args[0], e);
        1
    })?;

    let mut grouped: BTreeMap<String, Vec<RelocationRequest>> = BTreeMap::new();

    for section in &elf.section_headers {
        let relocation_kind = match section.sh_type {
            SHT_REL => "REL",
            SHT_RELA => "RELA",
            _ => continue,
        };

        let relocation_section_name = elf
            .shdr_strtab
            .get_at(section.sh_name)
            .unwrap_or("<unnamed relocation section>")
            .to_string();
        let entry_count = count_matching_relocations(
            &elf_bytes,
            section.sh_offset,
            section.sh_size,
            section.sh_entsize,
            filter,
        )?;
        if entry_count == 0 {
            continue;
        }

        let target_index = section.sh_info as usize;
        let target_name = elf
            .section_headers
            .get(target_index)
            .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name))
            .map(str::to_string)
            .unwrap_or_else(|| format!("<invalid target section {}>", target_index));

        grouped
            .entry(target_name)
            .or_default()
            .push(RelocationRequest {
                relocation_section_name,
                relocation_kind,
                entry_count,
            });
    }

    println!("- ELF : {}", filename);
    if filter == RelocationFilter::ArmAbs32Only {
        println!("- Filter : R_ARM_ABS32 only");
    }
    if grouped.is_empty() {
        println!("- Sections with relocation requests : none.");
        return Ok(());
    }

    let total_relocation_sections: usize = grouped.values().map(Vec::len).sum();
    println!(
        "- Sections with relocation requests : {} target sections via {} relocation sections",
        grouped.len(),
        total_relocation_sections
    );

    for (target_name, requests) in grouped {
        println!("- {}", target_name);
        for request in requests {
            println!(
                "  {} ({}, {} entries)",
                request.relocation_section_name, request.relocation_kind, request.entry_count
            );
        }
    }

    if dump_entries {
        for section in &elf.section_headers {
            let relocation_kind = match section.sh_type {
                SHT_REL => "REL",
                SHT_RELA => "RELA",
                _ => continue,
            };
            let relocation_section_name = elf
                .shdr_strtab
                .get_at(section.sh_name)
                .unwrap_or("<unnamed relocation section>");
            let entry_size = section.sh_entsize as usize;
            if entry_size == 0 {
                continue;
            }
            let start = section.sh_offset as usize;
            let end = start + section.sh_size as usize;
            let target_name = elf
                .section_headers
                .get(section.sh_info as usize)
                .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                .unwrap_or("<invalid target>");

            println!(
                "- Dump {} -> {} ({})",
                relocation_section_name, target_name, relocation_kind
            );
            for off in (start..end).step_by(entry_size) {
                let r_offset = u32::from_le_bytes([
                    elf_bytes[off],
                    elf_bytes[off + 1],
                    elf_bytes[off + 2],
                    elf_bytes[off + 3],
                ]);
                let r_info = u32::from_le_bytes([
                    elf_bytes[off + 4],
                    elf_bytes[off + 5],
                    elf_bytes[off + 6],
                    elf_bytes[off + 7],
                ]);
                let r_type = r_info & 0xff;
                if filter == RelocationFilter::ArmAbs32Only && r_type != constants::R_ARM_ABS32 {
                    continue;
                }
                let symbol_index = (r_info >> 8) as usize;
                let symbol_name = elf
                    .syms
                    .get(symbol_index)
                    .and_then(|sym| elf.strtab.get_at(sym.st_name))
                    .unwrap_or("<no-symbol>");
                let symbol_section = elf
                    .syms
                    .get(symbol_index)
                    .and_then(|sym| elf.section_headers.get(sym.st_shndx))
                    .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                    .unwrap_or("<no-section>");
                println!(
                    "  off=0x{r_offset:08x} type={} sym={} section={}",
                    r_type, symbol_name, symbol_section
                );
            }
        }
    }

    Ok(())
}
