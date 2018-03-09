Architecture of librsvg
=======================

This document describes the architecture of librsvg, and future plans
for it.

# A bit of history

Librsvg is an old library.  It started around 2001, when Eazel (the
original makers of GNOME's file manager Nautilus) needed a library to
render SVG images.  At that time the SVG format was being
standardized, so librsvg grew along with the SVG specification.  This
is why you will sometimes see references to deprecated SVG features in
the source code.

Librsvg started as an experiment to use libxml2's new SAX parser, so
that SVG could be streamed in, instead of first creating a DOM.
Originally it used libart as a rendering library; this was GNOME's
first antialiased renderer with alpha compositing.  Later, the
renderer was replaced with Cairo.

Librsvg started as a C library with an ad-hoc API.  At some point it
got turned into a GObject library, so that the main `RsvgHandle`
object defines most of the entry points into the library.  Through
GObject Introspection, this allows librsvg to be used from other
programming languages.

In 2016, librsvg started getting ported to Rust.  The plan is to leave
the C API/ABI intact, but to have as much of the internals as possible
implemented in Rust.  This way we can use a memory-safe, modern
language, but retain the traditional API/ABI.

# RsvgHandle

The `RsvgHandle` object is the only GObject that librsvg exposes in
the public API.

During its lifetime, an `RsvgHandle` can be in either of two stages:

* Loading - the caller feeds the handle with SVG data.  The SVG XML is
  parsed into a DOM-like structure.

* Rendering - the SVG is finished loading.  The caller can then render
  the image as many times as it wants to Cairo contexts.
  
## Loading SVG data

The following happens in `rsvg_handle_read_stream_sync()`:

* The function peeks the first bytes of the stream to see if it is
  compressed with gzip.  In that case, it plugs a
  `g_zlib_decompressor_new()` to the use-supplied stream.
  
* The function creates an XML parser for the stream.  The SAX parser's
  callbacks are functions that create DOM-like objects within the
  `RsvgHandle`.  The most important callback is
  `rsvg_start_element()`, and the one that actually creates our
  element implementations is `rsvg_standard_element_start()`.

## Translating SVG data into Nodes

`RsvgHandlePrivate` has a `treebase` field, which is the root of the
DOM tree.  Each node in the tree is an `RsvgNode` object.

`rsvg_standard_element_start()` gets called from the XML parsing
machinery; it takes an SVG element name like "`rect`" or "`filter`"
and a key/value list of attributes within the element.  It then creates the
appropriate subclass of an `RsvgNode` object, hooks the node to the
DOM tree, and tells the node to set its attributes from the key/value
pairs.

While a node sets its key/value pairs in its `set_atts()` method, it
may encounter an invalid value, for example, a negative width where
only nonnegative ones are allowed.  In this case the element may
decide to set itself to be "in error" via the `node.set_error()`
method.  If a node is in error, the node's children will get parsed as
usual, but the node and its children will be ignored during the
rendering stage.

The SVG spec says that SVG rendering should stop on the first element
that is "in error".  However, most implementations simply seem to
ignore erroneous elements instead of completely stopping rendering,
and we do the same in librsvg.

## Element attributes and specified/computed values

Some HTML or SVG engines like Gecko / Servo make a very clear
distinction between "specified values" and "computed values" for
element attributes.  Currently librsvg doesn't have a clear
distinction.

Unspecified attributes cause librsvg to use defaults, some as per the
spec, and some (erroneously) as values that seemed to make sense at
the time of implementation.  Please help us find these and make them
spec-compliant!

For specified attributes, sometimes the set_atts() methods will
validate the values and resolve them to their final computed form, and
sometimes they will just store them as they come in the SVG data.  The
computed or actually used values will be generated at rendering time.

# Rendering

The public `rsvg_handle_render_cairo()` and `rsvg_handle_cairo_sub()`
functions initiate a rendering process; the first function just calls
the second one with the root element of the SVG.

This second function creates `RsvgDrawingCtx`, which contains the
rendering state.  This structure gets passed around into all the
rendering functions.  It carries the vtable for rendering in the
`render` field, the CSS state for the node being rendered in the
`state` field, and other values which are changed as rendering
progresses.

# Comparing floating-point numbers

Librsvg sometimes needs to compute things like "are these points
equal?" or "did this computed result equal this test reference
number?".

We use `f64` numbers in Rust, and `double` numbers in C, for all
computations on real numbers.  These types cannot be simply compared
with `==` effectively, since it doesn't work when the numbers are
slightly different due to numerical inaccuracies.

Similarly, we don't `assert_eq!(a, b)` for floating-point numbers.

Most of the time we are dealing with coordinates which will get passed
to Cairo.  In turn, Cairo converts them from doubles to a fixed-point
representation (as of March 2018, Cairo uses 24.8 fixnums with 24 bits of
integral part and 8 bits of fractional part).

So, we can consider two numbers to be "equal" if they would be represented
as the same fixed-point value by Cairo.  Librsvg implements this in
the [`ApproxEqCairo` trait][ApproxEqCairo] trait.  You can use it like
this:

```rust
use float_eq_cairo::ApproxEqCairo; // bring the trait into scope

let a: f64 = ...;
let b: f64 = ...;

if a.approx_eq_cairo(&b) { // not a == b
    ... // equal!
}

assert!(1.0_f64.approx_eq_cairo(&1.001953125_f64)); // 1 + 1/512 - cairo rounds to 1
```

As of March 2018 this is not implemented for the C code; the hope is
that we will move all that code to Rust and we'll be able to do this
kind of approximate comparisons there.

[ApproxEqCairo]: rsvg_internals/src/float_eq_cairo.rs
