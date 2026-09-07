use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use goblin::elf::section_header::{
    SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHT_NOBITS, SHT_REL, SHT_RELA, SHT_SYMTAB,
};
use object::read::archive::ArchiveFile;

use crate::constants;
use crate::elf::parse_elf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RustInputSectionPlan {
    pub rom_sections: Vec<String>,
    pub got_sections: Vec<String>,
    pub rom_ram_sections: Vec<String>,
    pub ram_sections: Vec<String>,
    pub discard_sections: Vec<String>,
    pub alloc_abs32_sections: Vec<String>,
    pub nonalloc_abs32_sections: Vec<String>,
}

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanBucket {
    Rom,
    Got,
    RomRam,
    Ram,
    Discard,
    AllocAbs32,
    NonAllocAbs32,
}

impl PlanBucket {
    fn as_str(self) -> &'static str {
        match self {
            Self::Rom => "rom",
            Self::Got => "got",
            Self::RomRam => "rom_ram",
            Self::Ram => "ram",
            Self::Discard => "discard",
            Self::AllocAbs32 => "alloc_abs32",
            Self::NonAllocAbs32 => "nonalloc_abs32",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "rom" => Some(Self::Rom),
            "got" => Some(Self::Got),
            "rom_ram" => Some(Self::RomRam),
            "ram" => Some(Self::Ram),
            "discard" => Some(Self::Discard),
            "alloc_abs32" => Some(Self::AllocAbs32),
            "nonalloc_abs32" => Some(Self::NonAllocAbs32),
            _ => None,
        }
    }
}

impl RustInputSectionPlan {
    pub fn parse(text: &str) -> Result<Self> {
        let mut plan = Self::default();

        for (lineno, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((bucket, section_name)) = line.split_once('\t') else {
                bail!("invalid section plan line {}: {:?}", lineno + 1, raw_line);
            };
            let Some(bucket) = PlanBucket::parse(bucket) else {
                bail!(
                    "invalid section plan bucket on line {}: {:?}",
                    lineno + 1,
                    bucket
                );
            };
            plan.push(bucket, section_name);
        }

        Ok(plan)
    }

    pub fn read_from_path(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Self::parse(&text)
    }

    pub fn write_to_path(&self, path: &Path) -> Result<()> {
        fs::write(path, self.to_text()).with_context(|| format!("cannot write {}", path.display()))
    }

    pub fn write_atomically_to_path(&self, path: &Path) -> Result<()> {
        let parent = path
            .parent()
            .context("section plan path has no parent directory")?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("section plan path has no valid UTF-8 file name")?;
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(".{file_name}.tmp.{}.{}", process::id(), counter));

        fs::write(&temp_path, self.to_text())
            .with_context(|| format!("cannot write {}", temp_path.display()))?;

        replace_file_portably(&temp_path, path)?;
        Ok(())
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();

        for name in &self.rom_sections {
            push_line(&mut out, PlanBucket::Rom, name);
        }
        for name in &self.got_sections {
            push_line(&mut out, PlanBucket::Got, name);
        }
        for name in &self.rom_ram_sections {
            push_line(&mut out, PlanBucket::RomRam, name);
        }
        for name in &self.ram_sections {
            push_line(&mut out, PlanBucket::Ram, name);
        }
        for name in &self.discard_sections {
            push_line(&mut out, PlanBucket::Discard, name);
        }
        for name in &self.alloc_abs32_sections {
            push_line(&mut out, PlanBucket::AllocAbs32, name);
        }
        for name in &self.nonalloc_abs32_sections {
            push_line(&mut out, PlanBucket::NonAllocAbs32, name);
        }

        out
    }

    pub fn merge(&mut self, other: &Self) {
        for name in &other.rom_sections {
            self.push(PlanBucket::Rom, name);
        }
        for name in &other.got_sections {
            self.push(PlanBucket::Got, name);
        }
        for name in &other.rom_ram_sections {
            self.push(PlanBucket::RomRam, name);
        }
        for name in &other.ram_sections {
            self.push(PlanBucket::Ram, name);
        }
        for name in &other.discard_sections {
            self.push(PlanBucket::Discard, name);
        }
        for name in &other.alloc_abs32_sections {
            self.push(PlanBucket::AllocAbs32, name);
        }
        for name in &other.nonalloc_abs32_sections {
            self.push(PlanBucket::NonAllocAbs32, name);
        }
    }

    fn push(&mut self, bucket: PlanBucket, value: &str) {
        match bucket {
            PlanBucket::Got | PlanBucket::RomRam | PlanBucket::Ram | PlanBucket::Discard => {
                remove_section(&mut self.rom_sections, value);
            }
            PlanBucket::Rom => {
                if self.got_sections.iter().any(|existing| existing == value)
                    || self
                        .rom_ram_sections
                        .iter()
                        .any(|existing| existing == value)
                    || self.ram_sections.iter().any(|existing| existing == value)
                    || self
                        .discard_sections
                        .iter()
                        .any(|existing| existing == value)
                {
                    return;
                }
            }
            PlanBucket::AllocAbs32 | PlanBucket::NonAllocAbs32 => {}
        }

        let target = match bucket {
            PlanBucket::Rom => &mut self.rom_sections,
            PlanBucket::Got => &mut self.got_sections,
            PlanBucket::RomRam => &mut self.rom_ram_sections,
            PlanBucket::Ram => &mut self.ram_sections,
            PlanBucket::Discard => &mut self.discard_sections,
            PlanBucket::AllocAbs32 => &mut self.alloc_abs32_sections,
            PlanBucket::NonAllocAbs32 => &mut self.nonalloc_abs32_sections,
        };

        if !target.iter().any(|existing| existing == value) {
            target.push(value.to_owned());
        }
    }
}

fn remove_section(bucket: &mut Vec<String>, value: &str) {
    bucket.retain(|existing| existing != value);
}

fn push_line(out: &mut String, bucket: PlanBucket, name: &str) {
    out.push_str(bucket.as_str());
    out.push('\t');
    out.push_str(name);
    out.push('\n');
}

fn replace_file_portably(temp_path: &Path, destination: &Path) -> Result<()> {
    match fs::rename(temp_path, destination) {
        Ok(()) => Ok(()),
        Err(rename_error) => {
            if destination.exists() {
                fs::remove_file(destination)
                    .with_context(|| format!("cannot remove existing {}", destination.display()))?;
                fs::rename(temp_path, destination).with_context(|| {
                    format!(
                        "cannot replace {} with {} after removing destination: {}",
                        destination.display(),
                        temp_path.display(),
                        rename_error
                    )
                })?;
                Ok(())
            } else {
                Err(rename_error).with_context(|| {
                    format!(
                        "cannot move temporary section plan {} to {}",
                        temp_path.display(),
                        destination.display()
                    )
                })
            }
        }
    }
}

pub fn collect_rust_input_section_plan(paths: &[PathBuf]) -> Result<RustInputSectionPlan> {
    let mut plan = RustInputSectionPlan::default();

    for path in paths {
        scan_input_path(path, &mut plan)?;
    }

    Ok(plan)
}

fn scan_input_path(path: &Path, plan: &mut RustInputSectionPlan) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;

    if is_elf(&bytes) {
        return scan_elf_object(&bytes, plan);
    }

    if matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rlib") | Some("a")
    ) {
        return scan_archive(path, &bytes, plan);
    }

    Ok(())
}

fn is_elf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && bytes[..4] == [0x7f, b'E', b'L', b'F']
}

fn scan_archive(path: &Path, bytes: &[u8], plan: &mut RustInputSectionPlan) -> Result<()> {
    let archive = ArchiveFile::parse(bytes)
        .with_context(|| format!("cannot parse archive {}", path.display()))?;

    for member in archive.members() {
        let member = member.with_context(|| format!("cannot read member in {}", path.display()))?;
        let name = String::from_utf8_lossy(member.name());
        if !name.ends_with(".o") {
            continue;
        }
        let member_bytes = member
            .data(bytes)
            .with_context(|| format!("cannot read member {} in {}", name, path.display()))?;
        if is_elf(member_bytes) {
            scan_elf_object(member_bytes, plan)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct SectionDescriptor {
    name: String,
    is_alloc: bool,
    is_writable: bool,
    is_executable: bool,
    is_nobits: bool,
}

impl SectionDescriptor {
    fn is_relro_candidate(&self) -> bool {
        self.name.starts_with(".data.rel.ro")
    }
}

fn section_descriptors(elf_bytes: &[u8]) -> Result<Vec<SectionDescriptor>> {
    let elf = parse_elf(elf_bytes)?;
    let mut sections = Vec::new();

    for (index, section) in elf.section_headers.iter().enumerate() {
        if index == 0 {
            continue;
        }
        let Some(name) = elf.shdr_strtab.get_at(section.sh_name) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }

        sections.push(SectionDescriptor {
            name: name.to_owned(),
            is_alloc: (section.sh_flags & (SHF_ALLOC as u64)) != 0,
            is_writable: (section.sh_flags & (SHF_WRITE as u64)) != 0,
            is_executable: (section.sh_flags & (SHF_EXECINSTR as u64)) != 0,
            is_nobits: section.sh_type == SHT_NOBITS,
        });
    }

    Ok(sections)
}

fn collect_abs32_section_edges(elf_bytes: &[u8]) -> Result<HashMap<String, Vec<String>>> {
    let elf = parse_elf(elf_bytes)?;
    let mut edges = HashMap::<String, Vec<String>>::new();

    for section in &elf.section_headers {
        if !matches!(section.sh_type, SHT_REL | SHT_RELA) {
            continue;
        }

        let target_index = section.sh_info as usize;
        let Some(target_section) = elf.section_headers.get(target_index) else {
            continue;
        };
        let Some(target_name) = elf.shdr_strtab.get_at(target_section.sh_name) else {
            continue;
        };
        let symtab_index = section.sh_link as usize;
        let Some(symtab_section) = elf.section_headers.get(symtab_index) else {
            continue;
        };
        if symtab_section.sh_type != SHT_SYMTAB {
            continue;
        }

        let entry_size = section.sh_entsize as usize;
        if entry_size == 0 {
            continue;
        }
        let start = section.sh_offset as usize;
        let size = section.sh_size as usize;
        let end = start
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("relocation section end overflow"))?;
        if end > elf_bytes.len() {
            bail!("relocation section extends beyond file bounds");
        }

        for off in (start..end).step_by(entry_size) {
            if off + 8 > elf_bytes.len() {
                bail!("truncated relocation entry");
            }
            let r_info = u32::from_le_bytes([
                elf_bytes[off + 4],
                elf_bytes[off + 5],
                elf_bytes[off + 6],
                elf_bytes[off + 7],
            ]);
            if (r_info & 0xff) == constants::R_ARM_ABS32 {
                let symbol_index = (r_info >> 8) as usize;
                let Some(symbol) = elf.syms.get(symbol_index) else {
                    continue;
                };
                let target_section_index = symbol.st_shndx;
                if target_section_index == 0 {
                    continue;
                }
                let Some(symbol_section) = elf.section_headers.get(target_section_index) else {
                    continue;
                };
                let Some(symbol_section_name) = elf.shdr_strtab.get_at(symbol_section.sh_name)
                else {
                    continue;
                };
                let targets = edges.entry(target_name.to_owned()).or_default();
                if !targets
                    .iter()
                    .any(|existing| existing == symbol_section_name)
                {
                    targets.push(symbol_section_name.to_owned());
                }
            }
        }
    }

    Ok(edges)
}

fn scan_elf_object(elf_bytes: &[u8], plan: &mut RustInputSectionPlan) -> Result<()> {
    let descriptors = section_descriptors(elf_bytes)?;
    let abs32_edges = collect_abs32_section_edges(elf_bytes)?;

    for section in &descriptors {
        let name = section.name.as_str();

        if name.starts_with(".ARM.exidx") || name.starts_with(".ARM.extab") {
            plan.push(PlanBucket::Discard, name);
            continue;
        }

        let has_abs32 = abs32_edges.contains_key(name);

        if !section.is_alloc {
            if has_abs32 {
                plan.push(PlanBucket::NonAllocAbs32, name);
            }
            continue;
        }

        if name.starts_with(".got") {
            plan.push(PlanBucket::Got, name);
            continue;
        }

        if section.is_relro_candidate() {
            if has_abs32 {
                plan.push(PlanBucket::AllocAbs32, name);
                plan.push(PlanBucket::RomRam, name);
            } else {
                plan.push(PlanBucket::Rom, name);
            }
            continue;
        }

        if section.is_writable {
            if section.is_nobits {
                plan.push(PlanBucket::Ram, name);
            } else {
                plan.push(PlanBucket::RomRam, name);
            }
            continue;
        }

        if has_abs32 && !section.is_executable {
            plan.push(PlanBucket::AllocAbs32, name);
            plan.push(PlanBucket::RomRam, name);
            continue;
        }

        if section.is_nobits {
            plan.push(PlanBucket::Ram, name);
            continue;
        }

        plan.push(PlanBucket::Rom, name);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::RustInputSectionPlan;

    #[test]
    fn section_plan_roundtrip() {
        let plan = RustInputSectionPlan {
            rom_sections: vec![".text.foo".to_owned()],
            got_sections: vec![".got".to_owned()],
            rom_ram_sections: vec![".rodata..Lanon.1".to_owned()],
            ram_sections: vec![".bss.foo".to_owned()],
            discard_sections: vec![".ARM.exidx.foo".to_owned()],
            alloc_abs32_sections: vec![".rodata..Lanon.1".to_owned()],
            nonalloc_abs32_sections: vec![".debug_info".to_owned()],
        };

        let text = plan.to_text();
        let decoded = RustInputSectionPlan::parse(&text).unwrap();
        assert_eq!(decoded, plan);
    }

    #[test]
    fn runtime_bucket_overrides_previous_rom_bucket() {
        let mut plan = RustInputSectionPlan::default();

        plan.push(super::PlanBucket::Rom, ".rodata..Lanon.1");
        plan.push(super::PlanBucket::AllocAbs32, ".rodata..Lanon.1");
        plan.push(super::PlanBucket::RomRam, ".rodata..Lanon.1");
        plan.push(super::PlanBucket::Rom, ".rodata..Lanon.1");

        assert!(plan.rom_sections.is_empty());
        assert_eq!(plan.rom_ram_sections, vec![".rodata..Lanon.1"]);
        assert_eq!(plan.alloc_abs32_sections, vec![".rodata..Lanon.1"]);

        let decoded = RustInputSectionPlan::parse(&plan.to_text()).unwrap();
        assert_eq!(decoded, plan);
    }

    #[test]
    fn readonly_abs32_moves_to_runtime_even_when_it_only_points_to_rom() {
        let mut plan = RustInputSectionPlan::default();
        plan.push(super::PlanBucket::AllocAbs32, ".rodata..Lanon.1");
        plan.push(super::PlanBucket::RomRam, ".rodata..Lanon.1");
        assert!(plan.rom_sections.is_empty());
        assert_eq!(plan.rom_ram_sections, vec![".rodata..Lanon.1"]);
        assert_eq!(plan.alloc_abs32_sections, vec![".rodata..Lanon.1"]);
    }
}
