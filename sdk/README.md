# SDK Layer

`sdk/` contains the developer-facing starting points that sit above the low-level
runtime and below application-specific projects.

This layer is distinct from:

- [`../crates/`](../crates), which contains the end-user CLI tools and their internal libraries
- [`../rt0/`](../rt0), which contains the low-level startup runtimes and firmware startup profiles

## Contents

- [`minimal-c/`](minimal-c): minimal C starter validated on QEMU
- [`fae-rustrt/`](fae-rustrt): reusable Rust runtime support layer under active development
- [`faeulibc/`](faeulibc): more complete C starter experiments around picolibc
- [`templates/`](templates): reusable application templates built on top of the SDK/runtime split

## Intent

The SDK layer is where application authors should start when they want a working
payload shape or a reusable project skeleton.

The low-level FAE startup logic still lives in `rt0/`, and the actual tooling
still lives in `crates/`, but the code under `sdk/` is the place where those
two layers become directly consumable by applications.
