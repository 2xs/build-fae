use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const THUMB_TARGET: &str = "thumbv7em-none-eabi";
const ARM_TARGET: &str = "armv7a-none-eabi";

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(cmd) = args.next() else {
        print_usage();
        bail!("missing xtask command");
    };

    match cmd.as_str() {
        "rustrt0-arm" => build_rustrt0_arm(),
        "rustrt0-thumb" => build_rustrt0_thumb(),
        "xipfs-rustrt0-thumb" => {
            let profile = args.next().unwrap_or(String::from("release"));
            build_xipfs_rustrt0_thumb(&profile)
        }
        "embedded-check" => check_embedded_projects(),
        "fae-rustrt" => match args.next().as_deref() {
            None => build_fae_rustrt(false),
            Some("fae") => build_fae_rustrt(true),
            Some("check") => check_fae_rustrt_codegen(),
            Some(other) => {
                print_usage();
                bail!("unknown fae-rustrt subcommand: {}", other);
            }
        },
        _ => {
            print_usage();
            bail!("unknown xtask command: {}", cmd)
        }
    }
}

fn print_usage() {
    eprintln!("usage: cargo rustrt0-arm");
    eprintln!("usage: cargo rustrt0-thumb");
    eprintln!("usage: cargo xipfs-rustrt0-thumb");
    eprintln!("usage: cargo embedded-check");
    eprintln!("usage: cargo run -p xtask -- fae-rustrt");
    eprintln!("usage: cargo run -p xtask -- fae-rustrt fae");
    eprintln!("usage: cargo run -p xtask -- fae-rustrt check");
    eprintln!("   or: cargo run -p xtask -- rustrt0-arm");
    eprintln!("   or: cargo run -p xtask -- rustrt0-thumb");
}

fn workspace_root() -> Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must live under workspace root")?
        .to_path_buf())
}

fn run_checked(cmd: &mut Command, label: &str) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {}", label))?;
    if !status.success() {
        bail!("{} failed with status {}", label, status);
    }
    Ok(())
}

fn run_checked_output(cmd: &mut Command, label: &str) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {}", label))?;
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            label,
            output.status,
            stdout,
            stderr
        );
    }
    String::from_utf8(output.stdout).context("command output was not utf-8")
}

fn build_rustrt0(rt_name: &str, profile: &str, target: &str) -> Result<()> {
    let root = workspace_root()?;
    let build_dir = root.join("build");
    fs::create_dir_all(&build_dir).context("cannot create build directory")?;

    let linker_script = root.join(format!("rt0/{}/link.ld", rt_name));
    let rustflags = format!(
        "-C linker=arm-none-eabi-ld -C link-arg=-T{} -C opt-level=z -C panic=abort -C link-arg=--gc-sections",
        linker_script.display()
    );
    let mut profile_arg = String::from("--profile=");
    profile_arg.push_str(profile);

    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&root)
        .env("RUSTFLAGS", rustflags)
        .arg("rustc")
        .arg(&profile_arg)
        .arg("--target")
        .arg(target)
        .arg("--target-dir")
        .arg(root.join("target"))
        .arg("--manifest-path")
        .arg(root.join(format!("rt0/{}/Cargo.toml", rt_name)))
        .arg("--bin")
        .arg(rt_name);
    run_checked(&mut cargo, &format!("cargo rustc for {}", rt_name))?;

    let profile_dir = match profile {
        "dev" => "debug",
        other => other,
    };

    let input_elf = root.join(format!("target/{}/{}/{}", target, profile_dir, rt_name));
    let output_bin = build_dir.join(format!("{}.bin", rt_name));

    let mut objcopy = Command::new("arm-none-eabi-objcopy");
    objcopy
        .arg("--input-target=elf32-littlearm")
        .arg("--output-target=binary")
        .arg(&input_elf)
        .arg(&output_bin);
    run_checked(&mut objcopy, "arm-none-eabi-objcopy")?;

    println!("Generated {}", output_bin.display());
    Ok(())
}

fn build_rustrt0_thumb() -> Result<()> {
    build_rustrt0("rustrt0-thumb", "release", THUMB_TARGET)
}

fn build_rustrt0_arm() -> Result<()> {
    build_rustrt0("rustrt0-arm", "release", ARM_TARGET)
}

fn build_xipfs_rustrt0_thumb(profile: &str) -> Result<()> {
    build_rustrt0("xipfs-rustrt0-thumb", profile, THUMB_TARGET)
}

fn check_embedded_projects() -> Result<()> {
    let root = workspace_root()?;
    let projects = [
        ("crates/rust-xipfs-lib", THUMB_TARGET),
        ("rt0/rustrt0-arm", ARM_TARGET),
        ("rt0/rustrt0-thumb", THUMB_TARGET),
        ("rt0/xipfs-rustrt0-thumb", THUMB_TARGET),
        ("examples/minimal_rust", THUMB_TARGET),
        ("examples/minimal_rust_print", THUMB_TARGET),
        ("tests/minimal_rust_print_test", THUMB_TARGET),
        ("tests/relocated_cstring_repro_test", THUMB_TARGET),
        ("tests/relocated_slice_location_repro_test", THUMB_TARGET),
        ("tests/minimal_rust_print_CStrBuf_test", THUMB_TARGET),
    ];

    for (project, target) in projects {
        let mut cargo = Command::new("cargo");
        cargo
            .current_dir(&root)
            .arg("check")
            .arg("--manifest-path")
            .arg(root.join(project).join("Cargo.toml"))
            .arg("--target")
            .arg(target);
        run_checked(&mut cargo, &format!("cargo check for {}", project))?;
    }

    Ok(())
}

fn has_elf_magic(path: &Path) -> Result<bool> {
    let mut file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut magic = [0u8; 4];
    let read = file
        .read(&mut magic)
        .with_context(|| format!("failed to read {}", path.display()))?;
    Ok(read == magic.len() && magic == [0x7f, b'E', b'L', b'F'])
}

fn latest_matching_elf_object(dir: &Path, prefix: &str) -> Result<PathBuf> {
    let mut candidates = Vec::new();

    for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("cannot read entry under {}", dir.display()))?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(prefix) || path.extension().and_then(|ext| ext.to_str()) != Some("o") {
            continue;
        }
        if !has_elf_magic(&path)? {
            continue;
        }
        let modified = entry
            .metadata()
            .with_context(|| format!("cannot stat {}", path.display()))?
            .modified()
            .with_context(|| format!("cannot read mtime for {}", path.display()))?;
        candidates.push((modified, path));
    }

    candidates.sort_by_key(|(modified, _)| *modified);
    candidates
        .pop()
        .map(|(_, path)| path)
        .context("no matching ELF object file found")
}

fn ensure_contains(haystack: &str, needle: &str, context: &str) -> Result<()> {
    if !haystack.contains(needle) {
        bail!("{}: expected to find {:?}", context, needle);
    }
    Ok(())
}

fn build_fae_rustrt(package_fae: bool) -> Result<()> {
    let root = workspace_root()?;
    let starter_dir = root.join("sdk/fae-rustrt");
    let app_dir = starter_dir.join("examples/helloworld");
    let app_manifest = app_dir.join("Cargo.toml");
    let build_dir = starter_dir.join("build");
    fs::create_dir_all(&build_dir).context("cannot create starter build directory")?;

    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&root)
        .arg("run")
        .arg("--bin")
        .arg("build_fae_rust")
        .arg("--")
        .arg("--manifest-path")
        .arg(&app_manifest);
    if package_fae {
        cargo.arg("--fae");
    } else {
        cargo.arg("--elf");
    }
    run_checked(&mut cargo, "cargo run --bin build_fae_rust")?;

    let app_build_dir = app_dir.join("build");
    let input_elf = app_build_dir.join("fae_rustrt_helloworld.elf");
    let output_elf = build_dir.join("helloworld.elf");
    fs::copy(&input_elf, &output_elf).with_context(|| {
        format!(
            "failed to copy starter elf from {} to {}",
            input_elf.display(),
            output_elf.display()
        )
    })?;
    println!("Generated {}", output_elf.display());

    if package_fae {
        let input_fae = app_build_dir.join("fae_rustrt_helloworld.fae");
        let output_fae = build_dir.join("helloworld.fae");
        fs::copy(&input_fae, &output_fae).with_context(|| {
            format!(
                "failed to copy starter fae from {} to {}",
                input_fae.display(),
                output_fae.display()
            )
        })?;
        println!("Generated {}", output_fae.display());

        let input_gdbinit = app_build_dir.join("fae_rustrt_helloworld.gdbinit");
        let output_gdbinit = build_dir.join("helloworld.gdbinit");
        fs::copy(&input_gdbinit, &output_gdbinit).with_context(|| {
            format!(
                "failed to copy starter gdbinit from {} to {}",
                input_gdbinit.display(),
                output_gdbinit.display()
            )
        })?;
        println!("Generated {}", output_gdbinit.display());
    }

    Ok(())
}

fn check_fae_rustrt_codegen() -> Result<()> {
    let root = workspace_root()?;
    let starter_dir = root.join("sdk/fae-rustrt");
    let target_dir = starter_dir.join("target-codegen-check");
    fs::create_dir_all(&target_dir).context("cannot create codegen check target directory")?;

    let mut cargo = Command::new("cargo");
    cargo
        .current_dir(&starter_dir)
        .arg("+nightly-aarch64-apple-darwin")
        .arg("rustc")
        .arg("-Z")
        .arg("build-std=core,alloc,compiler_builtins")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem")
        .arg("--lib")
        .arg("--target")
        .arg(THUMB_TARGET)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--")
        .arg("-C")
        .arg("relocation-model=ropi-rwpi")
        .arg("--emit=obj");
    run_checked(&mut cargo, "cargo rustc fae-rustrt codegen check")?;

    let deps_dir = target_dir.join(format!("{}/debug/deps", THUMB_TARGET));
    let object = latest_matching_elf_object(&deps_dir, "fae_rustrt-")?;

    let attributes = run_checked_output(
        Command::new("arm-none-eabi-readelf").arg("-A").arg(&object),
        "arm-none-eabi-readelf -A",
    )?;
    ensure_contains(
        &attributes,
        "Tag_ABI_PCS_R9_use: SB",
        "codegen ABI attributes",
    )?;
    ensure_contains(
        &attributes,
        "Tag_ABI_PCS_RW_data: SB-relative",
        "codegen ABI attributes",
    )?;
    ensure_contains(
        &attributes,
        "Tag_ABI_PCS_RO_data: PC-relative",
        "codegen ABI attributes",
    )?;

    let relocations = run_checked_output(
        Command::new("arm-none-eabi-readelf").arg("-r").arg(&object),
        "arm-none-eabi-readelf -r",
    )?;
    ensure_contains(&relocations, "R_ARM_SBREL32", "codegen relocation model")?;

    println!("Validated Rust codegen model in {}", object.display());
    Ok(())
}
