#[cfg(test)]
#[macro_use]
extern crate float_cmp;

#[cfg(test)]
mod cmdline;

#[cfg(test)]
mod loading_crash;

#[cfg(test)]
mod predicates;

mod utils;

fn main() {
    println!("Use 'cargo test' to run the tests.");
}
