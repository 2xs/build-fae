use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn cargo_bin(name: &str) -> PathBuf {
    static HOST_BINARIES: OnceLock<()> = OnceLock::new();
    HOST_BINARIES.get_or_init(|| {
        let status = Command::new("cargo")
            .args(["build", "--workspace", "--bins"])
            .current_dir(workspace_root())
            .status()
            .expect("failed to build host-side binaries");
        assert!(status.success(), "failed to build host-side binaries");
    });

    workspace_root()
        .join("target")
        .join("debug")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}
