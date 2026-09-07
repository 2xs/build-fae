use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod support;
use support::cargo_bin;

fn run_ok(mut cmd: Command, workdir: &Path) -> String {
    let output = cmd.current_dir(workdir).output().unwrap();
    if !output.status.success() {
        panic!(
            "command failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn arm_gcc_is_available() -> bool {
    Command::new("arm-none-eabi-gcc")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn compile_thumb_object(workdir: &Path, output_name: &str) -> PathBuf {
    fs::write(
        workdir.join("main.S"),
        r#".syntax unified
.thumb

.section .text
.global start
.type start, %function
start:
    bl ext_helper
    bx lr

.global helper
.type helper, %function
helper:
    movs r0, #42
    bx lr

.extern ext_helper

.section .rodata
.align 2
table:
    .word helper + 1
"#,
    )
    .unwrap();

    let object = workdir.join(output_name);
    run_ok(
        {
            let mut c = Command::new("arm-none-eabi-gcc");
            c.arg("-c")
                .arg("-mthumb")
                .arg("-mcpu=cortex-m4")
                .arg("main.S")
                .arg("-o")
                .arg(&object);
            c
        },
        workdir,
    );
    object
}

#[test]
fn check_elf_rel_lists_relocated_sections() {
    if !arm_gcc_is_available() {
        eprintln!("skipped: arm-none-eabi-gcc is not available");
        return;
    }

    let td = tempfile::tempdir().unwrap();
    let object = compile_thumb_object(td.path(), "main.o");

    let output = run_ok(
        {
            let mut c = Command::new(cargo_bin("check_elf_rel"));
            c.arg(&object);
            c
        },
        td.path(),
    );

    assert!(output.contains(
        "- Sections with relocation requests : 2 target sections via 2 relocation sections"
    ));
    assert!(output.contains("- .text"));
    assert!(output.contains(".rel.text (REL, 1 entries)"));
    assert!(output.contains("- .rodata"));
    assert!(output.contains(".rel.rodata (REL, 1 entries)"));
}

#[test]
fn check_elf_rel_abs32_only_filters_non_abs32_sections() {
    if !arm_gcc_is_available() {
        eprintln!("skipped: arm-none-eabi-gcc is not available");
        return;
    }

    let td = tempfile::tempdir().unwrap();
    let object = compile_thumb_object(td.path(), "main.o");

    let output = run_ok(
        {
            let mut c = Command::new(cargo_bin("check_elf_rel"));
            c.arg("--abs32-only").arg(&object);
            c
        },
        td.path(),
    );

    assert!(output.contains("- Filter : R_ARM_ABS32 only"));
    assert!(output.contains(
        "- Sections with relocation requests : 1 target sections via 1 relocation sections"
    ));
    assert!(output.contains("- .rodata"));
    assert!(output.contains(".rel.rodata (REL, 1 entries)"));
    assert!(!output.contains("- .text"));
}
