Librsvg test suite
==================

Librsvg's test harness is built upon [Glib's GTest utility
functions][gtest].  These let you define tests in the C language.

There are several different styles of tests:

* `rsvg-test` - The main test suite; loads, parses, and renders SVGs and
  compares the results against reference PNG images.

* `loading` - Tests that compressed and uncompressed SVGs are loaded
  correctly even if read one or two bytes at a time.  This is
  basically a test for the state machines in the loading code.

* `crash` - Ensures that loading and parsing (but not rendering) a
  particular SVG doesn't crash or yield a GError.
  
* `render-crash` - Ensures that rendering a loaded SVG doesn't crash.

* `dimensions` - Loads an SVG, and tests that librsvg computes the
  correct dimensions for the toplevel SVG element, or one of the
  individual sub-elements.

* `styles` - Loads and parses an SVG, and checks that a particular SVG
  object has a certain style.  These tests are currently disabled,
  since they depend on internal APIs of librsvg.

These are all "black box tests": they run the library with its public
API, and test the results.  They do not test the library's internals;
just the output.


Unit tests
----------

Additionally, the library's source code has smaller unit tests for
particular sections of the code.

**It is better to catch errors early**, in the unit tests, if
possible.  The test suite in this directory is for black box tests,
which run the library as a normal program would use it.

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

The easiest way to run all the tests is to go to librsvg's toplevel
directory and run `make check`.  This will run both the small unit
tests and the black box tests in this `librsvg/tests` directory.

If you want to run just the black box tests, go into this
`librsvg/tests` directory and run `make check`.  If you want to run
the unit tests, go to `librsvg/rsvg_internals` and run `cargo test`.

Those commands will yield exit code 0 if all the tests pass, or
nonzero if some tests fail.

Running `make check` will produce a `test/test-suite.log` file.  You can
see this file for the details of failed tests.

Additionally, all the black box tests (rsvg-test, crash, etc.) will
produce a test report in a text file.  In the tests directory, you can
see `rsvg-test.log`, `crash.log`, etc., respectively.


# Tests and test fixtures

## Image-based reference tests for `rsvg-test.c`

These will load, parse, and render an SVG, and compare the results
against a reference PNG image.

Each image-based reference test uses two files: `foo.svg` and
`foo-ref.png`.  The test harness will render `foo.svg` and compare the
results to `foo-ref.png`.  Currently we assume a pixel-perfect match.
If there are differences in the output, the test will fail; see
"[Examining failed reference tests](#examining-failed-reference-tests)" below.

These files can go anywhere under the `fixtures/reftests`
directory; the `rsvg-test` program will recursively look inside
`fixtures/reftests` for all SVG files, render them to `tests/output`, and
compare them to the `-ref.png` reference images. The rendered files can
later be removed by running `make clean`.

**Ignoring tests:** SVG test files or entire subdirectories in
`fixtures/reftests` whose names begin with "`ignore`" will be skipped from
the tests.  That is, anything that matches "`fixtures/reftests/ignore*`"
will not be included in the tests.  You can use this to skip a few
problematic files temporarily.

As of 2016/Nov/03 we have an informal organization of these files:

* `fixtures/reftests/svg1.1` - Tests from the W3C's SVG test suite.
  These are supposed to test all of SVG's features; we will add them one
  by one as librsvg starts implementing the features.

* `fixtures/reftests/bugs/*.svg` - Tests for particular bug numbers.
  Please use the bug number from Gitlab, like `1234.svg`, and the
  corresponding `1234-ref.png` for the known-good reference image.
  
  **Note:** Librsvg migrated from git.gnome.org and bugzilla.gnome.org
  to gitlab.gnome.org.  Bug numbers in Bugzilla were around 6 digits
  in length; in Gitlab, they are small numbers.

* `fixtures/reftests/*.svg` - Tests for special situations
  that arose during development.

### Examining failed reference tests

Let's say you run `make check` and see that one of the tests fails.
For example, `rsvg-test.log` may have lines that look like

```
# Storing test result image at tests/output/paths-data-18-f-out.png
# 6798 pixels differ (with maximum difference of 255) from reference image

# Storing test result image at tests/output/paths-data-18-f-diff.png
not ok 29 /rsvg-test/reftests/svg1.1/paths-data-18-f.svg
FAIL: rsvg-test 29 /rsvg-test/reftests/svg1.1/paths-data-18-f.svg
```

This means that the test named
`/rsvg-test/reftests/svg1.1/paths-data-18-f.svg` failed:  those are
autogenerated test names from GTest.  In this case, it means that the
test file `fixtures/reftests/svg1.1/paths-data-18-f.svg` got rendered,
and produced incorrect output when compared to
`fixtures/reftests/svg1.1/paths-data-18-f-ref.png`.

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

Let's continue with the example above, where the test
`/rsvg-test/reftests/svg1.1/paths-data-18-f.svg` failed.  Let's say
you fix the bug, or determine that the output image is in fact
correct, and just differs from the reference image due to antialiasing
artifacts.  In this case, your next step is to regenerate the
reference image so the test passes again.

If you want to regenerate the reference image
`fixtures/reftests/foo/bar-ref.png` from the corresponding `bar.svg`, you can run

```
rsvg-convert -o fixtures/reftests/foo/bar-ref.png fixtures/reftests/foo/bar.svg
```

This is just the normal rsvg-convert program doing its job; it will
just render the SVG image and output it to the specified PNG file.

You can then run `make check` again and ensure that the tests pass.


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


## Loading tests for `loading.c`

These test the code that decompresses compressed SVGs and feeds the
XML reader, by tring to load SVG data one or two bytes at a time.  The
SVG and SVGZ images are in the `fixtures/loading` directory.

## Crash tests for `crash.c`

These load and parse an SVG, and ensure that there are no errors in
the process.  Note that this does *not* render the images.

The SVG images live in the `fixtures/crash` directory.  The `crash`
program will look recursively look inside `fixtures/crash` for all SVG
files, load them, and ensure that no GError was produced while loading
each file.

## Rendering crash tests for `render-crash.c`

We use these tests to ensure there are no regressions after fixing a
bug where a particular SVG loads fine, but it crashes the renderer.
The test files are in the `fixtures/render-crash` directory.

## Dimensions tests for `dimensions.c`

Here we test that librsvg computes the correct dimensions for objects
in an SVG file.  The test files are in the `fixtures/dimensions`
directory.  The expected dimensions are declared in the test fixtures
in `dimensions.c`.

## Style tests for `styles.c`

These tests are currently disabled, since they depend on internal APIs
that are not visible to the test suite.

These load and parse an SVG, ask librsvg to fetch an SVG object by its
id, and ensure that the object has certain style properties with
particular values.

The SVG images live in the `fixtures/styles` directory.  The
corresponding object IDs and values to be tested for are in the
`styles.c` source code.


[gtest]: https://developer.gnome.org/glib/stable/glib-Testing.html
[bug]: ../CONTRIBUTING.md#reporting-bugs
[pull-requests]: ../CONTRIBUTING.md#pull-requests
[maintainer]: README.md#maintainers
