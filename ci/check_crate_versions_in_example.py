# Checks that the example Cargo.toml snippet from rsvg/src/lib.rs has the same versions for
# dependencies that librsvg uses during compilation.

import sys
import toml

# Looks for a crate version in the 'dependencies' section of a TOML document, either of these:
#
# [dependencies]
# foo = "1.2.3"
# bar = { version = "4.5.6", features=["something", "else", "here"]
def get_crate_version(toml_doc, crate_name):
    if 'dependencies' in toml_doc:
        crate_decl = toml_doc['dependencies'][crate_name]
    else:
        crate_decl = toml_doc['workspace']['dependencies'][crate_name]

    if isinstance(crate_decl, str):
        version = crate_decl
    else:
        version = crate_decl['version']

    return version

# Given a Rust file that has a toplevel comment somewhere like
#
#   //! ```toml
#   //! [dependencies]
#   //! librsvg = "2.57.0-beta.2"
#   //! cairo-rs = "0.18"
#   //! gio = "0.18"   # only if you need streams
#   //! ```
#
# extracts just the TOML as a string, without the //! prefix.
def find_toml_in_rust_toplevel_docs(lines):
    found_start = False
    start_index = 0
    end_index = 0

    for (i, line) in enumerate(lines):
        if not found_start and line.startswith('//! ```toml'):
            found_start = True
            start_index = i
            end_index = i
        elif found_start and line.startswith('//! ```'):
            end_index = i
            break

    if not found_start:
        raise Exception(
            "did not find start of ```toml block in the toplevel documentation comments"
        )

    snippet = lines[(start_index + 1):end_index]

    without_comment = [s.removeprefix('//! ') for s in snippet]

    return "".join(without_comment)

def check_dependency_version(cargo_toml_filename, cargo_toml, other_filename, other_toml,
                             dependency_name):
    dep_in_cargo_toml = get_crate_version(cargo_toml, dependency_name)
    dep_in_other = get_crate_version(other_toml, dependency_name)

    if dep_in_cargo_toml != dep_in_other:
        raise Exception(
            f"""{dependency_name} version in {cargo_toml_filename} is {dep_in_cargo_toml} but 
            is referenced in {other_filename} as {dep_in_other}"""
        )

def check():
    cargo_toml = toml.load('rsvg/Cargo.toml')
    librsvg_version = cargo_toml['package']['version']

    example_file = open('rsvg/src/lib.rs')
    example_contents = example_file.readlines()
    example_toml_str = find_toml_in_rust_toplevel_docs(example_contents)
    example_toml = toml.loads(example_toml_str)

    example_version = get_crate_version(example_toml, 'librsvg')

    if librsvg_version != example_version:
        raise Exception(
            f"""librsvg version in rsvg/Cargo.toml is {librsvg_version} but is referenced as
            {example_version} in rsvg/src/lib.rs"""
        )

    DEPENDENCIES = [
        'cairo-rs',
        'gio',
    ]

    cargo_toml = toml.load('Cargo.toml')
    for dependency_name in DEPENDENCIES:
        check_dependency_version(
            'Cargo.toml',
            cargo_toml,
            'rsvg/src/lib.rs',
            example_toml,
            dependency_name
        )

    print("Dependency versions match in rsvg/src/lib.rs.  All good!", file=sys.stderr)

if __name__ == '__main__':
    check()
