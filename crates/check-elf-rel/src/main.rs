mod tool;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(code) = tool::run(&args) {
        std::process::exit(code);
    }
}
