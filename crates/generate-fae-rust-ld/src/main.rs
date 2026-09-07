fn main() {
    std::process::exit(
        match generate_fae_rust_ld::tool::run(&std::env::args().collect::<Vec<_>>()) {
            Ok(()) => 0,
            Err(code) => code,
        },
    );
}
