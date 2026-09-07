use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use goblin::elf::Elf;

mod support;
use support::cargo_bin;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn integration_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn nightly_available() -> bool {
    Command::new("cargo")
        .arg("+nightly")
        .arg("-V")
        .current_dir(workspace_root())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_build_fae_built() {
    run_ok(
        {
            let mut c = Command::new("cargo");
            c.arg("build").arg("--bin").arg("build_fae");
            c
        },
        &workspace_root(),
    );
}

fn run_ok(mut cmd: Command, where_: &Path) -> String {
    let out = cmd.current_dir(where_).output().unwrap();
    assert!(
        out.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn build_final_rust_elf(example_name: &str) -> PathBuf {
    build_final_rust_elf_for_target(example_name, "thumbv7em-none-eabi")
}

fn build_final_rust_elf_for_target(example_name: &str, target: &str) -> PathBuf {
    let workspace = workspace_root();
    let manifest = workspace
        .join("examples")
        .join(example_name)
        .join("Cargo.toml");

    run_ok(
        {
            let mut c = Command::new("cargo");
            c.arg("run")
                .arg("--bin")
                .arg("build_fae_rust")
                .arg("--")
                .arg("--manifest-path")
                .arg(&manifest)
                .arg("--elf");
            c.arg("--target").arg(target);
            c
        },
        &workspace,
    );

    workspace
        .join("examples")
        .join(example_name)
        .join("build")
        .join(format!("{example_name}.elf"))
}

fn target_available(target: &str) -> bool {
    Command::new("rustup")
        .arg("target")
        .arg("list")
        .arg("--installed")
        .output()
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|line| line.trim() == target)
        })
        .unwrap_or(false)
}

fn section_size(elf_path: &Path, section_name: &str) -> usize {
    let bytes = fs::read(elf_path).unwrap();
    let elf = Elf::parse(&bytes).unwrap();
    elf.section_headers
        .iter()
        .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(section_name))
        .map(|sh| sh.sh_size as usize)
        .unwrap_or(0)
}

fn has_lanon_symbol_in_section(elf_path: &Path, expected_section_name: &str) -> bool {
    let bytes = fs::read(elf_path).unwrap();
    let elf = Elf::parse(&bytes).unwrap();
    elf.syms.iter().any(|sym| {
        elf.strtab
            .get_at(sym.st_name)
            .is_some_and(|name| name.starts_with(".Lanon."))
            && elf
                .section_headers
                .get(sym.st_shndx)
                .and_then(|section| elf.shdr_strtab.get_at(section.sh_name))
                == Some(expected_section_name)
    })
}

#[test]
fn build_fae_rust_moves_abs32_metadata_to_runtime_block() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    let manifest = workspace
        .join("tests")
        .join("relocated_slice_location_repro_test")
        .join("Cargo.toml");
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!(
        "xiprfs-relocated-slice-location-{unique}-{}",
        std::process::id()
    ));
    let build_dir = temp_root.join("build");
    let target_dir = temp_root.join("target");
    let output_elf = build_dir.join("relocated_slice_location_repro_test.elf");
    let section_plan = build_dir.join("auto-fae-input-sections.txt");
    let linker_script = build_dir.join("auto-fae-rust.ld");
    fs::create_dir_all(&build_dir).unwrap();
    fs::write(
        &section_plan,
        "rom\t.stale.section.must.not.survive\nrom\t.rodata..Lanon.1\nrom_ram\t.rodata..Lanon.1\n",
    )
    .unwrap();

    run_ok(
        {
            let mut c = Command::new("cargo");
            c.arg("run")
                .arg("--bin")
                .arg("build_fae_rust")
                .arg("--")
                .arg("--manifest-path")
                .arg(&manifest)
                .arg("--elf");
            c.env("BUILD_FAE_BUILD_DIR", &build_dir);
            c.env("BUILD_FAE_TARGET_DIR", &target_dir);
            c
        },
        &workspace,
    );
    assert!(output_elf.is_file());
    assert!(section_size(&output_elf, ".rel.rom") > 0);
    assert!(section_size(&output_elf, ".rom.ram") > 0);
    assert!(section_size(&output_elf, ".rel.rom.ram") > 0);
    assert!(has_lanon_symbol_in_section(&output_elf, ".rom.ram"));
    let section_plan_text = fs::read_to_string(&section_plan).unwrap();
    let linker_script_text = fs::read_to_string(&linker_script).unwrap();
    assert!(!section_plan_text.contains(".stale.section.must.not.survive"));
    assert!(!linker_script_text.contains(".stale.section.must.not.survive"));

    ensure_build_fae_built();
    run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg(&output_elf);
            c
        },
        &workspace,
    );
    assert!(build_dir
        .join("relocated_slice_location_repro_test.fae")
        .is_file());
}

#[test]
fn build_fae_accepts_descriptor_repro_after_sbrel_patch() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf("relocated_descriptor_repro");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}

#[test]
fn build_fae_accepts_dyn_trait_repro_after_sbrel_patch() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf("relocated_dyn_trait_repro");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}

#[test]
fn build_fae_accepts_descriptor_repro_after_sbrel_patch_arm() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }
    if !target_available("armv7a-none-eabi") {
        eprintln!("skipping: armv7a-none-eabi target is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf_for_target("relocated_descriptor_repro", "armv7a-none-eabi");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}

#[test]
fn build_fae_accepts_cstring_repro() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf("relocated_cstring_repro");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}

#[test]
fn build_fae_rust_accepts_release_cstring_repro_with_interleaved_thumb_adds() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    let manifest = workspace
        .join("tests")
        .join("relocated_cstring_repro_test")
        .join("Cargo.toml");

    let out = run_ok(
        {
            let mut c = Command::new("cargo");
            c.arg("run")
                .arg("--bin")
                .arg("build_fae_rust")
                .arg("--")
                .arg("--manifest-path")
                .arg(&manifest)
                .arg("--target")
                .arg("thumbv7em-none-eabi")
                .arg("--elf");
            c
        },
        &workspace,
    );
    assert!(out.contains("Patched"));
}

#[test]
fn build_fae_accepts_cstring_repro_arm() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }
    if !target_available("armv7a-none-eabi") {
        eprintln!("skipping: armv7a-none-eabi target is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf_for_target("relocated_cstring_repro", "armv7a-none-eabi");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}

#[test]
fn build_fae_accepts_dyn_trait_repro_after_sbrel_patch_arm() {
    if !nightly_available() {
        eprintln!("skipping: nightly toolchain is unavailable");
        return;
    }
    if !target_available("armv7a-none-eabi") {
        eprintln!("skipping: armv7a-none-eabi target is unavailable");
        return;
    }

    let _guard = integration_lock().lock().unwrap_or_else(|e| e.into_inner());
    let workspace = workspace_root();
    ensure_build_fae_built();
    let elf = build_final_rust_elf_for_target("relocated_dyn_trait_repro", "armv7a-none-eabi");
    assert!(section_size(&elf, ".rom.ram") > 0);
    assert!(section_size(&elf, ".rel.rom.ram") > 0);

    let out = run_ok(
        {
            let mut c = Command::new(cargo_bin("build_fae"));
            c.arg("--verbose");
            c.arg(&elf);
            c
        },
        &workspace,
    );
    assert!(out.contains("Export relocation table : entries count"));
}
