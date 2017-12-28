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

