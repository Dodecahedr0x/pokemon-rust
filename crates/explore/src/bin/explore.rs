//! The explorer CLI, run by `cargo xtask explore …` (see `explore::cli`).

fn main() -> anyhow::Result<()> {
    explore::cli::main(std::env::args().skip(1).collect())
}
