use std::fmt::Write as _;
use std::fs;

use goblin::elf::section_header::{
    SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHT_NOBITS, SHT_REL, SHT_RELA,
};

use fae_core::constants;
use fae_elf::elf::parse_elf;
use fae_elf::rust_input_section_plan::RustInputSectionPlan;

#[derive(Debug, Clone)]
struct SectionDescriptor {
    name: String,
    is_alloc: bool,
    is_writable: bool,
    is_executable: bool,
    is_nobits: bool,
    has_abs32: bool,
}

impl SectionDescriptor {
    fn is_relro_candidate(&self) -> bool {
        self.name.starts_with(".data.rel.ro")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LinkPlan {
    rom_sections: Vec<String>,
    got_sections: Vec<String>,
    rom_ram_sections: Vec<String>,
    ram_sections: Vec<String>,
    discard_sections: Vec<String>,
    alloc_abs32_sections: Vec<String>,
    non_alloc_abs32_sections: Vec<String>,
}

fn usage(argv0: &str) {
    println!("Usage:");
    println!("  {argv0} <discovery.elf> <output.ld>");
    println!("  {argv0} --section-plan <plan.txt> <output.ld>");
    println!("  {argv0} --help");
    println!();
    println!(
        "Generate a FAE linker script from a Rust discovery ELF, keeping sections with \
R_ARM_ABS32 requests in the ROM-backed writable block."
    );
}

fn usage_error(argv0: &str, message: &str) -> Result<(), i32> {
    eprintln!("error: {message}");
    usage(argv0);
    Err(1)
}

impl LinkPlan {
    fn from_section_plan(plan: RustInputSectionPlan) -> Self {
        Self {
            rom_sections: plan.rom_sections,
            got_sections: plan.got_sections,
            rom_ram_sections: plan.rom_ram_sections,
            ram_sections: plan.ram_sections,
            discard_sections: plan.discard_sections,
            alloc_abs32_sections: plan.alloc_abs32_sections,
            non_alloc_abs32_sections: plan.nonalloc_abs32_sections,
        }
    }
}

fn push_unique(list: &mut Vec<String>, value: &str) {
    if !list.iter().any(|existing| existing == value) {
        list.push(value.to_owned());
    }
}

fn collect_abs32_target_names(elf_bytes: &[u8]) -> Result<Vec<String>, i32> {
    let elf = parse_elf(elf_bytes).map_err(|e| {
        eprintln!("generate_fae_rust_ld: {e}");
        1
    })?;
    let mut target_names = Vec::new();

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

        let entry_size = section.sh_entsize as usize;
        if entry_size == 0 {
            continue;
        }
        let start = section.sh_offset as usize;
        let size = section.sh_size as usize;
        let end = start.checked_add(size).ok_or_else(|| {
            eprintln!("generate_fae_rust_ld: relocation section end overflow");
            1
        })?;
        if end > elf_bytes.len() {
            eprintln!("generate_fae_rust_ld: relocation section extends beyond file bounds");
            return Err(1);
        }

        for off in (start..end).step_by(entry_size) {
            if off + 8 > elf_bytes.len() {
                eprintln!("generate_fae_rust_ld: truncated relocation entry");
                return Err(1);
            }

            let r_info = u32::from_le_bytes([
                elf_bytes[off + 4],
                elf_bytes[off + 5],
                elf_bytes[off + 6],
                elf_bytes[off + 7],
            ]);
            if (r_info & 0xff) == constants::R_ARM_ABS32 {
                push_unique(&mut target_names, target_name);
                break;
            }
        }
    }

    Ok(target_names)
}

fn classify_sections(sections: &[SectionDescriptor]) -> LinkPlan {
    let mut plan = LinkPlan::default();

    for section in sections {
        if section.name == ".ARM.exidx" || section.name == ".ARM.extab" {
            push_unique(&mut plan.discard_sections, &section.name);
            continue;
        }

        if !section.is_alloc {
            if section.has_abs32 {
                push_unique(&mut plan.non_alloc_abs32_sections, &section.name);
            }
            continue;
        }

        if section.name.starts_with(".got") {
            push_unique(&mut plan.got_sections, &section.name);
            continue;
        }

        if section.is_relro_candidate() {
            if section.has_abs32 {
                push_unique(&mut plan.alloc_abs32_sections, &section.name);
                push_unique(&mut plan.rom_ram_sections, &section.name);
            } else {
                push_unique(&mut plan.rom_sections, &section.name);
            }
            continue;
        }

        if section.is_writable {
            if section.is_nobits {
                push_unique(&mut plan.ram_sections, &section.name);
            } else {
                push_unique(&mut plan.rom_ram_sections, &section.name);
            }
            continue;
        }

        if section.has_abs32 && !section.is_executable {
            push_unique(&mut plan.alloc_abs32_sections, &section.name);
            push_unique(&mut plan.rom_ram_sections, &section.name);
            continue;
        }

        if section.is_nobits {
            push_unique(&mut plan.ram_sections, &section.name);
            continue;
        }

        push_unique(&mut plan.rom_sections, &section.name);
    }

    plan
}

fn build_link_plan(elf_bytes: &[u8]) -> Result<LinkPlan, i32> {
    let elf = parse_elf(elf_bytes).map_err(|e| {
        eprintln!("generate_fae_rust_ld: {e}");
        1
    })?;
    let abs32_targets = collect_abs32_target_names(elf_bytes)?;

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
            has_abs32: abs32_targets.iter().any(|target| target == name),
        });
    }

    Ok(classify_sections(&sections))
}

fn input_section_pattern(section: &str) -> String {
    section.to_owned()
}

fn render_section_list(out: &mut String, sections: &[String]) {
    for section in sections {
        let pattern = input_section_pattern(section);
        let _ = writeln!(out, "        *({pattern})");
    }
}

fn render_link_script(plan: &LinkPlan) -> String {
    let mut out = String::new();

    out.push_str(
        "OUTPUT_FORMAT(\n    \"elf32-littlearm\",\n    \"elf32-littlearm\",\n    \
\"elf32-littlearm\"\n)\n",
    );
    out.push_str("OUTPUT_ARCH(arm)\n");
    out.push_str("ENTRY(start)\n\n");

    out.push_str("/* Auto-generated by generate_fae_rust_ld. */\n");
    if plan.alloc_abs32_sections.is_empty() {
        out.push_str("/* No allocatable R_ARM_ABS32 target sections were discovered. */\n");
    } else {
        out.push_str("/* Allocatable sections receiving R_ARM_ABS32 relocations: */\n");
        for section in &plan.alloc_abs32_sections {
            let _ = writeln!(out, "/*   {section} */");
        }
    }
    if !plan.non_alloc_abs32_sections.is_empty() {
        out.push_str("/* Non-allocatable sections also receiving R_ARM_ABS32 relocations: */\n");
        for section in &plan.non_alloc_abs32_sections {
            let _ = writeln!(out, "/*   {section} */");
        }
    }
    out.push('\n');

    // Keep the RWPI static base coherent: lld derives ARM BREL/SBREL offsets
    // from the PT_LOAD segment containing each symbol, while rt0 installs one
    // runtime writable-window base in r9.
    out.push_str("PHDRS\n{\n");
    out.push_str("    rx PT_LOAD FLAGS(5);\n"); // R E
    out.push_str("    rw PT_LOAD FLAGS(6);\n"); // R W
    out.push_str("}\n\n");

    out.push_str("SECTIONS\n{\n");

    out.push_str("    .rom :\n    {\n        . = ALIGN(4);\n        __rom_start = .;\n");
    render_section_list(&mut out, &plan.rom_sections);
    out.push_str("        . = ALIGN(4);\n        __rom_end = .;\n    } :rx\n");
    out.push_str("    __rom_size = __rom_end - __rom_start;\n\n");

    out.push_str("    .got :\n    {\n        . = ALIGN(4);\n        __got_start = .;\n");
    render_section_list(&mut out, &plan.got_sections);
    out.push_str("        . = ALIGN(4);\n        __got_end = .;\n    } :rw\n");
    out.push_str("    __got_size = __got_end - __got_start;\n\n");

    out.push_str("    .rom.ram :\n    {\n        . = ALIGN(4);\n        __rom_ram_start = .;\n");
    render_section_list(&mut out, &plan.rom_ram_sections);
    out.push_str("        . = ALIGN(4);\n        __rom_ram_end = .;\n    } :rw\n");
    out.push_str("    __rom_ram_size = __rom_ram_end - __rom_ram_start;\n\n");

    out.push_str("    .ram (NOLOAD) :\n    {\n        . = ALIGN(4);\n        __ram_start = .;\n");
    render_section_list(&mut out, &plan.ram_sections);
    out.push_str("        *(COMMON)\n");
    out.push_str("        . = ALIGN(4);\n        __ram_end = .;\n    } :rw\n");
    out.push_str("    __ram_size = __ram_end - __ram_start;\n\n");

    if !plan.discard_sections.is_empty() {
        out.push_str("    /DISCARD/ :\n    {\n");
        render_section_list(&mut out, &plan.discard_sections);
        out.push_str("    }\n");
    }
    out.push_str("}\n");

    out
}

pub fn render_link_script_from_section_plan(plan: RustInputSectionPlan) -> String {
    render_link_script(&LinkPlan::from_section_plan(plan))
}

pub fn render_link_script_from_elf_bytes(elf_bytes: &[u8]) -> Result<String, i32> {
    let plan = build_link_plan(elf_bytes)?;
    Ok(render_link_script(&plan))
}

pub fn run(args: &[String]) -> Result<(), i32> {
    match args {
        [argv0, help] if help == "-h" || help == "--help" => {
            usage(argv0);
            Ok(())
        }
        [_argv0, option, input_plan, output_ld] if option == "--section-plan" => {
            let plan = RustInputSectionPlan::read_from_path(std::path::Path::new(input_plan))
                .map(LinkPlan::from_section_plan)
                .map_err(|e| {
                    eprintln!("generate_fae_rust_ld: {e}");
                    1
                })?;
            let link_script = render_link_script(&plan);
            fs::write(output_ld, link_script).map_err(|e| {
                eprintln!("generate_fae_rust_ld: {e}");
                1
            })?;

            println!("- Section plan : {input_plan}");
            println!("- Output linker script : {output_ld}");
            println!(
                "- .rom : {} sections, .got : {} sections, .rom.ram : {} sections, .ram : {} sections",
                plan.rom_sections.len(),
                plan.got_sections.len(),
                plan.rom_ram_sections.len(),
                plan.ram_sections.len()
            );
            if !plan.alloc_abs32_sections.is_empty() {
                println!(
                    "- Allocatable ABS32 sections projected to .rom.ram : {}",
                    plan.alloc_abs32_sections.join(", ")
                );
            }
            if !plan.non_alloc_abs32_sections.is_empty() {
                println!(
                    "- Non-allocatable ABS32 sections left outside FAE payload layout : {}",
                    plan.non_alloc_abs32_sections.join(", ")
                );
            }
            Ok(())
        }
        [_argv0, input_elf, output_ld] => {
            let elf_bytes = fs::read(input_elf).map_err(|e| {
                eprintln!("generate_fae_rust_ld: {e}");
                1
            })?;
            let plan = build_link_plan(&elf_bytes)?;
            let link_script = render_link_script(&plan);
            fs::write(output_ld, link_script).map_err(|e| {
                eprintln!("generate_fae_rust_ld: {e}");
                1
            })?;

            println!("- Discovery ELF : {input_elf}");
            println!("- Output linker script : {output_ld}");
            println!(
                "- .rom : {} sections, .got : {} sections, .rom.ram : {} sections, .ram : {} sections",
                plan.rom_sections.len(),
                plan.got_sections.len(),
                plan.rom_ram_sections.len(),
                plan.ram_sections.len()
            );
            if !plan.alloc_abs32_sections.is_empty() {
                println!(
                    "- Allocatable ABS32 sections projected to .rom.ram : {}",
                    plan.alloc_abs32_sections.join(", ")
                );
            }
            if !plan.non_alloc_abs32_sections.is_empty() {
                println!(
                    "- Non-allocatable ABS32 sections left outside FAE payload layout : {}",
                    plan.non_alloc_abs32_sections.join(", ")
                );
            }
            Ok(())
        }
        [argv0, ..] => usage_error(
            argv0,
            "expected <discovery.elf> <output.ld> or --section-plan <plan.txt> <output.ld>",
        ),
        [] => Err(1),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_sections, render_link_script, LinkPlan, SectionDescriptor};

    #[test]
    fn classify_sections_moves_alloc_abs32_metadata_to_rom_ram() {
        let plan = classify_sections(&[
            SectionDescriptor {
                name: ".text.foo".to_owned(),
                is_alloc: true,
                is_writable: false,
                is_executable: true,
                is_nobits: false,
                has_abs32: false,
            },
            SectionDescriptor {
                name: ".rodata..Lanon.1".to_owned(),
                is_alloc: true,
                is_writable: false,
                is_executable: false,
                is_nobits: false,
                has_abs32: true,
            },
            SectionDescriptor {
                name: ".rodata.payload".to_owned(),
                is_alloc: true,
                is_writable: false,
                is_executable: false,
                is_nobits: false,
                has_abs32: false,
            },
            SectionDescriptor {
                name: ".data.foo".to_owned(),
                is_alloc: true,
                is_writable: true,
                is_executable: false,
                is_nobits: false,
                has_abs32: false,
            },
            SectionDescriptor {
                name: ".bss.foo".to_owned(),
                is_alloc: true,
                is_writable: true,
                is_executable: false,
                is_nobits: true,
                has_abs32: false,
            },
            SectionDescriptor {
                name: ".debug_info".to_owned(),
                is_alloc: false,
                is_writable: false,
                is_executable: false,
                is_nobits: false,
                has_abs32: true,
            },
        ]);

        assert_eq!(plan.rom_sections, vec![".text.foo", ".rodata.payload"]);
        assert_eq!(plan.rom_ram_sections, vec![".rodata..Lanon.1", ".data.foo"]);
        assert_eq!(plan.ram_sections, vec![".bss.foo"]);
        assert_eq!(plan.alloc_abs32_sections, vec![".rodata..Lanon.1"]);
        assert_eq!(plan.non_alloc_abs32_sections, vec![".debug_info"]);
    }

    #[test]
    fn classify_sections_moves_all_readonly_abs32_metadata_to_rom_ram() {
        let plan = classify_sections(&[
            SectionDescriptor {
                name: ".rodata.meta".to_owned(),
                is_alloc: true,
                is_writable: false,
                is_executable: false,
                is_nobits: false,
                has_abs32: true,
            },
            SectionDescriptor {
                name: ".rodata.bridge".to_owned(),
                is_alloc: true,
                is_writable: false,
                is_executable: false,
                is_nobits: false,
                has_abs32: true,
            },
            SectionDescriptor {
                name: ".data.foo".to_owned(),
                is_alloc: true,
                is_writable: true,
                is_executable: false,
                is_nobits: false,
                has_abs32: false,
            },
        ]);

        assert_eq!(plan.rom_sections, Vec::<String>::new());
        assert_eq!(
            plan.rom_ram_sections,
            vec![".rodata.meta", ".rodata.bridge", ".data.foo"]
        );
        assert_eq!(
            plan.alloc_abs32_sections,
            vec![".rodata.meta", ".rodata.bridge"]
        );
    }

    #[test]
    fn render_link_script_mentions_abs32_sections() {
        let text = render_link_script(&LinkPlan {
            rom_sections: vec![".text.foo".to_owned()],
            got_sections: vec![".got".to_owned()],
            rom_ram_sections: vec![".rodata..Lanon.1".to_owned(), ".data.foo".to_owned()],
            ram_sections: vec![".bss.foo".to_owned()],
            discard_sections: vec![".ARM.exidx.foo".to_owned()],
            alloc_abs32_sections: vec![".rodata..Lanon.1".to_owned()],
            non_alloc_abs32_sections: vec![".debug_info".to_owned()],
        });

        assert!(text.contains("Allocatable sections receiving R_ARM_ABS32 relocations"));
        assert!(text.contains("*(.rodata..Lanon.1)"));
        assert!(text.contains("*(.data.foo)"));
        assert!(text.contains("*(.bss.foo)"));
        assert!(text.contains("*(COMMON)"));
    }

    #[test]
    fn render_link_script_groups_runtime_ram_sections_in_rw_segment() {
        let text = render_link_script(&LinkPlan {
            rom_sections: vec![".text".to_owned()],
            got_sections: vec![".got".to_owned()],
            rom_ram_sections: vec![".rodata.descriptor".to_owned()],
            ram_sections: vec![".bss.heap".to_owned()],
            discard_sections: Vec::new(),
            alloc_abs32_sections: Vec::new(),
            non_alloc_abs32_sections: Vec::new(),
        });

        assert!(text.contains("PHDRS"));
        assert!(text.contains("rx PT_LOAD FLAGS(5);"));
        assert!(text.contains("rw PT_LOAD FLAGS(6);"));
        assert!(text.contains("__rom_end = .;\n    } :rx"));
        assert!(text.contains("__got_end = .;\n    } :rw"));
        assert!(text.contains("__rom_ram_end = .;\n    } :rw"));
        assert!(text.contains("__ram_end = .;\n    } :rw"));
    }

    #[test]
    fn render_link_script_uses_exact_base_sections() {
        let text = render_link_script(&LinkPlan {
            rom_sections: vec![".text".to_owned()],
            got_sections: vec![".got".to_owned()],
            rom_ram_sections: vec![".rodata".to_owned(), ".data".to_owned()],
            ram_sections: vec![".bss".to_owned()],
            discard_sections: vec![".ARM.exidx".to_owned(), ".ARM.extab".to_owned()],
            alloc_abs32_sections: vec![".rodata".to_owned()],
            non_alloc_abs32_sections: Vec::new(),
        });

        assert!(text.contains("*(.text)"));
        assert!(text.contains("*(.got)"));
        assert!(text.contains("*(.rodata)"));
        assert!(text.contains("*(.data)"));
        assert!(text.contains("*(.bss)"));
        assert!(text.contains("/DISCARD/"));
        assert!(text.contains("*(.ARM.exidx)"));
    }
}
