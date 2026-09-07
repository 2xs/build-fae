use std::fs;
use std::process::{Command, Output};

use tempfile::tempdir;

mod support;
use support::cargo_bin;

fn run(name: &str, args: &[&str]) -> Output {
    Command::new(cargo_bin(name))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to execute {name}: {error}"))
}

fn combined_output(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

#[test]
fn build_fae_help_describes_the_current_rt0_contract() {
    let output = run("build_fae", &["--help"]);
    let text = combined_output(&output);

    assert!(output.status.success(), "build_fae --help failed:\n{text}");
    assert!(text.contains("--rt0 <thumb|thumbv6m|arm|file.elf>"));
    assert!(text.contains("--startup-alone"));
    assert!(!text.contains("--startup-code"));
}

#[test]
fn build_fae_rejects_the_removed_startup_option() {
    let output = run("build_fae", &["--startup", "thumb", "payload.elf"]);
    let text = combined_output(&output);

    assert!(!output.status.success());
    assert!(text.contains("unrecognized option: --startup"));
    assert!(text.contains("--rt0"));
}

#[test]
fn build_fae_rejects_non_elf_input() {
    let directory = tempdir().expect("create temporary test directory");
    let input = directory.path().join("not-an-elf.bin");
    fs::write(&input, b"not an ELF file").expect("write invalid ELF fixture");

    let output = Command::new(cargo_bin("build_fae"))
        .arg(&input)
        .output()
        .expect("execute build_fae");
    let text = combined_output(&output);

    assert!(!output.status.success());
    assert!(text.contains("not an ELF file") || text.contains("ELF"));
}

#[test]
fn read_fae_rejects_empty_input() {
    let directory = tempdir().expect("create temporary test directory");
    let input = directory.path().join("empty.fae");
    fs::write(&input, []).expect("write empty FAE fixture");

    let output = Command::new(cargo_bin("read_fae"))
        .arg(&input)
        .output()
        .expect("execute read_fae");
    let text = combined_output(&output);

    assert!(!output.status.success());
    assert!(text.contains("empty") || text.contains("too small"));
}
