use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use goblin::elf::Elf;

mod support;
use support::cargo_bin;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_elf() -> PathBuf {
    workspace_root().join("build/hello-world.elf")
}

fn integration_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn prepare_fixture_workspace() -> Option<tempfile::TempDir> {
    let hello = fixture_elf();
    if !hello.exists() {
        return None;
    }

    let td = tempfile::tempdir().ok()?;
    let work = td.path();
    fs::create_dir_all(work.join("build")).ok()?;
    fs::copy(hello, work.join("build/hello-world.elf")).ok()?;
    Some(td)
}

fn run_ok(mut cmd: Command, where_: &Path) -> String {
    let out = cmd.current_dir(where_).output().unwrap();
    assert!(
        out.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn ensure_rustrt0_built() {
    let status = Command::new("cargo")
        .arg("rustrt0-thumb")
        .current_dir(workspace_root())
        .status()
        .unwrap();
    assert!(
        status.success(),
        "cargo rustrt0-thumb failed with status {status}"
    );
    assert!(workspace_root().join("build/rustrt0-thumb.bin").exists());
}

#[derive(Debug)]
struct SectionInfo {
    size: usize,
}

#[derive(Debug)]
struct ElfExpectations {
    rom_size: usize,
    got_size: usize,
    rom_ram_size: usize,
    ram_size: usize,
    entrypoint: usize,
    relocation_count: usize,
}

fn parse_elf_expectations(elf_path: &Path) -> ElfExpectations {
    let bytes = fs::read(elf_path).unwrap();
    let elf = Elf::parse(&bytes).unwrap();

    let mut sections: HashMap<&str, SectionInfo> = HashMap::new();
    for sh in &elf.section_headers {
        if let Some(name) = elf.shdr_strtab.get_at(sh.sh_name) {
            sections.insert(
                name,
                SectionInfo {
                    size: sh.sh_size as usize,
                },
            );
        }
    }

    let mut symbols = HashMap::new();
    for sym in &elf.syms {
        if sym.st_name == 0 {
            continue;
        }
        if let Some(name) = elf.strtab.get_at(sym.st_name) {
            symbols.insert(name.to_string(), sym.st_value as usize);
        }
    }

    let relocation_count = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(".rel.rom.ram"))
        .map(|sh| {
            (sh.sh_size as usize)
                .checked_div(sh.sh_entsize as usize)
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let pad4 = |size: usize| (size + 3) & !3;

    ElfExpectations {
        rom_size: pad4(sections.get(".rom").map(|s| s.size).unwrap_or(0)),
        got_size: pad4(sections.get(".got").map(|s| s.size).unwrap_or(0)),
        rom_ram_size: pad4(sections.get(".rom.ram").map(|s| s.size).unwrap_or(0)),
        ram_size: pad4(sections.get(".ram").map(|s| s.size).unwrap_or(0)),
        entrypoint: *symbols.get("start").unwrap(),
        relocation_count,
    }
}

fn read_le_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn partition_offset(fae: &[u8], startup_code_size: usize, reloc_count: usize) -> usize {
    let payload_padding_offset = startup_code_size + 4 + 4 + reloc_count * 4;
    let payload_padding = read_le_u32(fae, payload_padding_offset) as usize;
    payload_padding_offset + 4 + payload_padding
}

fn extract_section_bytes(elf_path: &Path, section_name: &str) -> Vec<u8> {
    let bytes = fs::read(elf_path).unwrap();
    let elf = Elf::parse(&bytes).unwrap();
    let section = elf
        .section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(section_name))
        .unwrap();
    let start = section.sh_offset as usize;
    let end = start + section.sh_size as usize;
    bytes[start..end].to_vec()
}

fn pad_section_bytes(bytes: &mut Vec<u8>) {
    let padded = (bytes.len() + 3) & !3;
    bytes.resize(padded, 0);
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

fn render_hexdump(data: &[u8]) -> String {
    let mut out = String::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let these_bytes = &data[offset..offset + remaining.min(16)];

        let (a, b) = if these_bytes.len() > 8 {
            let mut b = hex_space(&these_bytes[8..]);
            while b.len() < 23 {
                b.push(' ');
            }
            (hex_space(&these_bytes[..8]), b)
        } else {
            (
                hex_space(these_bytes),
                "                                   ".to_string(),
            )
        };

        let mut ascii = String::from("|");
        for byte in these_bytes {
            let ch = *byte as char;
            if ch.is_ascii() && !ch.is_ascii_control() {
                ascii.push(ch);
            } else {
                ascii.push('.');
            }
        }
        while ascii.len() < 17 {
            ascii.push(' ');
        }
        ascii.push('|');

        out.push_str(&format!("{offset:08x}  {a}  {b}  {ascii}\n"));
        offset += these_bytes.len();
    }
    out
}

fn build_fae_in(workdir: &Path) {
    run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--facade12").arg("build/hello-world.elf");
            c
        },
        workdir,
    );
}

#[test]
fn read_fae_output_matches_original_elf() {
    let _guard = integration_lock().lock().unwrap();
    ensure_rustrt0_built();
    let Some(td) = prepare_fixture_workspace() else {
        eprintln!("Skipping: build/hello-world.elf fixture not available");
        return;
    };
    let workdir = td.path();

    build_fae_in(workdir);

    let rs_out = run_ok(
        {
            let mut c = Command::new(cargo_bin("read_fae"));
            c.arg("build/hello-world.fae");
            c
        },
        workdir,
    );

    let elf = parse_elf_expectations(&workdir.join("build/hello-world.elf"));
    let fae = fs::read(workdir.join("build/hello-world.fae")).unwrap();
    let binary_size = fae.len();
    let footer = binary_size - 28;
    let magic = read_le_u32(&fae, footer + 24);
    let startup_code_size = read_le_u32(&fae, footer + 20) as usize;
    let binary_size_from_file = read_le_u32(&fae, startup_code_size) as usize;
    let reloc_count_from_file = read_le_u32(&fae, startup_code_size + 4) as usize;

    assert_eq!(binary_size, binary_size_from_file);
    assert_eq!(elf.relocation_count, reloc_count_from_file);

    let partition_offset = partition_offset(&fae, startup_code_size, reloc_count_from_file);

    let rom_start = partition_offset;
    let rom_end = rom_start + elf.rom_size - 1;
    let got_start = rom_end + 1;
    let got_end = got_start + elf.got_size - 1;
    let rom_ram_start = got_end + 1;

    assert!(rs_out.contains(&format!("- Binary size : {} bytes", binary_size)));
    assert!(rs_out.contains(&format!("- Magic Number And Version : {:#x}", magic)));
    assert!(rs_out.contains(&format!(
        "- startup code : [start : @0 byte , end : @{} bytes] (size : {} bytes)",
        startup_code_size - 1,
        startup_code_size
    )));
    if elf.relocation_count == 0 {
        assert!(rs_out.contains("- Relocation entries : none."));
    } else {
        assert!(rs_out.contains(&format!("\t- Count : {}", elf.relocation_count)));
    }
    assert!(rs_out.contains(&format!(
        "- Offset to partition : {} bytes",
        partition_offset
    )));
    let entrypoint_mode = if (elf.entrypoint & 1) != 0 {
        "Thumb"
    } else {
        "ARM"
    };
    assert!(rs_out.contains(&format!("- Entrypoint value : {} bytes", elf.entrypoint)));
    assert!(rs_out.contains(&format!("- Entrypoint mode : {}", entrypoint_mode)));
    assert!(rs_out.contains(&format!(
        "- Entrypoint offset in .rom : {} bytes",
        elf.entrypoint & !1
    )));
    assert!(rs_out.contains(&format!(
        "- .rom : [start : @{} bytes , end : @{} bytes] (size : {} bytes)",
        rom_start, rom_end, elf.rom_size
    )));
    assert!(rs_out.contains(&format!(
        "- .got : [start : @{} bytes , end : @{} bytes] (size : {} bytes)",
        got_start, got_end, elf.got_size
    )));
    if elf.rom_ram_size == 0 {
        assert!(rs_out.contains("- .rom.ram : none."));
    } else {
        assert!(rs_out.contains(&format!(
            "- .rom.ram : [start : @{} bytes , end : @{} bytes] (size : {} bytes)",
            rom_ram_start,
            rom_ram_start + elf.rom_ram_size - 1,
            elf.rom_ram_size
        )));
    }
    assert!(rs_out.contains(&format!(
        "- writable RAM (.data + .bss) : {} bytes",
        elf.ram_size
    )));
    assert!(rs_out.contains("- Integrity check : OK"));
}

#[test]
fn build_fae_writes_partition_in_canonical_section_order() {
    let _guard = integration_lock().lock().unwrap();
    ensure_rustrt0_built();
    let Some(td) = prepare_fixture_workspace() else {
        eprintln!("Skipping: build/hello-world.elf fixture not available");
        return;
    };
    let workdir = td.path();

    build_fae_in(workdir);

    let elf_path = workdir.join("build/hello-world.elf");
    let mut rom = extract_section_bytes(&elf_path, ".rom");
    let mut got = extract_section_bytes(&elf_path, ".got");
    let mut rom_ram = extract_section_bytes(&elf_path, ".rom.ram");
    pad_section_bytes(&mut rom);
    pad_section_bytes(&mut got);
    pad_section_bytes(&mut rom_ram);

    let mut expected_partition = Vec::new();
    expected_partition.extend_from_slice(&rom);
    expected_partition.extend_from_slice(&got);
    expected_partition.extend_from_slice(&rom_ram);

    let fae = fs::read(workdir.join("build/hello-world.fae")).unwrap();
    let binary_size = fae.len();
    let footer = binary_size - 28;
    let startup_code_size = read_le_u32(&fae, footer + 20) as usize;
    let reloc_count = read_le_u32(&fae, startup_code_size + 4) as usize;

    let partition_offset = partition_offset(&fae, startup_code_size, reloc_count);

    assert_eq!(
        &fae[partition_offset..partition_offset + expected_partition.len()],
        expected_partition.as_slice()
    );
}

#[test]
fn read_fae_hexdumps_match_original_elf_section_contents() {
    let _guard = integration_lock().lock().unwrap();
    ensure_rustrt0_built();
    let Some(td) = prepare_fixture_workspace() else {
        eprintln!("Skipping: build/hello-world.elf fixture not available");
        return;
    };
    let workdir = td.path();

    build_fae_in(workdir);

    let rs_out = run_ok(
        {
            let mut c = Command::new(cargo_bin("read_fae"));
            c.arg("-Hrom")
                .arg("-Hgot")
                .arg("-Hromram")
                .arg("build/hello-world.fae");
            c
        },
        workdir,
    );

    let elf_path = workdir.join("build/hello-world.elf");
    let rom = extract_section_bytes(&elf_path, ".rom");
    let got = extract_section_bytes(&elf_path, ".got");
    let rom_ram = extract_section_bytes(&elf_path, ".rom.ram");

    let rom_dump = render_hexdump(&rom);
    let got_dump = render_hexdump(&got);
    let rom_ram_dump = render_hexdump(&rom_ram);

    assert!(rs_out.contains(&rom_dump), "missing .rom hexdump\n{rs_out}");
    assert!(rs_out.contains(&got_dump), "missing .got hexdump\n{rs_out}");
    assert!(
        rs_out.contains(&rom_ram_dump),
        "missing .rom.ram hexdump\n{rs_out}"
    );
}

#[test]
fn gdbinit_matches_original_elf_layout() {
    let _guard = integration_lock().lock().unwrap();
    ensure_rustrt0_built();
    let Some(td) = prepare_fixture_workspace() else {
        eprintln!("Skipping: build/hello-world.elf fixture not available");
        return;
    };
    let workdir = td.path();

    build_fae_in(workdir);

    let elf = parse_elf_expectations(&workdir.join("build/hello-world.elf"));
    let fae = fs::read(workdir.join("build/hello-world.fae")).unwrap();
    let binary_size = fae.len();
    let footer = binary_size - 28;
    let startup_code_size = read_le_u32(&fae, footer + 20) as usize;
    let reloc_count = read_le_u32(&fae, startup_code_size + 4) as usize;
    let partition_offset = partition_offset(&fae, startup_code_size, reloc_count);

    let gdbinit = fs::read_to_string(workdir.join("build/hello-world.gdbinit")).unwrap();
    let expected = format!(
        "# Set this to the load base of the startup blob in ROM/flash.\n\
set $flash_base = 0x00000000\n\
# Set this to the relocated data base of the loaded image.\n\
# In the current ARM convention, this is the value held in r9.\n\
set $ram_base = $r9\n\
set $startup_text = $flash_base\n\
add-symbol-file {startup_elf} -s ._start $startup_text\n\
set $text = $startup_text + {partition_offset}\n\
set $got = $text + {rom_size}\n\
set $data_image = $got + {got_size}\n\
set $rel_got = $ram_base\n\
set $rel_data = $rel_got + {got_size}\n\
set $rel_ram_tail = $rel_data + {rom_ram_size}\n\
add-symbol-file {elf_path} -s .text $text -s .got $rel_got -s .rom.ram $rel_data -s .ram $rel_ram_tail\n\
set $flash_end = $flash_base + {flash_end}\n\
set $ram_end = $ram_base + {ram_end}\n",
        partition_offset = partition_offset,
        rom_size = elf.rom_size,
        got_size = elf.got_size,
        rom_ram_size = elf.rom_ram_size,
        startup_elf = "build_fae-rt0-thumb.elf",
        elf_path = "hello-world.elf",
        flash_end = partition_offset + elf.rom_size + elf.got_size + elf.rom_ram_size,
        ram_end = elf.got_size + elf.rom_ram_size + elf.ram_size,
    );

    assert_eq!(gdbinit, expected);
}
