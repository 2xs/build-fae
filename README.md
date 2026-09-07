# build-fae

> **Public release repository.** Development started on 10 March 2026 and
> continues in the 2XS GitLab at the University of Lille. This GitHub-facing
> repository contains curated release snapshots rather than the complete
> development history.


This repository provides the host-side tools, startup/runtime support, SDK
starters, examples, and documentation used to build, inspect, and validate FAE
executable images.

## Repository Map

- [`crates/README.md`](crates/README.md): host-side Rust tooling
  `build_fae`, `read_fae`, their helper CLIs, and the internal library crates
- [`rt0/README.md`](rt0/README.md): low-level startup runtimes and firmware startup profiles
- [`sdk/README.md`](sdk/README.md): starter code and reusable application templates
- [`examples/README.md`](examples/README.md): small focused examples around the FAE contract
- [`docs/README.md`](docs/README.md): documentation index

## Suggested Reading

- [`docs/getting-started.md`](docs/getting-started.md): first end-to-end run
- [`docs/user-manual.md`](docs/user-manual.md): practical usage constraints
- [`docs/build_fae.md`](docs/build_fae.md): `build_fae` contract
- [`docs/fae-format.md`](docs/fae-format.md): binary file format

## License

This project is distributed under the GNU General Public License v3.0 only. See [LICENSE](LICENSE).
See [AUTHORS](AUTHORS) for the project author list.
