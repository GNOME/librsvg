Title: Migrating from old APIs

# Migrating from old APIs

## Migrating from the deprecated API that does not use viewports

First, some context. Until librsvg version 2.44, the only way to
render an [class@Rsvg.Handle] into a Cairo context was with the
functions [`rsvg_handle_render_cairo(handle,
cairo_t)`](method.Handle.render_cairo.html) and
[`rsvg_handle_render_cairo_sub(handle, cairo_t,
id)`](method.Handle.render_cairo_sub.html) — respectively, to
render the whole document, and to render a single \"layer\" from it.
Both functions assumed that the SVG document was to be rendered at its
\"natural size\", or to the size overriden with
[method@Rsvg.Handle.set_size_callback]. Since the Cairo context can already
have an affine transform applied to it, that transform can further
change the size of the rendered image.

Librsvg 2.46 introduced the following functions, designed to replace the
`render_cairo` ones:

* [method@Rsvg.Handle.render_document] - renders the whole document
* [method@Rsvg.Handle.render_layer] - renders a single layer
* [method@Rsvg.Handle.render_element] - renders a single element
* Plus corresponding functions to get the geometries of the document/layer/element.

All of those functions take a viewport argument. Let\'s see what this
means. But first, some history.

### Historical note: before librsvg supported viewports

When librsvg was first written, its API basically consisted of only
functions to load an [class@Rsvg.Handle], plus [method@Rsvg.Handle.get_pixbuf] to
render it directly to a GdkPixbuf image. Internally the library used
libart (a pre-Cairo 2D rendering library), but did not expose it in the
public API.

The only way to specify a size at which to render an [class@Rsvg.Handle] was with
[method@Rsvg.Handle.set_size_callback], and the callback would run at an
unspecified time during *loading*: when just enough of the SVG document
had been loaded to read in the `width/height` attributes of the toplevel
`<svg>` element, the callback would let the application override these
values with its own desired size.

Some years later, Cairo was introduced, and it started to replace
libart. Unlike libart, which could only render to in-memory RGBA
buffers, Cairo had a notion of \"backends\": it could render to RGBA
buffers, or it could translate its drawing model commands into PDF or
PostScript. In Cairo\'s terms, one creates a [`cairo_surface_t`][cairo_surface_t] of a
particular kind (in-memory image surface, PDF surface, EPS surface,
etc.), and then a [`cairo_t`][cairo_t] context for the surface. The context is what
makes the drawing commands available.

Being able to render SVG documents directly to PDF or PostScript was
clearly attractive, so librsvg\'s API of [method@Rsvg.Handle.get_pixbuf]
would clearly not be enough. It would be better to pass a [`cairo_t`][cairo_t] for
an already-created surface, and have librsvg issue its drawing commands
to it. Then the application would be in control of the surface type, or
in the case of GTK widgets, they would already get a [`cairo_t`][cairo_t] passed to
their drawing functions. Librsvg got modified to export a
[`rsvg_handle_render_cairo(handle, cairo_t)`](method.Handle.render_cairo.html), and then it reimplemented the old
[method@Rsvg.Handle.get_pixbuf] in terms of Cairo.

At this point, librsvg still kept the notion of rendering SVG documents
at their \"natural size\": the `<svg>` element\'s `width` and `height`
attributes converted to pixels (e.g. converting from `width="5cm"` by
using the dots-per-inch value from the [class@Rsvg.Handle]), or if those
attributes don\'t exist, by using the `viewBox` as a pixel size. The
assumption was that if you needed a different size, you could always
start by setting the transformation matrix on your [`cairo_t`][cairo_t] and then
rendering to that.

[cairo_surface_t]: https://www.cairographics.org/manual/cairo-cairo-surface-t.html
[cairo_t]: https://www.cairographics.org/manual/cairo-cairo-t.html

### The problem with not having viewports

Most applications which use librsvg to render SVG assets for their user
interface generally work in the same way. For example, to take an SVG
icon and render it, they do something like this:

1.  Create an [class@Rsvg.Handle] by loading it from the SVG icon data.

2.  Ask the [class@Rsvg.Handle] for its dimensions.

3.  Divide the dimensions by the GUI\'s preferred size for icons.

4.  Translate a Cairo context so the icon will appear at the desired
    location.  Scale the Cairo context by the result of the previous
    step to obtain the desired dimensions.

5.  Render the [class@Rsvg.Handle] in that Cairo context.

This is\... too much work. The web world has moved on to using the CSS
box model practically everywhere. To embed an image you specify *where*
and at *what size* you want to place it, and it gets done automatically.
You actually have to do extra work if you want to do non-standard things
like scale an image non-proportionally.

### The new rendering API that uses viewports

Starting with librsvg 2.46, the following functions are available:

```c
typedef struct {
    double x;
    double y;
    double width;
    double height;
} RsvgRectangle;

gboolean rsvg_handle_render_document (RsvgHandle           *handle,
                                      cairo_t              *cr,
                                      const RsvgRectangle  *viewport,
                                      GError              **error);

gboolean rsvg_handle_render_layer    (RsvgHandle           *handle,
                                      cairo_t              *cr,
                                      const char           *id,
                                      const RsvgRectangle  *viewport,
                                      GError              **error);

gboolean rsvg_handle_render_element  (RsvgHandle           *handle,
                                      cairo_t              *cr,
                                      const char           *id,
                                      const RsvgRectangle  *element_viewport,
                                      GError              **error);
```
          

For brevity we will omit the `rsvg_handle` namespace prefix, and just talk about the actual
function names. You can see that [`render_document`](method.Handle.render_document.html)
is basically the same as [`render_cairo`](method.Handle.render_cairo.html), but it has an
extra `viewport` argument. The same occurs in
[`render_layer`](method.Handle.render_layer.html) versus
[`render_cairo_sub`](method.Handle.render_cairo_sub.html).

In both of those cases — [`render_document`](method.Handle.render_document.html) and
[`render_layer`](method.Handle.render_layer.html) —, the `viewport` argument specifies a
rectangle into which the SVG will be positioned and scaled to fit. Consider something like
this:

```c
RsvgRectangle viewport = {
    .x = 10.0,
    .y = 20.0,
    .width = 640.0,
    .height = 480.0,
};

rsvg_handle_render_document (handle, cr, &viewport, NULL);
```
          

This is equivalent to first figuring out the scaling factor to make the SVG fit
proportionally in 640×480 pixels, then translating the `cr` by (10, 20) pixels, and then
calling [method@Rsvg.Handle.render_cairo]. If the SVG has different proportions than the
width and height of the rectangle, it will be rendered and centered to fit the rectangle.

---
**Note:** [method@Rsvg.Handle.render_element] is new in librsvg 2.46. It extracts a
single element from the SVG and renders it scaled to the viewport you
specify. It is different from `render_layer` (or the old-style
`render_cairo_sub`) in that those ones act as if they rendered the whole
document\'s area, but they only paint the layer you specify.
---

Even better: the old functions to get an SVG\'s natural dimensions, like
[method@Rsvg.Handle.get_dimensions], returned integers instead of
floating-point numbers, so you could not always get an exact fit. Please
use the new `get_geometry` functions that take viewports; they will give you easier and
better results:

* [method@Rsvg.Handle.get_geometry_for_layer]
* [method@Rsvg.Handle.get_geometry_for_element]

### New API for obtaining an SVG's dimensions

Per the previous section, you should seldom need to obtain the \"natural
size\" of an SVG document now that you can render it directly into a
viewport. But if you still need to know what the SVG document specifies
for its own size, you can use the following functions, depending on the
level of detail you require:

```c
gboolean rsvg_handle_get_intrinsic_size_in_pixels (RsvgHandle *handle,
                                                   gdouble    *out_width,
                                                   gdouble    *out_height);
```

[method@Rsvg.Handle.get_intrinsic_size_in_pixels] returns an exact width and height in
floating-point pixels. **You should round up to the next integer** if you need to allocate
a pixel buffer big enough, to avoid clipping the last column or row of pixels, which may
be only partially covered.  For example, if a document's width is `41.3` CSS pixels, you
should create a raster image `42` pixels wide so it fits without clipping the last pixel.
You can do this with the `ceil()` function.

[method@Rsvg.Handle.get_intrinsic_size_in_pixels] works by resolving the
`width/height` attributes of the toplevel `<svg>` element against the
handle\'s current DPI and the `font-size` that is defined for the
`<svg>` element.

However, that is only possible if the `width/height` attributes actually
exist and are in physical units. The function will return FALSE if the
SVG has no resolvable units, for example if the `width/height`
attributes are specified in percentages (e.g. `width="50%"`), since the
function has no knowledge of the viewport where you will place the SVG,
or if those attributes are not specified.

The other way of obtaining an SVG\'s dimensions is to actually query its
\"intrinsic dimensions\", i.e. what is actually specified in the SVG
document:

```c
typedef enum {
    RSVG_UNIT_PERCENT,
    RSVG_UNIT_PX,
    RSVG_UNIT_EM,
    RSVG_UNIT_EX,
    RSVG_UNIT_IN,
    RSVG_UNIT_CM,
    RSVG_UNIT_MM,
    RSVG_UNIT_PT,
    RSVG_UNIT_PC
} RsvgUnit;

typedef struct {
    double   length;
    RsvgUnit unit;
} RsvgLength;

void rsvg_handle_get_intrinsic_dimensions (RsvgHandle *handle,
                                           gboolean   *out_has_width,
                                           RsvgLength *out_width,
                                           gboolean   *out_has_height,
                                           RsvgLength *out_height,
                                           gboolean   *out_has_viewbox,
                                           RsvgRectangle *out_viewbox);
```
          

[method@Rsvg.Handle.get_intrinsic_dimensions] will tell you precisely if the toplevel
`<svg>` has `width/height` attributes and their values, and also whether it has a
`viewBox` and its value.

---
**Note:** Remember that SVGs are *scalable*. They are not like raster images which have an
exact size in pixels, and which you must always take into account to scale them to a
convenient size. For SVGs, you can just render them to a viewport, and avoid working
directly with their size — which is kind of arbitrary, and all that matters is the
document's aspect ratio.
---

### SVGs with no intrinsic dimensions nor aspect ratio

SVG documents that have none of the `width`, `height`, or `viewBox` attributes are
thankfully not very common, but they are hard to deal with: the software cannot
immediately know their natural size or aspect ratio, so they cannot be easily scaled to
fit within a viewport.  If you need to measure the extents of all the objects in an SVG
document, you can use [method@Rsvg.Handle.get_geometry_for_element] by passing `NULL` for
the target element's `id`; this will measure all the elements in the document.

## Migrating to the geometry APIs

Until librsvg 2.44, the available APIs to query the geometry of a layer
or element were these:

```c
struct _RsvgPositionData {
    int x;
    int y;
};

gboolean rsvg_handle_get_position_sub (RsvgHandle       *handle,
                                       RsvgPositionData *position_data,
                                       const char       *id);

struct _RsvgDimensionData {
    int width;
    int height;
    gdouble em;
    gdouble ex;
};

gboolean rsvg_handle_get_dimensions_sub (RsvgHandle        *handle,
                                         RsvgDimensionData *dimension_data,
                                         const char        *id);
```
        
These functions are inconvenient — separate calls to get the position
and dimensions —, and also inexact, since they only return integer
values, while SVG uses floating-point units.

Since librsvg 2.46, you can use these functions instead:

```c
typedef struct {
    double x;
    double y;
    double width;
    double height;
} RsvgRectangle;

gboolean rsvg_handle_get_geometry_for_layer (RsvgHandle           *handle,
                                             const char           *id,
                                             const RsvgRectangle  *viewport,
                                             RsvgRectangle        *out_ink_rect,
                                             RsvgRectangle        *out_logical_rect,
                                             GError              **error);

gboolean rsvg_handle_get_geometry_for_element (RsvgHandle     *handle,
                                               const char     *id,
                                               RsvgRectangle  *out_ink_rect,
                                               RsvgRectangle  *out_logical_rect,
                                               GError        **error);
```

These functions return exact floating-point values. They also give you
the ink rectangle, or area covered by paint, as well as the logical
rectangle, which is the extents of unstroked paths (i.e. just the
outlines).

* [method@Rsvg.Handle.get_geometry_for_layer]
* [method@Rsvg.Handle.get_geometry_for_element]
