#[path = "../plugins.rs"]
#[allow(dead_code)]
mod plugins;

fn main() {
    let root = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    print!("{}", plugins::markdown(&root));
}
