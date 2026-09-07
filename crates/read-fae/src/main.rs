fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(code) = fae_read::read_tool::run(&args) {
        std::process::exit(code);
    }
}
