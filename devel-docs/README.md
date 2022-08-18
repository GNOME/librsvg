# Development documentation for librsvg

Two things here: design documents for developers of librsvg itself,
and a roadmap for development.

Before embarking on big changes, please write a little design document
modeled on the following ones, and submit a merge request.  We can
then discuss it before coding.  This way we will have a sort of
big-picture development history apart from commit messages.

Design documents:

* [`adding-a-property.md`](adding-a-property.md) - Tutorial on how to
  add support for a new CSS property.  Should remain always current.
  
* [`text-layout.md`](text-layout.md) - Status of the text layout
  engine as of librsvg 2.52.3, and a roadmap for improvement.  Still
  current as of 2021/Dec/17.

# Roadmap - an ever-changing list of development priorities - check this often

## Short term

* Merge [the Rust port of the gdk-pixbuf-loader](https://gitlab.gnome.org/GNOME/librsvg/-/merge_requests/722).

* Update the CI pipelines for the main and stable branches; a design document is upcoming.

* #778 is not the common case, but worrysome.

* Continue with the [revamp of the text engine](text-layout.md).

## Medium term

* Switch to meson, for real now.

* After that, #552 - Build the C library with cargo-c.

* [#459](https://gitlab.gnome.org/GNOME/librsvg/-/issues/459) - Support CSS var() for custom colors and other SVG properties

* [#843](https://gitlab.gnome.org/GNOME/librsvg/-/issues/843) - Support CSS calc()
