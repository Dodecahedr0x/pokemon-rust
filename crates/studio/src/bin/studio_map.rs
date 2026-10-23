//! The headless map editor (SIL1 Task C); see `studio::map_edit::cli`.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match studio::map_edit::cli::run(&args) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
    }
}
