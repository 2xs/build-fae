This directory is kept only as documentation anchor.

The startup ELF images used by `build_fae` are no longer versioned in the
repository. They are rebuilt automatically by the `fae-build` crate build
script into Cargo's `OUT_DIR` before `build_fae` is compiled.

If you modify the sources under `rt0/`, rebuilding `build_fae` regenerates the
embedded startup ELFs automatically.
