use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let embedded_dir = out_dir.join("embedded-startups");

    emit_rerun_if_changed(&repo_root.join("rt0/arm-thumb/core.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-thumb/core-v6m.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-thumb/link.ld"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-thumb/rt0-thumb.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-thumb/rt0-thumbv6m.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-arm/core.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-arm/link.ld"));
    emit_rerun_if_changed(&repo_root.join("rt0/arm-arm/rt0-arm.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/generic-cortex-m/boot_rt0.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/mps2-an385/boot_rt0.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/mps2-an385/link.ld"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/mps2-an386/boot_rt0.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/mps2-an386/link.ld"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/olimex-stm32-h405/boot_rt0.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/olimex-stm32-h405/link.ld"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/b-l475e-iot01a/boot_rt0.s"));
    emit_rerun_if_changed(&repo_root.join("rt0/bootable/b-l475e-iot01a/link.ld"));

    std::fs::create_dir_all(&embedded_dir).expect("create embedded startup output directory");

    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumb",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m4", "-mthumb"],
        "rt0/arm-thumb/core.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumb-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m4",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/arm-thumb/core.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumb-rustlet",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m4",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
            "-Wa,--defsym,OXIDE_SE_ABI=1",
        ],
        "rt0/arm-thumb/core.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumbv6m",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m0plus", "-mthumb"],
        "rt0/arm-thumb/core-v6m.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumbv6m-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m0plus",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/arm-thumb/core-v6m.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-thumbv6m-rustlet",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m0plus",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
            "-Wa,--defsym,OXIDE_SE_ABI=1",
        ],
        "rt0/arm-thumb/core-v6m.s",
        "rt0/arm-thumb/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-arm",
        "arm-none-eabi-gcc",
        &["-c", "-march=armv7-a"],
        "rt0/arm-arm/core.s",
        "rt0/arm-arm/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-arm-fae1",
        "arm-none-eabi-gcc",
        &["-c", "-march=armv7-a", "-Wa,--defsym,FAE1_FOOTER=1"],
        "rt0/arm-arm/core.s",
        "rt0/arm-arm/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "rt0-arm-rustlet",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-march=armv7-a",
            "-Wa,--defsym,FAE1_FOOTER=1",
            "-Wa,--defsym,OXIDE_SE_ABI=1",
        ],
        "rt0/arm-arm/core.s",
        "rt0/arm-arm/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-mps2-an385",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m3", "-mthumb"],
        "rt0/bootable/mps2-an385/boot_rt0.s",
        "rt0/bootable/mps2-an385/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-mps2-an385-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m3",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/bootable/mps2-an385/boot_rt0.s",
        "rt0/bootable/mps2-an385/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-mps2-an386",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m4", "-mthumb"],
        "rt0/bootable/mps2-an386/boot_rt0.s",
        "rt0/bootable/mps2-an386/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-mps2-an386-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m4",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/bootable/mps2-an386/boot_rt0.s",
        "rt0/bootable/mps2-an386/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-olimex-stm32-h405",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m4", "-mthumb"],
        "rt0/bootable/olimex-stm32-h405/boot_rt0.s",
        "rt0/bootable/olimex-stm32-h405/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-olimex-stm32-h405-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m4",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/bootable/olimex-stm32-h405/boot_rt0.s",
        "rt0/bootable/olimex-stm32-h405/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-b-l475e-iot01a",
        "arm-none-eabi-gcc",
        &["-c", "-mcpu=cortex-m4", "-mthumb"],
        "rt0/bootable/b-l475e-iot01a/boot_rt0.s",
        "rt0/bootable/b-l475e-iot01a/link.ld",
    );
    build_startup(
        &repo_root,
        &embedded_dir,
        "boot-b-l475e-iot01a-fae1",
        "arm-none-eabi-gcc",
        &[
            "-c",
            "-mcpu=cortex-m4",
            "-mthumb",
            "-Wa,--defsym,FAE1_FOOTER=1",
        ],
        "rt0/bootable/b-l475e-iot01a/boot_rt0.s",
        "rt0/bootable/b-l475e-iot01a/link.ld",
    );
}

fn emit_rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn build_startup(
    repo_root: &Path,
    embedded_dir: &Path,
    stem: &str,
    compiler: &str,
    compiler_flags: &[&str],
    source_rel: &str,
    linker_script_rel: &str,
) {
    let source = repo_root.join(source_rel);
    let linker_script = repo_root.join(linker_script_rel);
    let object = embedded_dir.join(format!("{stem}.o"));
    let elf = embedded_dir.join(format!("{stem}.elf"));

    let mut compile = Command::new(compiler);
    compile.args(compiler_flags);
    compile.arg("-I").arg(repo_root);
    compile.arg(&source);
    compile.arg("-o").arg(&object);
    run_command(compile, &format!("compile {}", source.display()));

    let mut link = Command::new("arm-none-eabi-ld");
    link.arg("-T").arg(&linker_script);
    link.arg(&object);
    link.arg("-o").arg(&elf);
    run_command(link, &format!("link {}", elf.display()));
}

fn run_command(mut command: Command, context: &str) {
    let output = command.output().unwrap_or_else(|error| {
        panic!("{context} failed: {error}");
    });
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "{context} failed with status {}.\nstdout:\n{}\nstderr:\n{}",
            output.status, stdout, stderr
        );
    }
}
