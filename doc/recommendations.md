Title: Recommendations for Applications

# Recommendations for Applications

## How sizing works in SVG

SVG documents are *scalable*. The conventional way to position SVG
documents, which comes from the web platform, is to consider a
*viewport* in which to place the SVG document — that is, a rectangular
region to where the SVG will be scaled and positioned.

SVG renderers are supposed to use the viewport provided by the
application, plus the SVG document's `width`, `height`, and `viewBox`
attributes, to compute the position and size for the rendered document.

Ideally, the toplevel `<svg>` element of an SVG document will contain
`width` and `height` attributes, that indicate the proportions and
"natural size" of the document. When those attributes are present, the
SVG renderer can unambiguously figure out the natural aspect ratio of
the document, and can also suggest a natural size for the document. For
example, `<svg width="100px" height="50px">` has a natural size of
100×50 pixels, but it could also be rendered scaled at 200×100 pixels.
Since SVGs are scalable, it is not mandatory to actually use its natural
size; it can be scaled arbitrarily. Of course, it is up to each
application how an SVG document will be scaled: a web browser would want
to consider the semantics of embedding images in HTML, which may be
different from a GUI toolkit loading SVG assets with hard-coded sizes.

If an SVG document's toplevel `<svg>` element does not have `width` and
`height` attributes, then the SVG renderer can try to figure out the
document's aspect ratio from the `viewBox` attribute. For example,
there is no natural size for `<svg viewBox="0 0 100 50">`, but it has a
2:1 (or 100:50) aspect ratio, so it is twice as wide as it is tall.

If there is no `viewBox` either, then the SVG renderer cannot easily
figure out the natural size of the document. It can either set a 1:1
scaling matrix within the application's viewport and render the SVG
there, or it can actually try to compute the size of each object in the
SVG document to figure out the size. The latter is a moderately
expensive operation, and can be avoided by having the SVG document
specify `width` and `height` attributes. Try not to have SVG documents
that just start with `<svg>` without any of those attributes.

### How librsvg computes document sizes

Librsvg looks for the `width` and `height` attributes in the toplevel
`<svg>` element. If they are present, librsvg uses them for the
"natural" size of the SVG, and this also defines the aspect ratio. The
size has actual units (pixels, centimeters, etc.) depending on the value
of the `width` and `height` attributes.

If there are no `width` or `height` attributes in the toplevel `<svg>`,
librsvg looks for the `viewBox` attribute. If present, this defines the
aspect ratio and a "natural" size in pixels.

In both cases above (with `width`/`height` and/or `viewBox`), librsvg
can determine the "natural" size and aspect ratio of an SVG document
immediately after loading.

Otherwise, if none of those attributes are present in the toplevel
`<svg>` element, librsvg must actually compute the coverage of all the
graphical elements in the SVG. This is a moderately expensive operation,
and depends on the complexity of the document.

## Recommendations for applications with SVG assets

Before librsvg 2.46, applications would normally load an SVG asset, then
they would query librsvg for the SVG's size, and then they would
compute the dimensions of their user interface based on the SVG's size.

With librsvg 2.46 and later, applications may have an easier time by
letting the UI choose whatever size it wants, or by hardcoding a size
for SVG assets, and then asking librsvg to render SVG assets at that
particular size. Applications can use [method@Rsvg.Handle.render_document],
which takes a destination viewport, to do this in a single step.

To extract individual elements from an SVG document and render them in
arbitrary locations — for example, to extract a single icon from a
document full of icons —, applications can use
[method@Rsvg.Handle.render_element].

### Injecting a user stylesheet

It is sometimes convenient for applications to inject an extra
stylesheet while rendering an SVG document. You can do this with
[method@Rsvg.Handle.set_stylesheet]. During the CSS cascade, the specified
stylesheet will be used with a ["User"
origin](https://drafts.csswg.org/css-cascade-3/#cascading-origins).
