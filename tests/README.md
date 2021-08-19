Librsvg test suite
==================

Librsvg's test suite is split like this:

* Unit tests in the Rust code, run normally with "cargo test".

* Rust integration tests in this tests/ directory.

* C API tests in this tests/ directory.

The C API and Rust tests run the library with its public APIs in
both languages.  In addition, the Rust tests also exercise the
rsvg-convert program.

**For the impatient:** you can use `cargo test` to run most of the
test suite; this is fine for regular development.  This will *not* run
the C API tests or some of the long-running tests that exercise the
hard-coded limits of the library.

To run the full test suite, see ["Running the test
suite"](#running-the-test-suite) below.

Unit tests
----------

The library's source code has small unit tests for particular sections
of the code.

**It is better to catch errors early**, in the unit tests, if
possible.  The test suite in this tests/ directory is for black box
tests, which run the library as a normal program would use it.

* **What should be in a unit test** - a small test of an algorithm; a
  check for computed values given some starting values; checks for
  edge cases.

* **What should be in these black-box tests** - rendering tests that
  exercise a particular part of the code; CSS cascading tests; images
  that expose bugs and that we want to avoid regressing on later.

For example, there are unit tests of the path data parser (the `<path
d="M10 10 L20 20 ...">` element and its `d` attribute, to ensure that
the parser handles all the path commands and catches errors
appropriately.  Correspondingly, there are a bunch of black-box tests
that exercise particular features of path rendering ("does this
actually draw a line, or an arc?").


Running the test suite
----------------------

For regular development, use `cargo test`.  This will run most of the
test suite, except for the C API tests and the long-running tests
which exercise the hard-coded limits of the library.

To run the full test suite, you need to go through autotools.  Run the
following commands in the toplevel source directory:

```sh
./autogen.sh
make check
```

## C API tests - `api.c`

These test the full C API of librsvg: all the public functions; the
RsvgHandle class, its methods, and its GObject properties; all the
deprecated functions.  Any new public APIs should get tested here.

The tests are intended to preserve the historical peculiarities of the
C API, to ensure ABI compatibility across versions of the library.

These tests are not meant to exhaustively test librsvg's features.
For those, you should look at the [Rust integration
tests][#rust-integration-tests].

This C API test suite is built upon [Glib's GTest utility
functions][gtest], which let you define tests in the C language.

## Rust integration tests

These are built as a Rust binary in this tests/ directory, and are
runnable with `cargo test`.

### Rust API tests - `api.rs`

Tests the public Rust API of librsvg.

### Crash tests - `loading_crash.rs`

These load and parse an SVG, and ensure that there are no crashes in
the process.  Note that this does *not* render the images.

The SVG images live in the `fixtures/crash` directory.  The files are
just tested to not cause crashes during the loading process; it does
not matter if the files are well-formed XML, for example.

## Rendering crash tests - `render_crash.rs`

We use these tests to ensure there are no regressions after fixing a
bug where a particular SVG loads fine, but it crashes the renderer.

The test files are in the `fixtures/render-crash` directory.  The
module loads the files and renders them, without comparing the results
to anything in particular.

## General bug regression tests - `bugs.rs`

These test fixes for specific bugs in the library, so that the bugs don't recur.

## Error tests - `errors.rs`

These test conditions which should produce errors during loading or rendering.

During loading, librsvg will report malformed XML as errors.  It will
also report an error if an SVG file has more elements than what is
configured in librsvg's internal limits; this is intended to prevent
uncontrolled memory consumption.

During rendering, librsvg will report errors if the SVG document hits
librsvg's internal limits for the number of instanced objects; this is
intended to prevent uncontrolled CPU consumption from malicious SVG
files.

The test files are in the `fixtures/errors` directory.

## Tests for SVG filter effects - `filters.rs`

These test the semantics of the `filter` property, and specific filter functions.

## Reference tests - `reference.rs`

These are the bulk of the rendering tests, where the results of
rendering SVG files are compared against reference PNG images.

The reference tests allow for minor differences in the pixel values of
the results.  Each pixel's RGBA components gets compared to the
corresponding one in the reference image:

* If the absolute value of the difference between corresponding RGBA
  components is more than 2, the test suite considers the result to be
  *distinguishable* from the reference, but otherwise acceptable.

* If the absolute value of the difference is more than the number in
  the `RSVG_TEST_TOLERANCE` environment variable, the result is
  *inacceptable* and the test suite fails; the default is 2 if that
  variable is not set.  You can tweak this value if your machine's
  floating-point unit produces wildly different results.

The test files are in the `fixtures/reftests/` directory.  Each
image-based reference test uses two files: `foo.svg` and
`foo-ref.png`.  The test harness will render `foo.svg` and compare the
results to `foo-ref.png`.

Failing tests will appear as part of the `cargo test` output.  It will
print the filenames for the output and difference images for failed
tests, as follows.

Each `foo.svg` test file produces a `foo-out.png` result, and if that
result is *distinguishable* from the reference PNG (per the
terminology above), the test will also produce a `foo-diff.png` which
you can examine by hand.  See "[Examining failed reference
tests](#examining-failed-reference-tests)" below.

**Ignoring tests:** SVG test files in `fixtures/reftests` whose names
begin with "`ignore`" will be skipped from the tests.  That is,
anything that matches "ignore*.svg`" will not be included in the
tests.  You can use this to skip a few problematic files temporarily.

As of 2020/Oct/22 we have an informal organization of these files:

* `fixtures/reftests/svg1.1` - Tests from the W3C's SVG1.1 test suite.
  These are supposed to test all of SVG's features; we will add them one
  by one as librsvg starts implementing the features.
  
* `fixtures/reftests/svg2` - Tests for SVG2 or CSS3 features.

* `fixtures/reftests/bugs/*.svg` - Tests for particular bug numbers.
  Please use the bug number from Gitlab, like `1234-blah.svg`, and the
  corresponding `1234-blah-ref.png` for the known-good reference image.
  
  **Note:** Librsvg migrated from git.gnome.org and bugzilla.gnome.org
  to gitlab.gnome.org.  Bug numbers in Bugzilla were around 6 digits
  in length; in Gitlab, they are small numbers.

* `fixtures/reftests/*.svg` - Tests for special situations
  that arose during development.
  
* `fixtures/reftests/adwaita/*.svg` - A snapshot of the Adwaita icon
  theme (GNOME's default icon theme), to ensure that librsvg renders
  it correctly.

### Examining failed reference tests

Let's say you run `make check` and see that one of the tests fails.  The test log may have lines like these:

```
---- reference::svg_1_1_tests_fixtures_reftests_svg1_1_painting_stroke_01_t_svg stdout ----
output: output/painting-stroke-01-t-out.png
painting-stroke-01-t: 12414 pixels changed with maximum difference of 255
diff: output/painting-stroke-01-t-diff.png
thread 'reference::svg_1_1_tests_fixtures_reftests_svg1_1_painting_stroke_01_t_svg' panicked at 'surfaces are too different', tests/src/reference.rs:319:25

```

This means that the test file
`fixtures/reftests/svg1.1/painting-stroke-01-t.svg` got rendered, and
produced incorrect output when compared to
`fixtures/reftests/svg1.1/painting-stroke-01-t-ref.png`.

When a test fails, rsvg-test creates two images in `tests/output`:

```
tests/output/foo-out.png
tests/output/foo-diff.png
```

In this case, `foo-out.png` is the actual rendered output, which is presumed to
be incorrect, since it differs from the `foo-ref.png` reference image.
The `foo-diff.png` is a "visual diff" that you can see in an image
viewer; pixels that differ are highlighted.

It is up to you to decide what to do next:

* If the `foo-out.png` image looks correct, and the only difference
  with respect to the `foo-ref.png` reference image is that
  antialiased edges look different, or font rendering is slightly
  different due to the font-rendering machinery in your system, you
  can just regenerate the test image.  See 
  "[Regenerating reference images](#regenerating-reference-images)" below.

* If the `foo-out.png` image is obviously wrong when compared to the
  `foo-ref.png` reference, you can [file a bug][bug].  You can wait
  until someone fixes it, or try to [fix the bug yourself][pull-requests]!

* Any other situation of course deserves attention.  Feel free to [ask
  the maintainers][maintainer] about it; even if you figure out the problem
  yourself, a failed test almost always indicates a problem that is
  not just on your end.


### Regenerating reference images

Let's say the test `tests/fixtures/reftests/.../foo.svg` failed.  Then you
fix the bug, or determine that the output image is in fact correct,
and it just differs from the reference image due to antialiasing
artifacts.  In this case, your next step is to regenerate the
reference image so the test passes again.

**You should not just use rsvg-convert to render test files!**  The
test machinery sets up conditions for [reproducible font
rendering][#reproducible-font-rendering], which are not available to
rsvg-convert.

Run `cargo test`, and copy the resulting `foo-out.png` to the
`tests/fixtures/.../foo-ref.png` that corresponds to `foo.svg`.

You can then run `cargo test` again and ensure that the tests pass.

### Issues with the official SVG test suite

Our SVG files in tests/fixtures/reftests/svg1.1 come from the "SVG 1.1
Second Edition test suite" archive linked here:

https://www.w3.org/Graphics/SVG/WG/wiki/Test_Suite_Overview

We don't know how the reference PNG files in that archive are
generated.  However, they are done in such a way that objects tend not
to be pixel-aligned.  For example, many tests have a rectangular frame
around the whole viewport, defined like this:

```
<rect id="test-frame" x="1" y="1" width="478" height="358" fill="none" stroke="#000000"/>
```

This specifies no stroke width, so it uses 1 by default.  The desired
effect is "stroke this rectangle with a 1-pixel wide line".

However, notice that the (x, y) coordinates of the rect are (1, 1).
This means that the actual bounds of the stroked outline are from
(0.5, 0.5) to (479.5, 359.5).  The result is a fuzzy outline: it
occupies two pixels of width, with each pixel having half-black
coverage.

Some elements in the reference PNG images from the official SVG test
suite are pixel-aligned, and some are not, like the example test-frame
above.  It looks like their renderer uses a (0.5, 0.5) offset just for
strokes, but not for fills, which sounds hackish.

Our test suite **does not** use special offsets, so that SVG images
not from the official test suite are rendered "normally".  **This means
that the reference images from the official test suite will always
fail initially**, since stroke outlines will be fuzzy in librsvg, but
not in the test suite (and conversely, SVGs *not* from the test suite
would be crisp in librsvg but probably not in the test suite's
renderer renderer).

Also, the original reference PNGs from the SVG 1.1 test suite either
use fonts that are different from those usually on free software
systems, or they use SVG fonts which librsvg currently doesn't support
(i.e. with glyph shapes referenced from a secondary SVG).

In any case, look at the results by hand, and compare them by eye to
the official reference image.  If the thing being tested looks
correct, and just the outlines are fuzzy — and also it is just the
actual font shapes that are different — then the test is probably
correct.  Follow the procedure as in
"[Regenerating reference images](#regenerating-reference-images)"
listed above in order to have a reference image suitable for librsvg.

### Reproducible font rendering

The test runners set up special conditions so that font rendering is
reproducible across systems.  Normally, font rendering can vary quite
a bit depending on various factors:

* Versions of fontconfig, freetype, cairo, and pango.
* Installed fonts.
* The system's font mappings.
* The user's settings for font antialiasing, hinting, etc.

The test suite includes part of the **Roboto** fonts in
`librsvg/tests/resources`, and creates a configuration font map with
just those fonts.  In addition, the Pango context used for rendering
is set up with a hardcoded mode for antialiasing, hinting, and hint
metrics.


[gtest]: https://docs.gtk.org/glib/testing.html
[bug]: ../CONTRIBUTING.md#reporting-bugs
[pull-requests]: ../CONTRIBUTING.md#pull-requests
[maintainer]: README.md#maintainers
