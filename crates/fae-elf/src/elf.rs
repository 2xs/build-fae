use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use goblin::elf::section_header::{
    SHF_ALLOC, SHF_EXECINSTR, SHT_NOBITS, SHT_REL, SHT_RELA, SHT_SYMTAB,
};
use goblin::elf::Elf;

use crate::constants;
use crate::fae_format::{remap_offset_to_padded_layout, ExportedSymbols};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmCodeMode {
    Arm,
    Thumb,
}

impl ArmCodeMode {
    pub fn from_symbol_value(value: u32) -> Self {
        if (value & 1) != 0 {
            Self::Thumb
        } else {
            Self::Arm
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Arm => "ARM",
            Self::Thumb => "Thumb",
        }
    }
}

pub fn parse_elf(bytes: &[u8]) -> Result<Elf<'_>> {
    Elf::parse(bytes).context("failed to parse ELF")
}

pub fn export_symbol_value(elf: &Elf<'_>, symbol_name: &str) -> Result<u32> {
    let _symtab = elf
        .section_headers
        .iter()
        .enumerate()
        .find(|(_, sh)| elf.shdr_strtab.get_at(sh.sh_name) == Some(".symtab"))
        .ok_or_else(|| {
            anyhow!("export_symbols_to_bytearray : .symtab : no section with this name found")
        })?
        .1;

    let mut values = Vec::new();
    for symbol in &elf.syms {
        if symbol.st_name == 0 {
            continue;
        }
        if elf.strtab.get_at(symbol.st_name) == Some(symbol_name) {
            values.push(symbol.st_value as u32);
        }
    }

    if values.is_empty() {
        bail!(
            "export_symbols_to_bytearray : .symtab : {}: no symbol with this name",
            symbol_name
        );
    }
    if values.len() > 1 {
        bail!(
            "export_symbols_to_bytearray : .symtab : {}: more than one symbol with this name",
            symbol_name
        );
    }

    Ok(values[0])
}

pub fn symbol_code_mode(elf: &Elf<'_>, symbol_name: &str) -> Result<ArmCodeMode> {
    Ok(ArmCodeMode::from_symbol_value(export_symbol_value(
        elf,
        symbol_name,
    )?))
}

pub fn export_symbols_to_struct(
    elf: &Elf<'_>,
    runtime_entry_symbol: &str,
    verbose: bool,
) -> Result<ExportedSymbols> {
    let symtab = elf
        .section_headers
        .iter()
        .enumerate()
        .find(|(_, sh)| elf.shdr_strtab.get_at(sh.sh_name) == Some(".symtab"))
        .ok_or_else(|| {
            anyhow!("export_symbols_to_bytearray : .symtab : no section with this name found")
        })?
        .1;

    if symtab.sh_type != SHT_SYMTAB {
        bail!("export_symbols_to_bytearray : .symtab : is not a SHT_SYMTAB section");
    }

    let runtime_entry_value = export_symbol_value(elf, runtime_entry_symbol)?;
    let mut found: HashMap<&str, Vec<u32>> = constants::EXPORTED_SIZE_SYMBOLS
        .iter()
        .map(|n| (*n, Vec::new()))
        .collect();

    for symbol in &elf.syms {
        if symbol.st_name == 0 {
            continue;
        }
        if let Some(name) = elf.strtab.get_at(symbol.st_name) {
            if let Some(bucket) = found.get_mut(name) {
                bucket.push(symbol.st_value as u32);
            }
        }
    }
    if verbose {
        println!(
            "Export symbol {} = {} bytes",
            runtime_entry_symbol, runtime_entry_value
        );
    }

    for symbol_name in constants::EXPORTED_SIZE_SYMBOLS {
        let values = found
            .get(symbol_name)
            .ok_or_else(|| anyhow!("internal symbol map error"))?;
        if values.is_empty() {
            bail!(
                "export_symbols_to_bytearray : .symtab : {}: no symbol with this name",
                symbol_name
            );
        }
        if values.len() > 1 {
            bail!(
                "export_symbols_to_bytearray : .symtab : {}: more than one symbol with this name",
                symbol_name
            );
        }
        if verbose {
            println!("Export symbol {} = {} bytes", symbol_name, values[0]);
        }
    }

    Ok(ExportedSymbols {
        start: runtime_entry_value,
        rom_ram_size: found[constants::EXPORTED_SYMBOL_ROM_RAM_SIZE][0],
        rom_size: found[constants::EXPORTED_SYMBOL_ROM_SIZE][0],
        got_size: found[constants::EXPORTED_SYMBOL_GOT_SIZE][0],
        ram_size: found[constants::EXPORTED_SYMBOL_RAM_SIZE][0],
    })
}

pub fn export_startup_code(elf: &Elf<'_>, elf_bytes: &[u8], verbose: bool) -> Result<Vec<u8>> {
    let startup_section_index = elf
        .section_headers
        .iter()
        .position(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(constants::STARTUP_SECTION_NAME))
        .ok_or_else(|| {
            anyhow!(
                "startup ELF: missing required section {}",
                constants::STARTUP_SECTION_NAME
            )
        })?;
    let startup_section = &elf.section_headers[startup_section_index];

    if startup_section.sh_type == SHT_NOBITS {
        bail!(
            "startup ELF: {} cannot be SHT_NOBITS",
            constants::STARTUP_SECTION_NAME
        );
    }

    for (idx, sh) in elf.section_headers.iter().enumerate() {
        if idx == 0 {
            continue;
        }
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("");
        if sh.sh_type == SHT_REL || sh.sh_type == SHT_RELA {
            bail!(
                "startup ELF: relocation sections are not allowed ({})",
                name
            );
        }
        let allowed = matches!(
            name,
            "._start"
                | ".symtab"
                | ".strtab"
                | ".shstrtab"
                | ".debug_info"
                | ".debug_abbrev"
                | ".debug_loclists"
                | ".debug_aranges"
                | ".debug_rnglists"
                | ".debug_line"
                | ".debug_str"
                | ".debug_frame"
                | ".debug_loc"
                | ".debug_ranges"
        );
        if !allowed {
            bail!("startup ELF: unexpected section {}", name);
        }
        if idx != startup_section_index && (sh.sh_flags & (SHF_ALLOC as u64)) != 0 {
            bail!("startup ELF: allocatable section {} is not allowed", name);
        }
    }

    let mut start_symbols = Vec::new();
    for symbol in &elf.syms {
        if symbol.st_name == 0 {
            continue;
        }
        if elf.strtab.get_at(symbol.st_name) == Some(constants::STARTUP_SYMBOL_NAME) {
            start_symbols.push(symbol);
        }
    }

    if start_symbols.is_empty() {
        bail!(
            "startup ELF: missing required symbol {}",
            constants::STARTUP_SYMBOL_NAME
        );
    }
    if start_symbols.len() > 1 {
        bail!(
            "startup ELF: more than one symbol named {}",
            constants::STARTUP_SYMBOL_NAME
        );
    }

    let start_symbol = start_symbols[0];
    if start_symbol.st_shndx != startup_section_index {
        bail!(
            "startup ELF: {} must be defined in {}",
            constants::STARTUP_SYMBOL_NAME,
            constants::STARTUP_SECTION_NAME
        );
    }
    let normalized_start = start_symbol.st_value & !1;
    if normalized_start != startup_section.sh_addr {
        bail!(
            "startup ELF: {} must point to the first byte of {}",
            constants::STARTUP_SYMBOL_NAME,
            constants::STARTUP_SECTION_NAME
        );
    }

    let actual_size = startup_section.sh_size as usize;
    if actual_size == 0 {
        bail!(
            "startup ELF: {} cannot be empty",
            constants::STARTUP_SECTION_NAME
        );
    }

    let start = startup_section.sh_offset as usize;
    let end = start
        .checked_add(actual_size)
        .ok_or_else(|| anyhow!("startup ELF: section end overflow"))?;
    if end > elf_bytes.len() {
        bail!(
            "startup ELF: {} extends beyond file bounds",
            constants::STARTUP_SECTION_NAME
        );
    }

    let out = elf_bytes[start..end].to_vec();
    if verbose {
        println!(
            "Export startup code {} : {} bytes",
            constants::STARTUP_SECTION_NAME,
            out.len()
        );
    }
    Ok(out)
}

pub fn validate_abs32_relocation_targets(elf: &Elf<'_>, elf_bytes: &[u8]) -> Result<()> {
    let invalid_targets = collect_invalid_alloc_abs32_target_sections(elf, elf_bytes)?;
    if let Some((relocation_name, target_name)) = invalid_targets.first() {
        bail!(
            "allocatable R_ARM_ABS32 relocations must target {} only; {} targets {}",
            constants::ROM_RAM_SECTION_NAME,
            relocation_name,
            target_name
        );
    }

    Ok(())
}

pub fn collect_invalid_alloc_abs32_target_sections(
    elf: &Elf<'_>,
    elf_bytes: &[u8],
) -> Result<Vec<(String, String)>> {
    let mut invalid_targets = Vec::new();

    for section in &elf.section_headers {
        if !matches!(section.sh_type, SHT_REL | SHT_RELA) {
            continue;
        }

        let relocation_name = elf
            .shdr_strtab
            .get_at(section.sh_name)
            .unwrap_or("<unnamed>");
        let target_index = section.sh_info as usize;
        let Some(target_section) = elf.section_headers.get(target_index) else {
            bail!(
                "relocation section {} targets invalid section index {}",
                relocation_name,
                target_index
            );
        };
        let target_name = elf
            .shdr_strtab
            .get_at(target_section.sh_name)
            .unwrap_or("<unnamed>");
        let target_is_alloc = (target_section.sh_flags & (SHF_ALLOC as u64)) != 0;
        let target_is_nobits = target_section.sh_type == SHT_NOBITS;

        let entry_size = match section.sh_type {
            SHT_REL => 8usize,
            SHT_RELA => 12usize,
            _ => continue,
        };
        let entsize = section.sh_entsize as usize;
        if entsize < entry_size
            || entsize == 0
            || !(section.sh_size as usize).is_multiple_of(entsize)
        {
            bail!("relocation section {} is malformed", relocation_name);
        }

        let start = section.sh_offset as usize;
        let size = section.sh_size as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| anyhow!("relocation section {} end overflow", relocation_name))?;
        if end > elf_bytes.len() {
            bail!(
                "relocation section {} extends beyond file bounds",
                relocation_name
            );
        }

        for off in (start..end).step_by(entsize) {
            let r_info = u32::from_le_bytes([
                elf_bytes[off + 4],
                elf_bytes[off + 5],
                elf_bytes[off + 6],
                elf_bytes[off + 7],
            ]);
            let r_type = r_info & 0xff;
            if r_type != constants::R_ARM_ABS32 {
                continue;
            }

            if !target_is_alloc {
                continue;
            }

            if target_name == constants::ROM_SECTION_NAME {
                let symbol_index = (r_info >> 8) as usize;
                let symbol_section_name = elf.syms.get(symbol_index).and_then(|symbol| {
                    elf.section_headers
                        .get(symbol.st_shndx)
                        .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name))
                });
                if symbol_section_name == Some(constants::ROM_SECTION_NAME) {
                    continue;
                }
            }

            if target_name != constants::ROM_RAM_SECTION_NAME || target_is_nobits {
                let entry = (relocation_name.to_owned(), target_name.to_owned());
                if !invalid_targets.iter().any(|existing| existing == &entry) {
                    invalid_targets.push(entry);
                }
            }
        }
    }

    Ok(invalid_targets)
}

pub fn validate_no_exec_relocations_to_runtime_writable_sections(
    elf: &Elf<'_>,
    elf_bytes: &[u8],
) -> Result<()> {
    for section in &elf.section_headers {
        if !matches!(section.sh_type, SHT_REL | SHT_RELA) {
            continue;
        }

        let relocation_name = elf
            .shdr_strtab
            .get_at(section.sh_name)
            .unwrap_or("<unnamed>");
        let target_index = section.sh_info as usize;
        let Some(target_section) = elf.section_headers.get(target_index) else {
            bail!(
                "relocation section {} targets invalid section index {}",
                relocation_name,
                target_index
            );
        };
        let target_is_exec = (target_section.sh_flags & (SHF_ALLOC as u64)) != 0
            && (target_section.sh_flags & (SHF_EXECINSTR as u64)) != 0;
        if !target_is_exec {
            continue;
        }

        let entry_size = match section.sh_type {
            SHT_REL => 8usize,
            SHT_RELA => 12usize,
            _ => continue,
        };
        let entsize = section.sh_entsize as usize;
        if entsize < entry_size
            || entsize == 0
            || !(section.sh_size as usize).is_multiple_of(entsize)
        {
            bail!("relocation section {} is malformed", relocation_name);
        }

        let start = section.sh_offset as usize;
        let size = section.sh_size as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| anyhow!("relocation section {} end overflow", relocation_name))?;
        if end > elf_bytes.len() {
            bail!(
                "relocation section {} extends beyond file bounds",
                relocation_name
            );
        }

        for (entry_index, off) in (start..end).step_by(entsize).enumerate() {
            let r_info = u32::from_le_bytes([
                elf_bytes[off + 4],
                elf_bytes[off + 5],
                elf_bytes[off + 6],
                elf_bytes[off + 7],
            ]);
            let r_type = r_info & 0xff;
            if r_type == 0 {
                continue;
            }
            let symbol_index = (r_info >> 8) as usize;
            if symbol_index == 0 {
                continue;
            }
            let Some(symbol) = elf.syms.get(symbol_index) else {
                bail!(
                    "relocation section {} entry {} references invalid symbol index {}",
                    relocation_name,
                    entry_index,
                    symbol_index
                );
            };

            let symbol_section_name = elf
                .section_headers
                .get(symbol.st_shndx)
                .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name));

            let Some(symbol_section_name) = symbol_section_name else {
                continue;
            };

            if !matches!(
                symbol_section_name,
                constants::GOT_SECTION_NAME | constants::ROM_RAM_SECTION_NAME | ".ram"
            ) {
                continue;
            }

            if matches!(
                r_type,
                constants::R_ARM_GOT_BREL
                    | constants::R_ARM_SBREL32
                    | constants::R_ARM_MOVW_BREL_NC
                    | constants::R_ARM_MOVT_BREL
                    | constants::R_ARM_THM_MOVW_BREL_NC
                    | constants::R_ARM_THM_MOVT_BREL
            ) {
                continue;
            }

            let symbol_name = elf
                .strtab
                .get_at(symbol.st_name)
                .unwrap_or(symbol_section_name);
            bail!(
                "executable relocation {} entry {} references writable runtime symbol {} in {}; code in {} must not address RAM-backed data through ELF relocations",
                relocation_name,
                entry_index,
                symbol_name,
                symbol_section_name,
                elf.shdr_strtab
                    .get_at(target_section.sh_name)
                    .unwrap_or("<unnamed>")
            );
        }
    }

    Ok(())
}

fn file_offset_for_vaddr(
    section: &goblin::elf::section_header::SectionHeader,
    vaddr: u32,
) -> Result<usize> {
    let section_start = section.sh_addr as u32;
    let section_size = section.sh_size as u32;
    let section_end = section_start
        .checked_add(section_size)
        .ok_or_else(|| anyhow!("section address overflow"))?;
    if vaddr < section_start || vaddr >= section_end {
        bail!(
            "virtual address 0x{vaddr:08x} is outside target section range 0x{section_start:08x}..0x{section_end:08x}"
        );
    }
    let local = (vaddr - section_start) as usize;
    Ok(section.sh_offset as usize + local)
}

fn encode_thumb_movw(rd: u16, imm16: u16) -> [u8; 4] {
    let first = 0xf240u16 | (((imm16 >> 11) & 0x1) << 10) | ((imm16 >> 12) & 0xf);
    let second = (((imm16 >> 8) & 0x7) << 12) | (rd << 8) | (imm16 & 0xff);
    let first = first.to_le_bytes();
    let second = second.to_le_bytes();
    [first[0], first[1], second[0], second[1]]
}

fn encode_thumb_movt(rd: u16, imm16: u16) -> [u8; 4] {
    let first = 0xf2c0u16 | (((imm16 >> 11) & 0x1) << 10) | ((imm16 >> 12) & 0xf);
    let second = (((imm16 >> 8) & 0x7) << 12) | (rd << 8) | (imm16 & 0xff);
    let first = first.to_le_bytes();
    let second = second.to_le_bytes();
    [first[0], first[1], second[0], second[1]]
}

fn encode_thumb_add_sb(rd: u16) -> [u8; 2] {
    let insn = 0x4400u16 | ((rd & 0x8) << 4) | (9 << 3) | (rd & 0x7);
    insn.to_le_bytes()
}

#[cfg(test)]
fn encode_thumb_add_pc(rd: u16) -> [u8; 2] {
    let insn = 0x4400u16 | ((rd & 0x8) << 4) | (15 << 3) | (rd & 0x7);
    insn.to_le_bytes()
}

fn decode_thumb_add_register(insn: u16) -> Option<(u16, u16)> {
    if (insn & 0xfc00) != 0x4400 {
        return None;
    }
    let rd = ((insn >> 4) & 0x8) | (insn & 0x7);
    let rm = (insn >> 3) & 0xf;
    Some((rd, rm))
}

fn find_thumb_add_pc_for_register(
    elf_bytes: &[u8],
    search_start: usize,
    search_end: usize,
    rd: u16,
) -> Result<Option<usize>> {
    let mut found = None;
    let mut off = search_start;
    while off + 2 <= search_end {
        let insn = read_u16(elf_bytes, off)?;
        if decode_thumb_add_register(insn) == Some((rd, 15)) {
            if found.is_some() {
                bail!(
                    "ambiguous Thumb address materialization; multiple `add r{}, pc` instructions found near MOVT",
                    rd
                );
            }
            found = Some(off);
        }
        off += 2;
    }
    Ok(found)
}

fn encode_arm_add_reg_base(cond: u32, rd: u32, rn: u32, rm: u32) -> [u8; 4] {
    let insn = (cond << 28) | 0x00800000u32 | (rn << 16) | (rd << 12) | rm;
    insn.to_le_bytes()
}

fn encode_arm_ldr_reg_offset(cond: u32, rd: u32, rn: u32, rm: u32) -> [u8; 4] {
    let insn = (cond << 28) | 0x07900000u32 | (rn << 16) | (rd << 12) | rm;
    insn.to_le_bytes()
}

fn decode_arm_add_pc_self(insn: u32) -> Option<(u32, u32)> {
    let cond = insn >> 28;
    let opcode = (insn >> 21) & 0xf;
    let s = (insn >> 20) & 0x1;
    let rn = (insn >> 16) & 0xf;
    let rd = (insn >> 12) & 0xf;
    let shift_imm = (insn >> 7) & 0x1f;
    let shift_ty = (insn >> 5) & 0x3;
    let reg_shift = (insn >> 4) & 0x1;
    let rm = insn & 0xf;
    let is_data_proc_reg = ((insn >> 26) & 0x3) == 0 && ((insn >> 25) & 0x1) == 0;
    if is_data_proc_reg
        && opcode == 0x4
        && s == 0
        && rn == 15
        && rd == rm
        && shift_imm == 0
        && shift_ty == 0
        && reg_shift == 0
    {
        Some((cond, rd))
    } else {
        None
    }
}

fn decode_arm_ldr_pc_self(insn: u32) -> Option<(u32, u32)> {
    let cond = insn >> 28;
    let class = (insn >> 25) & 0x7;
    let p = (insn >> 24) & 0x1;
    let u = (insn >> 23) & 0x1;
    let b = (insn >> 22) & 0x1;
    let w = (insn >> 21) & 0x1;
    let l = (insn >> 20) & 0x1;
    let rn = (insn >> 16) & 0xf;
    let rd = (insn >> 12) & 0xf;
    let shift_imm = (insn >> 7) & 0x1f;
    let shift_ty = (insn >> 5) & 0x3;
    let reg_shift = (insn >> 4) & 0x1;
    let rm = insn & 0xf;
    if class == 0b011
        && p == 1
        && u == 1
        && b == 0
        && w == 0
        && l == 1
        && rn == 15
        && rd == rm
        && shift_imm == 0
        && shift_ty == 0
        && reg_shift == 0
    {
        Some((cond, rd))
    } else {
        None
    }
}

fn encode_arm_movw(rd: u32, imm16: u32) -> [u8; 4] {
    let insn = 0xe3000000u32
        | (((imm16 >> 12) & 0xf) << 16)
        | (rd << 12)
        | (((imm16 >> 11) & 0x1) << 26)
        | ((imm16 >> 8) & 0xf)
        | (imm16 & 0xff);
    insn.to_le_bytes()
}

fn encode_arm_movt(rd: u32, imm16: u32) -> [u8; 4] {
    let insn = 0xe3400000u32
        | (((imm16 >> 12) & 0xf) << 16)
        | (rd << 12)
        | (((imm16 >> 11) & 0x1) << 26)
        | ((imm16 >> 8) & 0xf)
        | (imm16 & 0xff);
    insn.to_le_bytes()
}

fn find_thumb_ldr_literal_add_pc_pattern(
    elf_bytes: &[u8],
    target_section: &goblin::elf::section_header::SectionHeader,
    literal_vaddr: u32,
) -> Result<Option<(usize, usize, u16)>> {
    let section_file_start = target_section.sh_offset as usize;
    let section_file_end = section_file_start
        .checked_add(target_section.sh_size as usize)
        .ok_or_else(|| anyhow!("section file range overflow"))?;
    if section_file_end > elf_bytes.len() {
        bail!("section extends beyond file bounds");
    }

    let mut found = None;
    let mut off = section_file_start;
    while off + 4 <= section_file_end {
        let insn = read_u16(elf_bytes, off)?;
        if (insn & 0xf800) == 0x4800 {
            let rd = ((insn >> 8) & 0x7) as u16;
            let imm8 = (insn & 0xff) as u32;
            let instr_vaddr = target_section.sh_addr as u32 + (off - section_file_start) as u32;
            let literal_addr = (instr_vaddr + 4) & !3;
            let literal_addr = literal_addr
                .checked_add(imm8 * 4)
                .ok_or_else(|| anyhow!("literal address overflow"))?;
            if literal_addr == literal_vaddr {
                let search_end = (off + 16).min(section_file_end);
                let mut add_off = off + 2;
                while add_off + 2 <= search_end {
                    let add_insn = read_u16(elf_bytes, add_off)?;
                    if decode_thumb_add_register(add_insn) == Some((rd, 15)) {
                        if found.is_some() {
                            bail!(
                                "multiple Thumb literal+add-pc patterns reference literal at 0x{literal_vaddr:08x}"
                            );
                        }
                        found = Some((off, add_off, rd));
                        break;
                    }
                    add_off += 2;
                }
            }
        }
        off += 2;
    }

    Ok(found)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let tail = bytes
        .get(offset..offset + 2)
        .ok_or_else(|| anyhow!("truncated halfword at file offset {}", offset))?;
    Ok(u16::from_le_bytes([tail[0], tail[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let tail = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| anyhow!("truncated word at file offset {}", offset))?;
    Ok(u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]))
}

fn runtime_writable_symbol_section_name<'a>(
    elf: &'a Elf<'_>,
    symbol: &goblin::elf::sym::Sym,
) -> Option<&'a str> {
    elf.section_headers
        .get(symbol.st_shndx)
        .and_then(|sh| elf.shdr_strtab.get_at(sh.sh_name))
        .filter(|name| {
            matches!(
                *name,
                constants::GOT_SECTION_NAME | constants::ROM_RAM_SECTION_NAME | ".ram"
            )
        })
}

#[allow(clippy::too_many_arguments)]
fn find_paired_movt_relocation(
    elf_bytes: &[u8],
    rel_start: usize,
    rel_entsize: usize,
    rel_count: usize,
    movw_index: usize,
    symbol_index: usize,
    movw_offset: u32,
    expected_movt_type: u32,
) -> Result<Option<(usize, u32)>> {
    let max_search_offset = movw_offset.saturating_add(16);

    for candidate_index in movw_index + 1..rel_count {
        let candidate_off = rel_start + candidate_index * rel_entsize;
        let candidate_r_offset = u32::from_le_bytes([
            elf_bytes[candidate_off],
            elf_bytes[candidate_off + 1],
            elf_bytes[candidate_off + 2],
            elf_bytes[candidate_off + 3],
        ]);
        if candidate_r_offset > max_search_offset {
            break;
        }

        let candidate_r_info = u32::from_le_bytes([
            elf_bytes[candidate_off + 4],
            elf_bytes[candidate_off + 5],
            elf_bytes[candidate_off + 6],
            elf_bytes[candidate_off + 7],
        ]);
        let candidate_r_type = candidate_r_info & 0xff;
        let candidate_symbol_index = (candidate_r_info >> 8) as usize;

        if candidate_r_type == expected_movt_type && candidate_symbol_index == symbol_index {
            return Ok(Some((candidate_off, candidate_r_offset)));
        }
    }

    Ok(None)
}

pub fn rewrite_exec_runtime_references_to_sbrel(
    elf_bytes: &mut [u8],
    rom_size: u32,
) -> Result<usize> {
    struct BytePatch {
        offset: usize,
        bytes: Vec<u8>,
    }

    let elf = parse_elf(elf_bytes)?;
    let mut patched_pairs = 0usize;
    let mut patches = Vec::<BytePatch>::new();

    for section in &elf.section_headers {
        if section.sh_type != SHT_REL {
            continue;
        }

        let target_index = section.sh_info as usize;
        let Some(target_section) = elf.section_headers.get(target_index) else {
            bail!(
                "relocation section {} targets invalid section index {}",
                elf.shdr_strtab
                    .get_at(section.sh_name)
                    .unwrap_or("<unnamed>"),
                target_index
            );
        };

        let target_is_exec = (target_section.sh_flags & (SHF_ALLOC as u64)) != 0
            && (target_section.sh_flags & (SHF_EXECINSTR as u64)) != 0;
        if !target_is_exec {
            continue;
        }

        let entsize = section.sh_entsize as usize;
        if entsize < 8 || entsize == 0 || !(section.sh_size as usize).is_multiple_of(entsize) {
            bail!(
                "relocation section {} is malformed",
                elf.shdr_strtab
                    .get_at(section.sh_name)
                    .unwrap_or("<unnamed>")
            );
        }

        let start = section.sh_offset as usize;
        let end = start
            .checked_add(section.sh_size as usize)
            .ok_or_else(|| anyhow!("relocation section end overflow"))?;
        if end > elf_bytes.len() {
            bail!("relocation section extends beyond file bounds");
        }

        let entry_count = (end - start) / entsize;
        let mut entry_index = 0usize;
        while entry_index < entry_count {
            let off = start + entry_index * entsize;
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
            if r_type == constants::R_ARM_REL32 {
                let symbol_index = (r_info >> 8) as usize;
                let Some(symbol) = elf.syms.get(symbol_index) else {
                    bail!(
                        "relocation section {} entry {} references invalid symbol index {}",
                        elf.shdr_strtab
                            .get_at(section.sh_name)
                            .unwrap_or("<unnamed>"),
                        entry_index,
                        symbol_index
                    );
                };
                let Some(symbol_section_name) = runtime_writable_symbol_section_name(&elf, &symbol)
                else {
                    entry_index += 1;
                    continue;
                };

                let literal_file_off = file_offset_for_vaddr(target_section, r_offset)?;
                let Some((_, add_file_off, rd)) =
                    find_thumb_ldr_literal_add_pc_pattern(elf_bytes, target_section, r_offset)?
                else {
                    bail!(
                        "unsupported R_ARM_REL32 executable reference to writable runtime symbol {} in {}",
                        elf.strtab.get_at(symbol.st_name).unwrap_or(symbol_section_name),
                        elf.shdr_strtab.get_at(section.sh_name).unwrap_or("<unnamed>")
                    );
                };

                let sbrel = symbol.st_value as u32;
                if sbrel < rom_size {
                    bail!(
                        "runtime writable symbol {} unexpectedly resides before .got/.rom.ram",
                        elf.strtab
                            .get_at(symbol.st_name)
                            .unwrap_or(symbol_section_name)
                    );
                }
                let sbrel = sbrel - rom_size;
                patches.push(BytePatch {
                    offset: literal_file_off,
                    bytes: sbrel.to_le_bytes().to_vec(),
                });
                patches.push(BytePatch {
                    offset: add_file_off,
                    bytes: encode_thumb_add_sb(rd).to_vec(),
                });
                patches.push(BytePatch {
                    offset: off + 4,
                    bytes: 0u32.to_le_bytes().to_vec(),
                });
                patched_pairs += 1;
                entry_index += 1;
                continue;
            }

            if !matches!(
                r_type,
                constants::R_ARM_THM_MOVW_PREL_NC | constants::R_ARM_MOVW_PREL_NC
            ) {
                entry_index += 1;
                continue;
            }

            let symbol_index = (r_info >> 8) as usize;
            let Some(symbol) = elf.syms.get(symbol_index) else {
                bail!(
                    "relocation section {} entry {} references invalid symbol index {}",
                    elf.shdr_strtab
                        .get_at(section.sh_name)
                        .unwrap_or("<unnamed>"),
                    entry_index,
                    symbol_index
                );
            };

            let Some(symbol_section_name) = runtime_writable_symbol_section_name(&elf, &symbol)
            else {
                entry_index += 1;
                continue;
            };

            let is_thumb_mov_pair = r_type == constants::R_ARM_THM_MOVW_PREL_NC;
            let expected_movt_type = if is_thumb_mov_pair {
                constants::R_ARM_THM_MOVT_PREL
            } else {
                constants::R_ARM_MOVT_PREL
            };
            let Some((next_off, next_r_offset)) = find_paired_movt_relocation(
                elf_bytes,
                start,
                entsize,
                entry_count,
                entry_index,
                symbol_index,
                r_offset,
                expected_movt_type,
            )?
            else {
                bail!(
                    "unsupported executable relocation pattern for writable runtime symbol {} in {}",
                    elf.strtab.get_at(symbol.st_name).unwrap_or(symbol_section_name),
                    elf.shdr_strtab.get_at(section.sh_name).unwrap_or("<unnamed>")
                );
            };

            let sbrel = symbol.st_value as u32;
            if sbrel < rom_size {
                bail!(
                    "runtime writable symbol {} unexpectedly resides before .got/.rom.ram",
                    elf.strtab
                        .get_at(symbol.st_name)
                        .unwrap_or(symbol_section_name)
                );
            }
            let sbrel = sbrel - rom_size;
            let movw_file_off = file_offset_for_vaddr(target_section, r_offset)?;
            let movt_file_off = file_offset_for_vaddr(target_section, next_r_offset)?;

            if is_thumb_mov_pair {
                let movw_second = read_u16(elf_bytes, movw_file_off + 2)?;
                let movt_second = read_u16(elf_bytes, movt_file_off + 2)?;
                let rd = ((movw_second >> 8) & 0xf) as u16;
                let movt_rd = ((movt_second >> 8) & 0xf) as u16;
                if rd != movt_rd {
                    bail!(
                        "MOVW/MOVT register mismatch while patching writable runtime symbol {}",
                        elf.strtab
                            .get_at(symbol.st_name)
                            .unwrap_or(symbol_section_name)
                    );
                }

                // LLVM may interleave several MOVW/MOVT pairs before emitting their
                // matching ADD pc instructions in optimized Thumb code.
                let search_start = movt_file_off + 4;
                let search_end = (search_start + 16).min(elf_bytes.len());
                let Some(add_file_off) =
                    find_thumb_add_pc_for_register(elf_bytes, search_start, search_end, rd)?
                else {
                    let found = if search_start + 2 <= elf_bytes.len() {
                        Some(read_u16(elf_bytes, search_start)?)
                    } else {
                        None
                    };
                    bail!(
                        "unsupported Thumb address materialization for writable runtime symbol {}; expected `add r{}, pc` near MOVT but found {}",
                        elf.strtab.get_at(symbol.st_name).unwrap_or(symbol_section_name),
                        rd,
                        found
                            .map(|insn| format!("0x{insn:04x}"))
                            .unwrap_or_else(|| "end of file".to_string())
                    );
                };

                let movw = encode_thumb_movw(rd, (sbrel & 0xffff) as u16);
                let movt = encode_thumb_movt(rd, (sbrel >> 16) as u16);
                let add = encode_thumb_add_sb(rd);
                patches.push(BytePatch {
                    offset: movw_file_off,
                    bytes: movw.to_vec(),
                });
                patches.push(BytePatch {
                    offset: movt_file_off,
                    bytes: movt.to_vec(),
                });
                patches.push(BytePatch {
                    offset: add_file_off,
                    bytes: add.to_vec(),
                });
            } else {
                let op_file_off = movt_file_off + 4;
                if op_file_off + 4 > elf_bytes.len() {
                    bail!("truncated ARM executable sequence while patching SB-relative reference");
                }

                let movw_insn = read_u32(elf_bytes, movw_file_off)?;
                let movt_insn = read_u32(elf_bytes, movt_file_off)?;
                let rd = (movw_insn >> 12) & 0xf;
                let movt_rd = (movt_insn >> 12) & 0xf;
                if rd != movt_rd {
                    bail!(
                        "ARM MOVW/MOVT register mismatch while patching writable runtime symbol {}",
                        elf.strtab
                            .get_at(symbol.st_name)
                            .unwrap_or(symbol_section_name)
                    );
                }

                let op_insn = read_u32(elf_bytes, op_file_off)?;
                let replacement = if let Some((cond, decoded_rd)) = decode_arm_add_pc_self(op_insn)
                {
                    if decoded_rd != rd {
                        bail!(
                            "ARM ADD register mismatch while patching writable runtime symbol {}",
                            elf.strtab
                                .get_at(symbol.st_name)
                                .unwrap_or(symbol_section_name)
                        );
                    }
                    encode_arm_add_reg_base(cond, rd, 9, rd).to_vec()
                } else if let Some((cond, decoded_rd)) = decode_arm_ldr_pc_self(op_insn) {
                    if decoded_rd != rd {
                        bail!(
                            "ARM LDR register mismatch while patching writable runtime symbol {}",
                            elf.strtab
                                .get_at(symbol.st_name)
                                .unwrap_or(symbol_section_name)
                        );
                    }
                    encode_arm_ldr_reg_offset(cond, rd, 9, rd).to_vec()
                } else {
                    bail!(
                        "unsupported ARM address materialization for writable runtime symbol {}; expected `add r{}, pc, r{}` or `ldr r{}, [pc, r{}]` but found 0x{:08x}",
                        elf.strtab.get_at(symbol.st_name).unwrap_or(symbol_section_name),
                        rd,
                        rd,
                        rd,
                        rd,
                        op_insn,
                    );
                };

                let movw_reencoded = encode_arm_movw(rd, sbrel & 0xffff).to_vec();
                let movt_reencoded = encode_arm_movt(rd, (sbrel >> 16) & 0xffff).to_vec();
                patches.push(BytePatch {
                    offset: movw_file_off,
                    bytes: movw_reencoded,
                });
                patches.push(BytePatch {
                    offset: movt_file_off,
                    bytes: movt_reencoded,
                });
                patches.push(BytePatch {
                    offset: op_file_off,
                    bytes: replacement,
                });
            }
            patches.push(BytePatch {
                offset: off + 4,
                bytes: 0u32.to_le_bytes().to_vec(),
            });
            patches.push(BytePatch {
                offset: next_off + 4,
                bytes: 0u32.to_le_bytes().to_vec(),
            });
            patched_pairs += 1;
            entry_index += 1;
        }
    }

    for patch in patches {
        let end = patch
            .offset
            .checked_add(patch.bytes.len())
            .ok_or_else(|| anyhow!("patch end overflow"))?;
        let dst = elf_bytes
            .get_mut(patch.offset..end)
            .ok_or_else(|| anyhow!("patch out of bounds at file offset {}", patch.offset))?;
        dst.copy_from_slice(&patch.bytes);
    }

    Ok(patched_pairs)
}

pub fn export_relocation_table(
    elf: &Elf<'_>,
    elf_bytes: &[u8],
    relocation_table_name: &str,
    raw_symbols: ExportedSymbols,
    padded_symbols: ExportedSymbols,
    verbose: bool,
) -> Result<(Vec<u8>, Vec<u32>)> {
    let maybe_section = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(relocation_table_name));

    let Some(section) = maybe_section else {
        if verbose {
            println!("No relocation section named {}", relocation_table_name);
        }
        return Ok(((0u32).to_le_bytes().to_vec(), Vec::new()));
    };

    if section.sh_type == SHT_RELA {
        bail!(
            "export_relocation_table : {} : unsupported RELA",
            relocation_table_name
        );
    }
    if section.sh_type != SHT_REL {
        bail!(
            "export_relocation_table : {}: is not a relocation section",
            relocation_table_name
        );
    }

    let entsize = section.sh_entsize as usize;
    if entsize == 0 || !(section.sh_size as usize).is_multiple_of(entsize) {
        bail!(
            "export_relocation_table : {} : malformed relocation section",
            relocation_table_name
        );
    }

    let count = section.sh_size as usize / entsize;
    let mut out = Vec::with_capacity(4 + count * 4);
    let mut raw_offsets = Vec::with_capacity(count);
    out.extend_from_slice(&(count as u32).to_le_bytes());
    if verbose {
        println!("Export relocation table : entries count : {}", count);
    }

    let rom_size = raw_symbols.rom_size as usize;
    let got_size = raw_symbols.got_size as usize;
    let rom_ram_size = raw_symbols.rom_ram_size as usize;
    let ram_size = raw_symbols.ram_size as usize;
    let writable_start = rom_size + got_size;
    let writable_end = writable_start + rom_ram_size + ram_size;

    let base = section.sh_offset as usize;
    for i in 0..count {
        let off = base + i * entsize;
        if off + 8 > elf_bytes.len() {
            bail!(
                "export_relocation_table : {} : entry {} out of file bounds",
                relocation_table_name,
                i
            );
        }
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
        if r_type != constants::R_ARM_ABS32 {
            bail!(
                "export_relocation_table : {} : entry {}: unsupported relocation type",
                relocation_table_name,
                i
            );
        }
        let off = r_offset as usize;
        if off < writable_start || off >= writable_end {
            bail!(
                "export_relocation_table : {} : entry {}: relocation offset {} targets non-writable area (.rom/.got or out of range)",
                relocation_table_name,
                i,
                r_offset
            );
        }
        let mapped = remap_offset_to_padded_layout(r_offset, raw_symbols, padded_symbols)
            .ok_or_else(|| {
                anyhow!(
                    "export_relocation_table : {} : entry {}: relocation offset {} cannot be remapped to padded layout",
                    relocation_table_name,
                    i,
                    r_offset
                )
            })?;
        raw_offsets.push(r_offset);
        out.extend_from_slice(&mapped.to_le_bytes());
        if verbose {
            println!("\t- Exporting relocation entry {} : offset {}", i, r_offset);
        }
    }

    Ok((out, raw_offsets))
}

pub fn export_section_bytes(
    elf: &Elf<'_>,
    elf_bytes: &[u8],
    section_name: &str,
    expected_size: usize,
    verbose: bool,
) -> Result<Vec<u8>> {
    let maybe_section = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(section_name));

    let Some(section) = maybe_section else {
        if expected_size == 0 {
            if verbose {
                println!("No section named {}", section_name);
            }
            return Ok(Vec::new());
        }
        bail!(
            "export_section_bytes : {} : no section with this name found",
            section_name
        );
    };

    let actual_size = section.sh_size as usize;
    if actual_size != expected_size {
        bail!(
            "export_section_bytes : {} : section size ({}) differs from exported size ({})",
            section_name,
            actual_size,
            expected_size
        );
    }

    if actual_size == 0 {
        if verbose {
            println!("Export section {} : 0 bytes", section_name);
        }
        return Ok(Vec::new());
    }

    if section.sh_type == SHT_NOBITS {
        bail!(
            "export_section_bytes : {} : SHT_NOBITS section cannot be embedded in partition payload",
            section_name
        );
    }

    let start = section.sh_offset as usize;
    let end = start.checked_add(actual_size).ok_or_else(|| {
        anyhow!(
            "export_section_bytes : {} : section end overflow",
            section_name
        )
    })?;
    if end > elf_bytes.len() {
        bail!(
            "export_section_bytes : {} : section extends beyond file bounds",
            section_name
        );
    }

    let out = elf_bytes[start..end].to_vec();
    if verbose {
        println!("Export section {} : {} bytes", section_name, out.len());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_arm_add_pc_self, decode_arm_ldr_pc_self, decode_thumb_add_register,
        encode_thumb_add_pc, encode_thumb_add_sb, find_thumb_add_pc_for_register,
    };

    #[test]
    fn encode_thumb_add_sb_supports_low_registers() {
        assert_eq!(u16::from_le_bytes(encode_thumb_add_sb(4)), 0x444c);
    }

    #[test]
    fn encode_thumb_add_sb_supports_high_registers() {
        assert_eq!(u16::from_le_bytes(encode_thumb_add_sb(12)), 0x44cc);
    }

    #[test]
    fn encode_thumb_add_pc_supports_high_registers() {
        assert_eq!(u16::from_le_bytes(encode_thumb_add_pc(12)), 0x44fc);
    }

    #[test]
    fn decode_thumb_add_register_handles_high_register_pc_base() {
        assert_eq!(decode_thumb_add_register(0x44fc), Some((12, 15)));
    }

    #[test]
    fn thumb_add_pc_search_skips_interleaved_materializations() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encode_thumb_add_pc(0));
        bytes.extend_from_slice(&encode_thumb_add_pc(2));
        assert_eq!(
            find_thumb_add_pc_for_register(&bytes, 0, bytes.len(), 2).unwrap(),
            Some(2)
        );
    }

    #[test]
    fn thumb_add_pc_search_rejects_ambiguous_matches() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encode_thumb_add_pc(2));
        bytes.extend_from_slice(&encode_thumb_add_pc(2));
        assert!(find_thumb_add_pc_for_register(&bytes, 0, bytes.len(), 2).is_err());
    }

    #[test]
    fn decode_arm_pc_relative_materialization_patterns() {
        assert_eq!(decode_arm_add_pc_self(0xe08ff00f), Some((0xe, 15)));
        assert_eq!(decode_arm_ldr_pc_self(0xe79ff00f), Some((0xe, 15)));
    }
}
