use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mode = match args.next().as_deref() {
        None | Some("elf") => Mode::Elf,
        Some("fae") => Mode::Fae,
        Some("bootable") => {
            let board = args.next().ok_or("missing board after bootable")?;
            Mode::Bootable(board)
        }
        Some(other) => return Err(format!("unknown xtask command: {other}").into()),
    };

    let repo_root = repo_root()?;
    let app_manifest = repo_root.join("examples/rust-fae-helloworld/Cargo.toml");
    let mut cargo = Command::new("cargo");
    cargo
        .arg("run")
        .arg("--bin")
        .arg("build_fae_rust")
        .arg("--")
        .arg("--manifest-path")
        .arg(app_manifest);

    match mode {
        Mode::Elf => {
            cargo.arg("--elf");
        }
        Mode::Fae => {
            cargo.arg("--fae");
        }
        Mode::Bootable(board) => {
            cargo.arg("bootable").arg(board);
        }
    }

    let status = cargo.status()?;
    if !status.success() {
        return Err(format!("app build tool failed with status {status}").into());
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum Mode {
    Elf,
    Fae,
    Bootable(String),
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent")?
        .parent()
        .ok_or("example has no parent")?
        .parent()
        .ok_or("repo root not found")?
        .to_path_buf())
}
