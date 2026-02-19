# AGENTS.md - Guidelines for Agentic Coding in Librsvg

This file provides guidelines for agents working on the librsvg codebase.

## Project Overview

Librsvg is a Rust library for rendering SVG images to Cairo surfaces. It is part of the GNOME project and uses a Cargo workspace with multiple crates.

- **Rust Edition**: 2024
- **Minimum Rust Version**: 1.92.0
- **Workspace**: Multiple crates (rsvg, rsvg_convert, librsvg-c, etc.)
- **Default Crates**: rsvg, rsvg_convert

## Build Commands

### Basic Build

```bash
# Build all default crates
cargo build

# Build all targets including tests
cargo build --all-targets --workspace --exclude 'librsvg-rebind*'
```

### Running Tests

```bash
# Run all tests in the workspace
cargo test --workspace --exclude pixbufloader-svg --exclude 'librsvg-rebind*'

# Run tests for a specific crate
cargo test -p rsvg

# Run a single test by name
cargo test test_name

# Run tests excluding certain slow tests
cargo test --workspace --exclude pixbufloader-svg --exclude 'librsvg-rebind*' -- --skip loading_crash --skip reference --skip render_crash

# Run unit tests (inline in source files)
cargo test --lib

# Run doc tests
cargo test --doc
```

### Linting and Formatting

```bash
# Check formatting
cargo fmt --all -- --check

# Auto-fix formatting
cargo fmt --all

# Run clippy lints
cargo clippy

# Run clippy with warnings as errors (as used in CI)
RUSTFLAGS='-D warnings' cargo clippy

# Check for dependency issues
cargo deny check
```

### Documentation

```bash
# Generate Rust documentation
cargo doc --no-deps

# Generate documentation with all features
cargo doc --all-features --no-deps
```

### Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark
cargo bench --bench box_blur
```

## Code Style Guidelines

### Formatting

- Use **4 spaces** for indentation (Rust standard)
- Follow Rust's default formatting (enforced by `cargo fmt`)
- Maximum line length: 100 characters (soft limit, can be exceeded for good reason)
- Use blank lines generously to separate logical sections

### Naming Conventions

- **Types/Enums**: `PascalCase` (e.g., `BoundingBox`, `LengthUnit`)
- **Functions/Methods**: `snake_case` (e.g., `render_document`, `get_dimensions`)
- **Variables**: `snake_case` (e.g., `handle`, `surface`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_DPI`)
- **Traits**: `PascalCase` (e.g., `Parse`, `Normalize`)
- **Type Parameters**: `PascalCase` (e.g., `N: Normalize`, `V: Validate`)

### Imports and Module Structure

Group imports in this order:
1. Standard library imports (`std::`, `core::`)
2. External crate imports (alphabetical)
3. Crate-local imports (`crate::`)

```rust
// Standard library
use std::fmt;
use std::sync::Arc;

// External crates (alphabetical)
use cairo::Context;
use cssparser::Parser;
use gio::prelude::*;

// Crate-local
use crate::document::Document;
use crate::error::*;
use crate::node::Node;
```

### Module Organization

- One module per file or directory
- Use `mod.rs` style for directory modules
- Prefix private modules with underscore if needed: `mod _private`
- Use `pub mod` for public modules, keep everything else private

### Documentation

- Use `//!` for module-level documentation at the top of files
- Use `///` for public function/type documentation
- Document all public APIs
- Include examples in doc comments where helpful

```rust
//! This module handles bounding box calculations.

/// Computes the bounding box for a node.
///
/// # Arguments
///
/// * `node` - The node to compute the bounding box for
///
/// # Returns
///
/// The computed bounding box, or None if the node has no visual representation
pub fn compute_bbox(node: &Node) -> Option<BoundingBox> {
    // implementation
}
```

### Error Handling

- Use custom error enums for domain-specific errors
- Implement `std::error::Error` and `fmt::Display` for error types
- Use `From` implementations for error conversion
- Use `Result<T, Error>` for fallible operations

```rust
#[derive(Debug, Clone)]
pub enum LoadingError {
    Parse(String),
    Io(std::io::Error),
    // ...
}

impl std::error::Error for LoadingError {}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadingError::Parse(s) => write!(f, "parse error: {}", s),
            // ...
        }
    }
}

impl From<std::io::Error> for LoadingError {
    fn from(e: std::io::Error) -> LoadingError {
        LoadingError::Io(e)
    }
}
```

### Public API Design

- Use `#[non_exhaustive]` for enums that may gain new variants
- Use `pub use` to re-export public API from submodules
- Prefix internal-only re-exports with a comment: `// Don't merge these in the "pub use" above!`
- Keep implementation details private

```rust
// Public API - re-exported
pub use crate::{
    drawing_ctx::Viewport,
    error::LoadingError,
    length::RsvgLength as Length,
};

// Internal - NOT part of public API
use crate::document::Document;
use crate::session::Session;
```

### Derive Macros

Use these derive macros where appropriate:
- `#[derive(Debug, Clone, Copy, PartialEq)]` for simple data types
- `#[derive(Default)]` for types with sensible defaults
- `#[derive(Serialize, Deserialize)]` for types that need serialization

### Testing

- Place unit tests in the same file using `#[cfg(test)]` and `#[test]` modules
- Place integration tests in the `tests/` directory
- Use `proptest` for property-based testing
- Use the test utilities in `rsvg/src/test_utils/`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() {
        assert_eq!(expected, actual);
    }
}
```

### Conditional Compilation

- Use `#[cfg(feature = "...")]` for feature-gated code
- Document unexpected cfgs in Cargo.toml:

```toml
[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = [
    'cfg(system_deps_have_fontconfig)',
    'cfg(system_deps_have_pangoft2)',
] }
```

### GObject Integration

- The project uses GLib/GObject for C API bindings
- Follow the patterns in `librsvg-c` crate for C interop
- Use `#[repr(C)]` for types that need C compatibility

## CI Configuration

The project uses GitLab CI with these key jobs:
- `check`: Basic cargo check
- `cargo_test`: Test suite
- `fmt`: Formatting check
- `clippy`: Linting
- `deny`: Dependency audit

CI uses `RUSTFLAGS='-D warnings'` to treat warnings as errors.

## Additional Resources

- [Development Guide](https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/index.html)
- [Contributing Guide](https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/contributing.html)
- [Cargo.toml](./Cargo.toml) - Workspace configuration
- [deny.toml](./deny.toml) - Dependency audit configuration
