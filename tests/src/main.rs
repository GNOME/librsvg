#[cfg(test)]
mod api;

#[cfg(test)]
mod bugs;

#[cfg(test)]
mod cmdline;

#[cfg(test)]
mod compare_surfaces;

#[cfg(test)]
mod errors;

#[cfg(test)]
mod filters;

#[cfg(test)]
mod geometries;

#[cfg(test)]
mod intrinsic_dimensions;

#[cfg(test)]
mod legacy_sizing;

#[cfg(test)]
mod loading_crash;

#[cfg(test)]
mod loading_disallowed;

#[cfg(test)]
mod predicates;

#[cfg(test)]
mod primitive_geometries;

#[cfg(test)]
mod primitives;

#[cfg(test)]
mod reference;

#[cfg(test)]
mod reference_utils;

#[cfg(test)]
mod render_crash;

#[cfg(test)]
mod shapes;

#[cfg(test)]
mod text;

#[cfg(test)]
mod utils;

fn main() {
    println!("Use 'cargo test' to run the tests.");
}
