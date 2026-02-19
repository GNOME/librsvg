# Evaluation Test Cases for AI Code Analysis Agent

This document contains test cases designed to evaluate an AI Code Analysis Agent's ability to perform context-aware code review on the librsvg codebase.

## Test Case 1: Security - URL Resolution Bypass

**File**: `rsvg/src/url_resolver.rs` (historical issue - already fixed)

**Scenario**: A PR introduces a change to `resolve_href` that removes the check for query strings in URLs.

**Change**:
```rust
// BEFORE (secure):
if url.query().is_some() {
    return Err(AllowedUrlError::NoQueriesAllowed);
}

// AFTER (vulnerable - proposed change):
// Query string check removed
```

**Expected Analysis**:
- Agent should identify this as a security vulnerability (CVE reference)
- Agent should explain the attack vector (query strings can be used to access arbitrary files)
- Agent should check for existing tests covering this case

---

## Test Case 2: Memory Safety - Unsafe Pointer Arithmetic

**File**: `rsvg/src/surface_utils/shared_surface.rs`

**Scenario**: A contributor proposes a change to optimize pixel access.

**Change**:
```rust
// Current code:
let value = unsafe { *(self.data_ptr.as_ptr().offset(offset) as *const u32) };

// Proposed "optimization":
let value = *(self.data_ptr.as_ptr() as *const u32).add(offset);
```

**Expected Analysis**:
- Agent should recognize both are equivalent in safety
- Agent should note the importance of the `unsafe` block
- Agent should check if bounds checking is performed elsewhere
- Agent should verify the offset calculation is correct for the data type

---

## Test Case 3: API Compatibility - Removing Public Field

**File**: `rsvg/src/api.rs`

**Scenario**: A refactoring PR proposes making a struct field private.

**Change**:
```rust
// BEFORE:
pub struct Loader {
    pub session: Session,
    // other fields
}

// AFTER:
pub struct Loader {
    session: Session,  // Made private
    // other fields
}
```

**Expected Analysis**:
- Agent should identify this as a breaking API change
- Agent should search for external usages of `Loader.session`
- Agent should check if there's a getter method provided
- Agent should note the version bump requirement (semver)

---

## Test Case 4: Performance - Unnecessary Clone

**File**: `rsvg/src/element.rs`

**Scenario**: A contributor adds defensive cloning.

**Change**:
```rust
// BEFORE:
fn get_attributes(&self) -> &HashMap<QualName, Value> {
    &self.attributes
}

// AFTER (with unnecessary clone):
fn get_attributes(&self) -> HashMap<QualName, Value> {
    self.attributes.clone()  // Unnecessary allocation
}
```

**Expected Analysis**:
- Agent should identify unnecessary heap allocation
- Agent should suggest returning a reference instead
- Agent should consider if the clone was added for thread safety (Rc<T> or Arc<T> alternative)

---

## Test Case 5: Correctness - Floating Point Comparison

**File**: `rsvg/src/float_eq_cairo.rs`

**Scenario**: A contributor proposes using direct equality for coordinate comparison.

**Change**:
```rust
// BEFORE:
fn approximately_equal(a: f64, b: f64) -> bool {
    (a - b).abs() < EPSILON
}

// AFTER:
fn approximately_equal(a: f64, b: f64) -> bool {
    a == b  // Direct comparison - problematic
}
```

**Expected Analysis**:
- Agent should explain floating-point precision issues
- Agent should reference the existing `float_eq_cairo.rs` module
- Agent should warn about NaN handling
- Agent should provide context on where this function is used

---

## Test Case 6: Concurrency - Missing Send/Sync

**File**: `rsvg/src/filters/mod.rs`

**Scenario**: A new filter primitive is added with internal state.

**Change**:
```rust
// New filter primitive:
pub struct CustomFilter {
    cache: RefCell<Vec<u8>>,  // Not Send/Sync
}

unsafe impl Send for CustomFilter {}
unsafe impl Sync for CustomFilter {}
```

**Expected Analysis**:
- Agent should question the blanket `unsafe impl`
- Agent should verify the internal state is actually safe to share across threads
- Agent should suggest using `Arc<Mutex<T>>` or similar if thread safety is needed
- Agent should note that `unsafe impl` requires careful justification

---

## Test Case 7: Error Handling - Silent Failure

**File**: `rsvg/src/path_parser.rs`

**Scenario**: A contributor proposes to continue parsing on errors.

**Change**:
```rust
// BEFORE:
fn parse_number(&mut self) -> Result<f64, ParseError> {
    let token = self.lexer.next()?;
    // ... validation
}

// AFTER:
fn parse_number(&mut self) -> Result<f64, ParseError> {
    let token = self.lexer.next().unwrap_or(Token::Number(0.0));
    // Silently defaults to 0 on error
}
```

**Expected Analysis**:
- Agent should identify silent failure pattern
- Agent should explain why this is dangerous (SVG rendering would produce wrong output)
- Agent should suggest using `?` or explicit error propagation
- Agent should check for test coverage of error paths

---

## Test Case 8: Documentation - Missing Safety Contract

**File**: `rsvg/src/util.rs`

**Scenario**: An unsafe function is added without safety documentation.

**Change**:
```rust
// New function:
pub unsafe fn utf8_cstr<'a>(s: *const libc::c_char) -> &'a str {
    // No safety documentation
    unsafe { str::from_utf8_unchecked(CStr::from_ptr(s).to_bytes()) }
}
```

**Expected Analysis**:
- Agent should require `# Safety` section in documentation
- Agent should note the caller must ensure null termination
- Agent should check for existing unsafe function patterns in the codebase
- Agent should suggest using `cstr` crate as alternative

---

## Test Case 9: Testing - Missing Edge Case

**File**: `rsvg/src/parsers.rs`

**Scenario**: A new parser is added with incomplete test coverage.

**Change**:
```rust
// New parser with basic tests:
impl Parse for Color {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        // ... implementation
    }
}

// Tests only cover happy path:
#[test]
fn test_color_parsing() {
    assert_eq!(Color::parse_str("red"), Ok(Color::Red));
}
```

**Expected Analysis**:
- Agent should identify missing test cases:
  - Invalid color names
  - Hex colors (#RGB, #RRGGBB, #RRGGBBAA)
  - rgb()/rgba() functional notation
  - Empty string
  - Case sensitivity
- Agent should check existing parser test patterns in codebase

---

## Test Case 10: Regression - Breaking Existing Behavior

**File**: `rsvg/src/text.rs`

**Scenario**: A refactoring changes text baseline calculation.

**Change**:
```rust
// BEFORE:
fn calculate_baseline(offset: f64) -> f64 {
    offset * 1.2  // Empirical value from spec
}

// AFTER (mathematically "correct" but changes rendering):
fn calculate_baseline(offset: f64) -> f64 {
    offset  // Simplified
}
```

**Expected Analysis**:
- Agent should flag this as a potential rendering regression
- Agent should search for reference tests that might be affected
- Agent should note that visual output tests exist in `tests/fixtures/reftests/`
- Agent should require visual regression testing

---

## Test Case 11: Dependency Update - Breaking Change

**File**: `Cargo.toml`

**Scenario**: A contributor updates a dependency version.

**Change**:
```toml
# BEFORE:
cairo-rs = "0.20.0"

# AFTER:
cairo-rs = "0.22.0-alpha"
```

**Expected Analysis**:
- Agent should check changelog for breaking changes
- Agent should verify API compatibility with existing code
- Agent should check for alpha/beta version risks
- Agent should verify CI passes with new version
- Agent should note version in release notes

---

## Test Case 12: Clippy Violation - Style Inconsistency

**File**: `rsvg/src/shapes.rs`

**Scenario**: New code doesn't follow existing patterns.

**Change**:
```rust
// New code uses different pattern:
let width = match self.width {
    Length::Pixels(v) => v,
    _ => 0.0,
};

// Existing code pattern:
let width = self.width.resolve_or(0.0);
```

**Expected Analysis**:
- Agent should run clippy and report warnings
- Agent should identify the more idiomatic pattern
- Agent should check existing codebase for preferred patterns

---

## Test Case 13: Internationalization - Hardcoded String

**File**: `rsvg/src/error.rs`

**Scenario**: Error message added without proper i18n handling.

**Change**:
```rust
// New error variant:
ValueErrorKind::Parse(String),  // Message in English

// Usage:
return Err(ValueErrorKind::Parse("Expected a number".to_string()));
```

**Expected Analysis**:
- Agent should note that this is a user-facing string
- Agent should check if librsvg has i18n infrastructure
- Agent should consider if error messages need translation

---

## Test Case 14: Performance - Inefficient String Concatenation

**File**: `rsvg/src/gradient.rs`

**Scenario**: New code uses repeated string concatenation.

**Change**:
```rust
// Inefficient:
let mut result = String::new();
for segment in segments {
    result.push_str(&segment);
    result.push(' ');
}

// Efficient alternative:
let result = segments.join(" ");
```

**Expected Analysis**:
- Agent should identify O(n²) complexity
- Agent should suggest using `join()` or `concat()`
- Agent should note this matters in hot paths

---

## Test Case 15: Build Configuration - Feature Gate

**File**: `rsvg/src/lib.rs`

**Scenario**: New code should be feature-gated.

**Change**:
```rust
// New code added without feature gate:
#[cfg(test)]
mod benchmark_helpers {
    // These are only needed for benchmarks
}

// Should be:
#[cfg(feature = "bench-utils")]
mod benchmark_helpers {
    // Only compiled when benchmarking
}
```

**Expected Analysis**:
- Agent should identify unnecessary compilation in release builds
- Agent should check existing feature flags in Cargo.toml
- Agent should suggest appropriate feature name

---

## Test Case 16: Security - Integer Overflow

**File**: `rsvg/src/limits.rs`

**Scenario**: New code doesn't check for overflow.

**Change**:
```rust
// New calculation without overflow check:
let total_size = width * height * 4;  // Could overflow for large images

// Safer alternative:
let total_size = width.checked_mul(height).and_then(|h| h.checked_mul(4));
```

**Expected Analysis**:
- Agent should identify potential integer overflow
- Agent should check for existing overflow handling patterns
- Agent should consider DoS attack vectors (malicious SVG with large dimensions)

---

## Test Case 17: API Design - Builder Pattern

**File**: `rsvg/src/api.rs`

**Scenario**: New API should use builder pattern.

**Change**:
```rust
// Complex constructor:
let renderer = CairoRenderer::new(
    handle,
    Some(dpi),
    Some(width),
    Some(height),
    Some(keep_aspect_ratio),
    None,
);

// Should be:
let renderer = CairoRenderer::new(handle)
    .dpi(96.0)
    .width(800)
    .height(600)
    .keep_aspect_ratio(true);
```

**Expected Analysis**:
- Agent should suggest builder pattern for complex constructors
- Agent should check existing builder patterns in codebase
- Agent should note backward compatibility considerations

---

## Test Case 18: Testing - Flaky Test

**File**: `rsvg/tests/reference.rs`

**Scenario**: Test depends on system time or random values.

**Change**:
```rust
#[test]
fn test_render_timestamp() {
    let surface = render_svg("timed.svg");
    // Comparison depends on current time in the SVG
    assert_eq!(surface.get_pixel(0, 0), expected_pixel);
}
```

**Expected Analysis**:
- Agent should identify test flakiness
- Agent should suggest using fixed test fixtures
- Agent should note the reference test tolerance settings

---

## Test Case 19: Code Review - Large PR

**Scenario**: A contributor submits 50+ files changed.

**Expected Analysis**:
- Agent should suggest splitting into smaller PRs
- Agent should identify logical groupings (e.g., "API changes", "refactoring", "new feature")
- Agent should prioritize security and correctness issues
- Agent should note review burden on maintainers

---

## Test Case 20: CI/CD - Missing Test Run

**Scenario**: PR modifies code but doesn't run all relevant tests.

**Change**: Code changes in `filters/` module but only `cargo test --test api` is run.

**Expected Analysis**:
- Agent should identify which test suites are relevant
- Agent should note reference tests exist for visual rendering
- Agent should suggest running filter-specific tests

---

## Running These Evaluations

To use these test cases:

1. Create a branch with each proposed change
2. Run the AI Code Analysis Agent on the branch
3. Compare the agent's output against the "Expected Analysis"
4. Score the agent on:
   - **Security awareness**: Cases 1, 6, 16
   - **Correctness**: Cases 5, 7, 10
   - **Performance**: Cases 4, 14
   - **API/Compatibility**: Cases 3, 11, 17
   - **Code Quality**: Cases 2, 8, 12, 13, 15
   - **Testing**: Cases 9, 10, 18
   - **Process**: Cases 19, 20
