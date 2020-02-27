#[cfg(test)]
#[macro_use]
extern crate float_cmp;

#[cfg(test)]
mod cmdline;

#[cfg(test)]
mod predicates;

fn main() {
    println!("Use 'cargo test' to run the tests.");
}
