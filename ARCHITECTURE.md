Architecture of librsvg
=======================

This document describes the architecture of librsvg, and future plans
for it.

The library's internals are being documented at
https://gnome.pages.gitlab.gnome.org/librsvg/doc/rsvg_internals/index.html

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

*Through this document we may use **node** and **element**
interchangeably:* a node is the struct we use to represent an SVG/XML
element.

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

The public `rsvg_handle_render_cairo()` and `rsvg_handle_render_cairo_sub()`
functions initiate a rendering process; the first function just calls
the second one with the root element of the SVG.

This second function creates `RsvgDrawingCtx`, which contains the
rendering state.  This structure gets passed around into all the
rendering functions.  It carries the vtable for rendering in the
`render` field, the CSS state for the node being rendered in the
`state` field, and other values which are changed as rendering
progresses.

## CSS cascading

For historical reasons, librsvg does the CSS cascade *and* rendering
in a single traversal of the tree of nodes.  This is somewhat awkward,
and in the future we hope to move to a Servo-like model where CSS is
cascaded and sizes are resolved before rendering.

Rendering starts at `rsvg_handle_render_cairo_sub()`.  It calls
`rsvg_cairo_new_drawing_ctx()`, which creates an `RsvgDrawingCtx` with
a default `state`:  this is the default CSS state per
`rsvg_state_init()` (in reality that state carries an affine
transformation already set up for this rendering pass; we can ignore
it for now).

Then, `rsvg_handle_render_cairo_sub()` starts the recursive drawing
process by calling
`rsvg_drawing_ctx_draw_node_from_stack()`, starting at the tree root
(`handle->priv->treebase`).  In turn, that function creates a
temporary `state` struct by calling `rsvg_drawing_ctx_state_push()`,
calls `rsvg_node_draw()` on the current node, and destroys the temporary
`state` struct with `rsvg_drawing_ctx_state_pop()`.

Each node draws itself in the following way:

* It resolves relative lengths from the size of current viewport by calling
  `length.normalize()` on each length value.  The size of the current
  viewport is maintained as a stack of `RsvgBbox` structures (it
  stands for "bounding box").

* It calls drawing_ctx::state_reinherit_top() with the node's own
  `state` field.  This causes the temporary state in the `draw_ctx` to
  obtain the final cascaded CSS values.

* It calls the low-level rendering functions like
  `drawing_ctx::render_path_builder()` or
  `drawing_ctx::render_pango_layout()`.  These functions translate the
  values from the `state` in the `draw_ctx` into Cairo values, they
  configure the `cairo::Context`, and call actual Cairo functions to
  draw paths/text/etc.

### What about referenced nodes which have a different cascade?

Sometimes, though, the node being considered in the recursive
traversal has to refer to some other node.  For example, a shape like
a `rect`angle may reference a `linearGradient` for its `fill`
attribute.  In this case, the `rect`'s cascaded values will contain
things like its fill opacity, or its stroke width and color.  However,
the `linearGradient` has cascaded values that come from its own place
in the element tree, not from the `rect` that references it (multiple
objects may reference the same gradient; in each case, the gradient
has its own cascade derived only from its ancestors).

In such cases, the code that needs to resolve the referenced node's
CSS properties needs to do this:

* Create a temporary `state` with `rsvg_state_new()`, or grab the
  temporary `draw_ctx.get_state()`.
  
* Call `state::reconstruct(state, node)`.  This will walk the tree
  from the root directly down to the node, reconstructing the CSS
  cascade state for *that* node.
  
This is a rather ugly special case for elements that are referenced
outside the "normal" recursion used for rendering.  We hope to move to
a model where all CSS properties are cascaded first, then bounding
boxes are propagated, and finally all rendering can happen in a single
pass in a fully-resolved tree.

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
