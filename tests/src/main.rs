#[cfg(test)]
#[macro_use]
extern crate float_cmp;

#[cfg(test)]
mod cmdline;

#[cfg(test)]
mod errors;

#[cfg(test)]
mod loading_crash;

#[cfg(test)]
mod predicates;

#[cfg(test)]
mod reference;

#[cfg(test)]
mod render_crash;

#[cfg(test)]
mod utils;

fn main() {
    println!("Use 'cargo test' to run the tests.");
}
