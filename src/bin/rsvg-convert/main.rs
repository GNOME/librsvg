#[macro_use]
extern crate clap;

mod cli;

fn main() {
    let args = cli::Args::new().unwrap_or_else(|e| e.exit());

    println!("{:?}", args);
}
