use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};

use fae_core::constants;
use fae_elf::elf::{export_symbol_value, parse_elf, rewrite_exec_runtime_references_to_sbrel};
use fae_elf::rust_input_section_plan::collect_rust_input_section_plan;
use generate_fae_rust_ld::tool::render_link_script_from_section_plan;

const DEFAULT_TARGET: &str = "thumbv7em-none-eabi";
const ENV_MODE: &str = "FAE_RUST_APP_TOOL_MODE";
const ENV_MODE_LINK_WRAPPER: &str = "link-wrapper";
const ENV_REAL_LINKER: &str = "FAE_RUST_APP_TOOL_REAL_LINKER";
const ENV_SECTION_PLAN: &str = "FAE_RUST_APP_TOOL_SECTION_PLAN";

#[derive(Debug, Clone)]
enum PackagingMode {
    ElfOnly,
    Fae,
    Bootable { board: String },
}

#[derive(Debug, Clone)]
struct Options {
    app_manifest: PathBuf,
    bin_name: Option<String>,
    target: String,
    align_payload: usize,
    align_size: usize,
    facade12: bool,
    profile_kind: Option<RustletProfileKind>,
    isa_descriptor: Option<u32>,
    isa_words: Vec<u32>,
    abi_descriptor: Option<u32>,
    abi_words: Vec<u32>,
    rt0: Option<String>,
    allow_rt0_mismatch: bool,
    stack_size: u32,
    packaging: PackagingMode,
    profile: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RustletProfileKind {
    Application,
    SecurityDomain,
}

fn main() -> Result<()> {
    if env::var(ENV_MODE).ok().as_deref() == Some(ENV_MODE_LINK_WRAPPER) {
        run_link_wrapper();
    }

    let options = parse_args()?;
    run(options)
}

fn run_link_wrapper() -> ! {
    let result = (|| -> Result<i32> {
        let real_linker =
            env::var(ENV_REAL_LINKER).context("missing real linker path for link wrapper")?;
        let args = env::args().skip(1).collect::<Vec<_>>();

        if let Ok(path) = env::var(ENV_SECTION_PLAN) {
            let inputs = collect_linker_input_files(&args);
            let plan = collect_rust_input_section_plan(&inputs)?;
            merge_section_plan(Path::new(&path), &plan)?;
        }

        let status = Command::new(&real_linker)
            .args(&args)
            .status()
            .with_context(|| format!("failed to run real linker {}", real_linker))?;
        Ok(status.code().unwrap_or(1))
    })();

    match result {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("build_fae_rust link wrapper: {err}");
            std::process::exit(1);
        }
    }
}

fn collect_linker_input_files(args: &[String]) -> Vec<PathBuf> {
    let mut inputs = Vec::new();
    let mut skip_next = false;

    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }

        if matches!(
            arg.as_str(),
            "-o" | "-L"
                | "-T"
                | "-e"
                | "-m"
                | "--entry"
                | "--library-path"
                | "--script"
                | "--sysroot"
                | "--Map"
        ) {
            skip_next = true;
            continue;
        }

        if arg.starts_with('-') {
            continue;
        }

        let path = PathBuf::from(arg);
        if !path.is_file() {
            continue;
        }

        if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("o") | Some("rlib") | Some("a")
        ) && !inputs.iter().any(|existing| existing == &path)
        {
            inputs.push(path);
        }
    }

    inputs
}

fn merge_section_plan(
    path: &Path,
    new_plan: &fae_elf::rust_input_section_plan::RustInputSectionPlan,
) -> Result<()> {
    with_section_plan_lock(path, || {
        let mut merged = if path.exists() {
            fae_elf::rust_input_section_plan::RustInputSectionPlan::read_from_path(path)?
        } else {
            fae_elf::rust_input_section_plan::RustInputSectionPlan::default()
        };
        merged.merge(new_plan);
        merged.write_atomically_to_path(path)
    })
}

fn with_section_plan_lock<T>(path: &Path, action: impl FnOnce() -> Result<T>) -> Result<T> {
    let lock_dir = section_plan_lock_dir(path)?;
    loop {
        match fs::create_dir(&lock_dir) {
            Ok(()) => break,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("cannot create section-plan lock {}", lock_dir.display())
                });
            }
        }
    }

    let result = action();
    let _ = fs::remove_dir(&lock_dir);
    result
}

fn section_plan_lock_dir(path: &Path) -> Result<PathBuf> {
    let parent = path
        .parent()
        .context("section plan path has no parent directory")?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("section plan path has no valid UTF-8 file name")?;
    Ok(parent.join(format!("{file_name}.lock")))
}

fn copy_if_changed(from: &Path, to: &Path) -> Result<bool> {
    if to.exists() && fs::read(from)? == fs::read(to)? {
        return Ok(false);
    }
    fs::copy(from, to).with_context(|| {
        format!(
            "failed to copy file from {} to {}",
            from.display(),
            to.display()
        )
    })?;
    Ok(true)
}

fn parse_args() -> Result<Options> {
    let mut args = env::args().skip(1);
    let mut app_manifest = None;
    let mut bin_name = None;
    let mut target = DEFAULT_TARGET.to_owned();
    let mut align_payload = 0usize;
    let mut align_size = constants::PADDING_MPU_ALIGNMENT;
    let mut facade12 = false;
    let mut profile_kind = None;
    let mut isa_descriptor = None;
    let mut isa_words = Vec::new();
    let mut abi_descriptor = None;
    let mut abi_words = Vec::new();
    let mut rt0 = None;
    let mut allow_rt0_mismatch = false;
    let mut stack_size = 2048u32;
    let mut packaging_option = None;
    let mut profile = constants::DEFAULT_PROFILE.to_owned();
    let mut trailing = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest-path" => {
                let value = args.next().context("missing value after --manifest-path")?;
                app_manifest = Some(PathBuf::from(value));
            }
            "--bin" => {
                let value = args.next().context("missing value after --bin")?;
                bin_name = Some(value);
            }
            "--target" => {
                let value = args.next().context("missing value after --target")?;
                target = value;
            }
            "--align_payload" => {
                let value = args.next().context("missing value after --align_payload")?;
                align_payload = parse_alignment("--align_payload", &value)?;
            }
            "--align_size" => {
                let value = args.next().context("missing value after --align_size")?;
                align_size = parse_alignment("--align_size", &value)?;
            }
            "--facade12" => {
                facade12 = true;
            }
            "-rs" | "--rustlet" => {
                if profile_kind.is_some() || abi_descriptor.is_some() {
                    bail!("--rustlet cannot be combined with another ABI profile");
                }
                profile_kind = Some(RustletProfileKind::Application);
            }
            "-sd" | "--securitydomain" => {
                if profile_kind.is_some() || abi_descriptor.is_some() {
                    bail!("--securitydomain cannot be combined with another ABI profile");
                }
                profile_kind = Some(RustletProfileKind::SecurityDomain);
            }
            "--isa" => {
                isa_descriptor = Some(parse_u32_option(
                    "--isa",
                    &args.next().context("missing value after --isa")?,
                )?);
            }
            "--isa-word" => {
                isa_words.push(parse_u32_option(
                    "--isa-word",
                    &args.next().context("missing value after --isa-word")?,
                )?);
            }
            "--abi" => {
                if profile_kind.is_some() {
                    bail!("--abi cannot be combined with a Rustlet profile alias");
                }
                abi_descriptor = Some(parse_u32_option(
                    "--abi",
                    &args.next().context("missing value after --abi")?,
                )?);
            }
            "--abi-word" => {
                abi_words.push(parse_u32_option(
                    "--abi-word",
                    &args.next().context("missing value after --abi-word")?,
                )?);
            }
            "-rt0" | "--rt0" => {
                let value = args.next().context("missing value after --rt0")?;
                if !matches!(value.as_str(), "thumb" | "thumbv6m" | "arm")
                    && !Path::new(&value).is_file()
                {
                    bail!("--rt0 expects thumb, thumbv6m, arm, or an existing RT0 ELF file");
                }
                rt0 = Some(value);
            }
            "--allow-rt0-mismatch" => {
                allow_rt0_mismatch = true;
            }
            "--stack-size" => {
                stack_size = args
                    .next()
                    .context("missing value after --stack-size")?
                    .parse()
                    .context("--stack-size expects an integer")?;
            }
            "--elf" => {
                set_packaging_option(&mut packaging_option, PackagingMode::ElfOnly, "--elf")?;
            }
            "--fae" => {
                set_packaging_option(&mut packaging_option, PackagingMode::Fae, "--fae")?;
            }
            "--profile" => {
                let value = args.next().context("missing value after --profile")?;
                profile = value;
            }
            "-h" | "--help" => {
                usage();
                std::process::exit(0);
            }
            other => trailing.push(other.to_owned()),
        }
    }

    let trailing_packaging = match trailing.as_slice() {
        [] => None,
        [cmd, board] if cmd == "bootable" => Some(PackagingMode::Bootable {
            board: board.clone(),
        }),
        _ => {
            usage();
            bail!("expected --elf, --fae, or trailing 'bootable <board>'");
        }
    };
    let packaging = if packaging_option.is_none() && trailing_packaging.is_none() {
        usage();
        bail!("missing packaging option: pass --elf or --fae");
    } else if packaging_option.is_some() && trailing_packaging.is_some() {
        usage();
        bail!("use either --elf/--fae or a trailing packaging command, not both");
    } else {
        packaging_option
            .or(trailing_packaging)
            .expect("packaging presence was checked above")
    };

    let app_manifest = app_manifest.context("missing required --manifest-path <Cargo.toml>")?;
    if !isa_words.is_empty() && isa_descriptor.is_none() {
        bail!("--isa-word requires --isa");
    }
    if !abi_words.is_empty() && abi_descriptor.is_none() {
        bail!("--abi-word requires --abi");
    }
    validate_descriptor_word_count("ISA", isa_descriptor, &isa_words)?;
    validate_descriptor_word_count("ABI", abi_descriptor, &abi_words)?;
    if facade12 && (profile_kind.is_some() || isa_descriptor.is_some() || abi_descriptor.is_some())
    {
        bail!("--facade12 cannot be combined with FAE 1.0 ISA/ABI options");
    }
    Ok(Options {
        app_manifest,
        bin_name,
        target,
        align_payload,
        align_size,
        facade12,
        profile_kind,
        isa_descriptor,
        isa_words,
        abi_descriptor,
        abi_words,
        rt0,
        allow_rt0_mismatch,
        stack_size,
        packaging,
        profile,
    })
}

fn set_packaging_option(
    packaging: &mut Option<PackagingMode>,
    value: PackagingMode,
    option: &str,
) -> Result<()> {
    if packaging.is_some() {
        bail!("only one packaging option may be passed; duplicate {option}");
    }
    *packaging = Some(value);
    Ok(())
}

fn parse_alignment(option: &str, value: &str) -> Result<usize> {
    let align = value
        .trim()
        .parse::<usize>()
        .with_context(|| format!("{option} expects an integer"))?;
    if align > 1 && !align.is_power_of_two() {
        bail!("{option} expects 0, 1, or a power of two");
    }
    Ok(align)
}

fn parse_u32_option(option: &str, value: &str) -> Result<u32> {
    let compact = value.trim().replace('_', "");
    compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
        .map(|hex| u32::from_str_radix(hex, 16))
        .unwrap_or_else(|| compact.parse::<u32>())
        .with_context(|| format!("{option} expects a 32-bit decimal or 0x-prefixed value"))
}

fn validate_descriptor_word_count(
    kind: &str,
    descriptor: Option<u32>,
    words: &[u32],
) -> Result<()> {
    if let Some(descriptor) = descriptor {
        let expected = (descriptor & 0x0F) as usize;
        if expected != words.len() {
            bail!(
                "{kind} descriptor declares {expected} additional word(s), but {} were supplied",
                words.len()
            );
        }
    }
    Ok(())
}

fn usage() {
    eprintln!("Usage:");
    eprintln!("  build_fae_rust --manifest-path <app/Cargo.toml> --elf");
    eprintln!("  build_fae_rust --manifest-path <app/Cargo.toml> --fae");
    eprintln!("  build_fae_rust --manifest-path <app/Cargo.toml> bootable <mps2-an385|mps2-an386>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --bin <name>     Override the produced executable name");
    eprintln!("  --target <triple>  Default: {DEFAULT_TARGET}");
    eprintln!("  --align_payload <bytes>  Default: 0");
    eprintln!(
        "  --align_size <bytes>  Default: {}",
        constants::PADDING_MPU_ALIGNMENT
    );
    eprintln!("  --facade12          Emit the legacy 0xFACADE12 footer");
    eprintln!("  --isa <u32>          Explicit FAE 1.0 ISA descriptor");
    eprintln!("  --isa-word <u32>     Append one ISA descriptor word");
    eprintln!("  --abi <u32>          Explicit FAE 1.0 ABI descriptor");
    eprintln!("  --abi-word <u32>     Append one ABI descriptor word");
    eprintln!("  -rs, --rustlet       Rustlet application profile alias");
    eprintln!("  -sd, --securitydomain  Rustlet Security Domain profile alias");
    eprintln!("  -rt0, --rt0 <kind|elf>  thumb, thumbv6m, arm, or an explicit RT0 ELF");
    eprintln!("  --allow-rt0-mismatch Downgrade RT0/payload mismatch to a warning");
    eprintln!("  --stack-size <bytes> Rustlet stack requirement; default: 2048");
}

fn repo_root() -> Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("tool must live under repo_root/tools")?
        .parent()
        .context("tool must live under repo_root/tools")?
        .to_path_buf())
}

fn run_checked(cmd: &mut Command, label: &str) -> Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {label}"))?;
    if !status.success() {
        bail!("{label} failed with status {status}");
    }
    Ok(())
}

fn run_checked_output(cmd: &mut Command, label: &str) -> Result<String> {
    let output = cmd
        .output()
        .with_context(|| format!("failed to run {label}"))?;
    if !output.status.success() {
        bail!(
            "{label} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
    String::from_utf8(output.stdout).context("command output was not utf-8")
}

fn remove_stale_generated_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("cannot remove {}", path.display())),
    }
}

fn host_target() -> Result<String> {
    let stdout = run_checked_output(Command::new("rustc").arg("-vV"), "rustc -vV")?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("rustc -vV did not report a host target")
}

fn preferred_nightly_toolchain() -> Result<String> {
    let host = host_target()?;
    let preferred = format!("nightly-{host}");
    let list = run_checked_output(
        Command::new("rustup").arg("toolchain").arg("list"),
        "rustup toolchain list",
    )?;
    if list
        .lines()
        .any(|line| line.split_whitespace().next() == Some(preferred.as_str()))
    {
        return Ok(preferred);
    }
    if list
        .lines()
        .any(|line| line.split_whitespace().next() == Some("nightly"))
    {
        return Ok("nightly".to_owned());
    }
    bail!("no nightly Rust toolchain is installed")
}

fn rust_lld_path(toolchain: &str) -> Result<String> {
    let host = host_target()?;
    let sysroot = run_checked_output(
        Command::new("rustc")
            .arg(format!("+{toolchain}"))
            .arg("--print")
            .arg("sysroot"),
        "rustc --print sysroot",
    )?;
    let path = Path::new(sysroot.trim())
        .join("lib")
        .join("rustlib")
        .join(host)
        .join("bin")
        .join("rust-lld");
    Ok(path.display().to_string())
}

fn parse_package_name(manifest_path: &Path) -> Result<String> {
    let manifest = fs::read_to_string(manifest_path)
        .with_context(|| format!("cannot read {}", manifest_path.display()))?;

    let mut in_package = false;
    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some(rest) = line.strip_prefix("name") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let value = rest.trim().trim_matches('"');
                if !value.is_empty() {
                    return Ok(value.to_owned());
                }
            }
        }
    }

    bail!(
        "failed to find [package].name in {}",
        manifest_path.display()
    )
}

fn build_rustflags(linker_script: Option<&Path>, extra_cfg: Option<&str>) -> Vec<String> {
    let mut flags = vec![
        "-Z".to_owned(),
        "function-sections=yes".to_owned(),
        "-C".to_owned(),
        "panic=abort".to_owned(),
        "-C".to_owned(),
        "relocation-model=ropi-rwpi".to_owned(),
        "-C".to_owned(),
        "link-arg=--emit-relocs".to_owned(),
        "-C".to_owned(),
        "link-arg=--gc-sections".to_owned(),
        "-C".to_owned(),
        "link-arg=--no-undefined".to_owned(),
    ];

    if let Some(linker_script) = linker_script {
        flags.push("-Z".to_owned());
        flags.push(format!("pre-link-arg=-T{}", linker_script.display()));
    }

    if let Some(extra_cfg) = extra_cfg {
        flags.push("--cfg".to_owned());
        flags.push(extra_cfg.to_owned());
    }

    flags
}

fn toml_array(items: &[String]) -> String {
    let mut out = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index != 0 {
            out.push(',');
        }
        out.push('"');
        for ch in item.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                _ => out.push(ch),
            }
        }
        out.push('"');
    }
    out.push(']');
    out
}

#[allow(clippy::too_many_arguments)]
fn cargo_build_app(
    toolchain: &str,
    app_manifest: &Path,
    target: &str,
    target_dir: &Path,
    linker_program: &Path,
    linker_env: &[(String, String)],
    linker_script: Option<&Path>,
    extra_cfg: Option<&str>,
    profile: &str,
) -> Result<()> {
    let rustflags = toml_array(&build_rustflags(linker_script, extra_cfg));

    let mut cargo = Command::new("cargo");
    cargo
        .arg(format!("+{toolchain}"))
        .arg("build")
        .arg("-Z")
        .arg("build-std=core,alloc,compiler_builtins")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem")
        .arg("--manifest-path")
        .arg(app_manifest)
        .arg(format!("--profile={profile}"))
        .arg("--target")
        .arg(target)
        .arg("--target-dir")
        .arg(target_dir)
        .arg("--config")
        .arg(format!(
            "target.{target}.linker=\"{}\"",
            linker_program.display()
        ))
        .arg("--config")
        .arg(format!("target.{target}.rustflags={rustflags}"));

    for (key, value) in linker_env {
        cargo.env(key, value);
    }

    run_checked(&mut cargo, "cargo build app")
}

fn run(options: Options) -> Result<()> {
    let repo_root = repo_root()?;
    let toolchain = preferred_nightly_toolchain()?;
    let rust_lld = rust_lld_path(&toolchain)?;
    let wrapper_path =
        env::current_exe().context("cannot resolve current build_fae_rust executable")?;
    let app_manifest = fs::canonicalize(&options.app_manifest).with_context(|| {
        format!(
            "cannot canonicalize manifest path {}",
            options.app_manifest.display()
        )
    })?;
    let app_dir = app_manifest
        .parent()
        .context("application manifest has no parent directory")?;
    let build_dir = std::env::var_os("BUILD_FAE_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| app_dir.join("build"));
    fs::create_dir_all(&build_dir)
        .with_context(|| format!("cannot create {}", build_dir.display()))?;

    let bin_name = match &options.bin_name {
        Some(name) => name.clone(),
        None => parse_package_name(&app_manifest)?,
    };

    let target_root = std::env::var_os("BUILD_FAE_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| app_dir.join("target"));
    let discovery_target = target_root.join("fae-discovery");
    let final_target = target_root.join("fae-final");
    let profile_dir = match options.profile.as_str() {
        "dev" => "debug",
        other => other,
    };
    let discovery_elf = discovery_target
        .join(&options.target)
        .join(profile_dir)
        .join(&bin_name);
    let final_elf = final_target
        .join(&options.target)
        .join(profile_dir)
        .join(&bin_name);
    let generated_linker = build_dir.join("auto-fae-rust.ld");
    let generated_section_plan = build_dir.join("auto-fae-input-sections.txt");
    let output_elf = build_dir.join(format!("{bin_name}.elf"));

    remove_stale_generated_file(&generated_section_plan)?;
    remove_stale_generated_file(&generated_linker)?;
    let discovery_cfg = format!(
        "fae_rust_section_plan_refresh_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_nanos()
    );
    let final_link_cfg = format!("{discovery_cfg}_final_link");

    let discovery_linker_env = vec![
        (ENV_MODE.to_owned(), ENV_MODE_LINK_WRAPPER.to_owned()),
        (ENV_REAL_LINKER.to_owned(), rust_lld.clone()),
        (
            ENV_SECTION_PLAN.to_owned(),
            generated_section_plan.display().to_string(),
        ),
    ];

    cargo_build_app(
        &toolchain,
        &app_manifest,
        &options.target,
        &discovery_target,
        &wrapper_path,
        &discovery_linker_env,
        None,
        Some(&discovery_cfg),
        &options.profile,
    )?;

    let section_plan = fae_elf::rust_input_section_plan::RustInputSectionPlan::read_from_path(
        &generated_section_plan,
    )
    .with_context(|| {
        format!(
            "failed to read discovery section plan from {}",
            generated_section_plan.display()
        )
    })?;
    let linker_script = render_link_script_from_section_plan(section_plan);
    fs::write(&generated_linker, linker_script)
        .with_context(|| format!("cannot write {}", generated_linker.display()))?;

    cargo_build_app(
        &toolchain,
        &app_manifest,
        &options.target,
        &final_target,
        Path::new(&rust_lld),
        &[],
        Some(&generated_linker),
        Some(&final_link_cfg),
        &options.profile,
    )?;

    {
        let mut final_bytes = fs::read(&final_elf)
            .with_context(|| format!("failed to read final ELF {}", final_elf.display()))?;
        let elf = parse_elf(&final_bytes)?;
        let rom_size = export_symbol_value(&elf, constants::EXPORTED_SYMBOL_ROM_SIZE)?;
        let patched = rewrite_exec_runtime_references_to_sbrel(&mut final_bytes, rom_size)?;
        if patched > 0 {
            fs::write(&final_elf, &final_bytes)
                .with_context(|| format!("failed to rewrite final ELF {}", final_elf.display()))?;
            println!(
                "Patched {} executable RAM-data reference pair(s) to SB-relative form",
                patched
            );
        }
    }

    if copy_if_changed(&final_elf, &output_elf)? {
        println!("Generated {}", output_elf.display());
    } else {
        println!("Unchanged {}", output_elf.display());
    }
    println!("Generated {}", discovery_elf.display());

    match &options.packaging {
        PackagingMode::ElfOnly => {}
        PackagingMode::Fae => {
            let startup = runtime_startup_for_target(&options.target);
            let mut command =
                runtime_build_fae_command(&repo_root, &output_elf, &options, startup)?;
            run_checked(&mut command, "cargo run --bin build_fae")?;
        }
        PackagingMode::Bootable { board } => {
            let mut command = Command::new("cargo");
            command
                .arg("run")
                .arg("--target")
                .arg(host_target()?)
                .arg("--manifest-path")
                .arg(repo_root.join("Cargo.toml"))
                .arg("--bin")
                .arg("build_fae")
                .arg("--")
                .arg("--firmware")
                .arg(board)
                .arg("--align_payload")
                .arg(options.align_payload.to_string())
                .arg("--align_size")
                .arg(options.align_size.to_string());
            if options.facade12 {
                command.arg("--facade12");
            }
            command.arg(&output_elf);
            run_checked(&mut command, "cargo run --bin build_fae --firmware")?;
        }
    }

    Ok(())
}

fn runtime_build_fae_command(
    repo_root: &Path,
    output_elf: &Path,
    options: &Options,
    startup: Option<&str>,
) -> Result<Command> {
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--target")
        .arg(host_target()?)
        .arg("--manifest-path")
        .arg(repo_root.join("Cargo.toml"))
        .arg("--bin")
        .arg("build_fae")
        .arg("--");
    if let Some(rt0) = options.rt0.as_deref().or(startup) {
        command.arg("--rt0").arg(rt0);
    }
    if options.allow_rt0_mismatch {
        command.arg("--allow-rt0-mismatch");
    }
    if options.facade12 {
        command.arg("--facade12");
    }
    match options.profile_kind {
        Some(RustletProfileKind::Application) => {
            command.arg("--rustlet");
        }
        Some(RustletProfileKind::SecurityDomain) => {
            command.arg("--securitydomain");
        }
        None => {}
    }
    if let Some(descriptor) = options.isa_descriptor {
        command.arg("--isa").arg(format!("0x{descriptor:08X}"));
        for word in &options.isa_words {
            command.arg("--isa-word").arg(format!("0x{word:08X}"));
        }
    }
    if let Some(descriptor) = options.abi_descriptor {
        command.arg("--abi").arg(format!("0x{descriptor:08X}"));
        for word in &options.abi_words {
            command.arg("--abi-word").arg(format!("0x{word:08X}"));
        }
    }
    command
        .arg("--stack-size")
        .arg(options.stack_size.to_string());
    command
        .arg("--align_payload")
        .arg(options.align_payload.to_string())
        .arg("--align_size")
        .arg(options.align_size.to_string())
        .arg(output_elf);
    Ok(command)
}

fn runtime_startup_for_target(target: &str) -> Option<&'static str> {
    if target.starts_with("thumbv6m") {
        Some("thumbv6m")
    } else {
        None
    }
}
