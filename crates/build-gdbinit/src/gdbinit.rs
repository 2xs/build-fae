use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::constants;
use crate::fae_format::ExportedSymbols;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GdbMemoryLayout {
    Relocatable { flash_base_hint: Option<u32> },
    Fixed { flash_base: u32, ram_base: u32 },
}

fn path_from_gdbinit_dir(target: &Path, gdbinit_filename: &Path) -> Result<PathBuf> {
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };

    let gdbinit_dir = gdbinit_filename
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let gdbinit_dir = if gdbinit_dir.is_absolute() {
        gdbinit_dir
    } else {
        std::env::current_dir()?.join(gdbinit_dir)
    };

    let relative = target
        .strip_prefix(&gdbinit_dir)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| target.clone());
    Ok(relative)
}

#[allow(clippy::too_many_arguments)]
pub fn generate_gdbinit(
    app_elf_filename: Option<&str>,
    startup_elf_path: &Path,
    gdbinit_filename: &Path,
    metadata_size: usize,
    exported_symbols: Option<ExportedSymbols>,
    ram_size: u32,
    memory_layout: GdbMemoryLayout,
    verbose: bool,
) -> Result<()> {
    let startup_symbol_path = path_from_gdbinit_dir(startup_elf_path, gdbinit_filename)?;

    let mut data = String::new();
    append_memory_layout_prelude(&mut data, memory_layout);
    data.push_str("set $startup_text = $flash_base\n");
    data.push_str(&format!(
        "add-symbol-file {} -s {} $startup_text\n",
        startup_symbol_path.display(),
        constants::STARTUP_SECTION_NAME
    ));

    if let (Some(elf_filename), Some(exported_symbols)) = (app_elf_filename, exported_symbols) {
        let text_size = exported_symbols.rom_size;
        let got_size = exported_symbols.got_size;
        let data_size = exported_symbols.rom_ram_size;
        let writable_ram_size = exported_symbols.ram_size;
        let app_symbol_path = path_from_gdbinit_dir(Path::new(elf_filename), gdbinit_filename)?;

        data.push_str(&format!("set $text = $startup_text + {}\n", metadata_size));
        data.push_str(&format!("set $got = $text + {}\n", text_size));
        data.push_str(&format!("set $data_image = $got + {}\n", got_size));
        data.push_str("set $rel_got = $ram_base\n");
        data.push_str(&format!("set $rel_data = $rel_got + {}\n", got_size));
        data.push_str(&format!("set $rel_ram_tail = $rel_data + {}\n", data_size));
        data.push_str(&format!(
            "add-symbol-file {} -s .text $text -s .got $rel_got -s .rom.ram $rel_data -s .ram $rel_ram_tail\n",
            app_symbol_path.display()
        ));
        data.push_str(&format!(
            "set $flash_end = $flash_base + {}\n",
            metadata_size as u32 + text_size + got_size + data_size
        ));
        data.push_str(&format!(
            "set $ram_end = $ram_base + {}\n",
            got_size + data_size + writable_ram_size
        ));
    } else {
        data.push_str(&format!(
            "set $flash_end = $flash_base + {}\n",
            metadata_size as u32
        ));
        data.push_str(&format!("set $ram_end = $ram_base + {}\n", ram_size));
    }

    fs::write(gdbinit_filename, data)
        .with_context(|| format!("cannot write {}", gdbinit_filename.display()))?;
    if verbose {
        println!("{} has been generated.", gdbinit_filename.display());
    }

    Ok(())
}

fn append_memory_layout_prelude(out: &mut String, memory_layout: GdbMemoryLayout) {
    match memory_layout {
        GdbMemoryLayout::Relocatable { flash_base_hint } => {
            out.push_str("# Set this to the load base of the startup blob in ROM/flash.\n");
            if let Some(flash_base_hint) = flash_base_hint {
                out.push_str(&format!("set $flash_base = 0x{flash_base_hint:08x}\n"));
            } else {
                out.push_str("set $flash_base = 0x00000000\n");
            }
            out.push_str(
                "# Set this to the relocated data base of the loaded image.\n\
# In the current ARM convention, this is the value held in r9.\n",
            );
            out.push_str("set $ram_base = $r9\n");
        }
        GdbMemoryLayout::Fixed {
            flash_base,
            ram_base,
        } => {
            out.push_str("# Fixed firmware memory map selected from the bundled board profile.\n");
            out.push_str(&format!("set $flash_base = 0x{flash_base:08x}\n"));
            out.push_str(&format!("set $ram_base = 0x{ram_base:08x}\n"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{append_memory_layout_prelude, GdbMemoryLayout};

    #[test]
    fn relocatable_gdbinit_uses_r9_for_ram_base() {
        let mut out = String::new();
        append_memory_layout_prelude(
            &mut out,
            GdbMemoryLayout::Relocatable {
                flash_base_hint: None,
            },
        );

        assert!(out.contains("set $ram_base = $r9"));
        assert!(out.contains("value held in r9"));
    }

    #[test]
    fn fixed_gdbinit_uses_known_firmware_bases() {
        let mut out = String::new();
        append_memory_layout_prelude(
            &mut out,
            GdbMemoryLayout::Fixed {
                flash_base: 0x0800_0000,
                ram_base: 0x2000_0000,
            },
        );

        assert!(out.contains("set $flash_base = 0x08000000"));
        assert!(out.contains("set $ram_base = 0x20000000"));
    }
}
