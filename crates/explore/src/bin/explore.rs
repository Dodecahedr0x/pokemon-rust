//! The explorer CLI, run by `cargo xtask explore …` (see `explore::cli`).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = explore::cli::main(args.clone());
    if let Err(e) = &r {
        eprintln!("Error: {e:#}");
    }
    std::process::exit(explore::cli::exit_code(&args, &r));
}
