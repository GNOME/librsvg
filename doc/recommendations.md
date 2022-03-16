Title: Recommendations for Applications

# Recommendations for Applications

Let's consider two common cases for rendering SVG documents:

* Your application uses fixed-size assets, for example, "all icons at
  16×16 pixels".

* Your application needs to accept arbitrarily-sized SVG documents, to
  either render them at a fixed size, or to render them at a "natural"
  size.

In either case, librsvg assumes that for rendering you have already
obtained a Cairo surface, and a Cairo context to draw on the surface.
For the case of fixed-size assets, this is easy; you create a surface
of the size you know you want, and tell librsvg to render to it at
that exact size.  For the case of wanting to use a "natural" size, you
first have to ask librsvg about the document's size so you can create
an appropriately-sized surface.  Let's see how to do both cases.

## Rendering SVG assets at a fixed size

This is the case when you have a known `WIDTH` and `HEIGHT`, and you
want to tell librsvg to render an SVG at that size:

```c
GError *error = NULL;
GFile *file = g_file_new_for_path ("hello.svg");
RsvgHandle *handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);    /* 1 */

if (!handle)
  {
    g_error ("could not load: %s", error->message);
  }

cairo_surface_t *surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, WIDTH, HEIGHT);           /* 2 */
cairo_t *cr = cairo_create (surface);

/* Render the handle scaled proportionally into that whole surface */

RsvgRectangle viewport = {                                                                            /* 3 */
  .x = 0.0,
  .y = 0.0,
  .width = WIDTH,
  .height = HEIGHT,
};

if (!rsvg_handle_render_document (handle, cr, &viewport, &error))                                     /* 4 */
  {
    g_error ("could not render: %s", error->message);
  }

/* The surface is now rendered */
```

1. Load the SVG document.

2. Create an image surface of the size you want.

3. Declare a viewport of that size.  If you want a non-zero `(x, y)`
   offset you can set it right there.

4. Render the document within that viewport.  Done!

This will scale the SVG document proportionally to make it fit in
`WIDTH×HEIGHT` pixels.

## Picking a "natural" size for SVG documents

In some cases, your application may not want to use a predefined size,
but instead query the SVG for a "natural" size at which to render.  Some
SVG documents make this easy, and some don't.

### SVG documents with intrinsic dimensions in absolute units

Consider an SVG document that starts like this:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">
```

This is completely unambiguous; the SVG says that its intrinsic size
is 200×100 pixels.  It can be scaled arbitrarily, but of course most
of the time you'll want to scale it proportionally at that 2:1 ratio.

Here is a slightly more complicated case:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="10cm" height="5cm">
```

The SVG says that its intrinsic size is 10cm × 5cm.  This is not hard
to convert to pixels if you know the Dots-Per-Inch (DPI) at which you
want to render things, but it is a small inconvenience.

And here is a more complicated case still:

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="10em" height="5em">
  <style>
    * { font-size: 2cm; }
  </style>
```

This means that the width is 10 times the font size of 2cm, and the
height is 5 times the font size.

In general an application cannot figure this out easily, since it
would need a CSS parser and cascading engine to even be able to know
what the font size is for the toplevel `<svg>`.  Fortunately, librsvg
already does that!

In all those cases, the width and height are in physical units (px,
cm, mm, etc.), or font-based units (em, ex) that can be resolved to
pixels.  You can use [method@Rsvg.Handle.get_intrinsic_size_in_pixels]
to do the conversion easily if all you want to do is to create a
surface with the "natural" number of pixels:

```c
gboolean rsvg_handle_get_intrinsic_size_in_pixels (RsvgHandle *handle,
                                                   gdouble    *out_width,
                                                   gdouble    *out_height);
```

However, the documentation for [that
function](method.Handle.get_intrinsic_size_in_pixels.html) indicates
that it may return `FALSE` in some cases.  Let's see what those ugly
cases are.

### SVG documents with just proportions

What if we have this:

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
```

This document has no intrinsic dimensions, but it clearly states that
its *proportions* are 200:100, or 2:1.  It will look good when scaled
to a rectangle that is twice as wide as it is tall.  Our old friend
[method@Rsvg.Handle.render_document] will still scale the SVG
proportionally to fit the viewport size you pass in.  But what if you
must pick a size yourself, instead of having a predefined viewport?

In that case you bite the bullet, call
[method@Rsvg.Handle.get_intrinsic_dimensions] and make your own choice
about what to do with the proportions that come in the `viewBox`:

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

You'll cleverly note that I have not answered your question.  You have
an SVG with only a `viewBox`, and you want to pick a reasonable size
to render it.

And here is where I want to say, SVG documents are **scalable**.  Pick
a size, any size for a viewport!  Here are some suggestions:

* The size of your window's visible area.

* The size of your device's screen.

* The size of your sheet of paper, minus the margins.

Take that size, pass it as the viewport size to
[method@Rsvg.Handle.render_document], and be done with it.

### SVG documents without any usable sizing information at all

This is pretty nasty, but *possibly* not useless for doing special
effects in web browsers:

```xml
<svg xmlns="http://www.w3.org/2000/svg">
```

That's right, no `width`, no `height`, no `viewBox`.  There is no easy
way to figure out a suitable size for this.  You have two options:

* Shrug your shoulders, and [method@Rsvg.Handle.render_document] with
  a comfortable viewport size like in the last section.

* Do a best-effort job of actually computing the geometries of all the
  elements in the document.  You can use
  [method@Rsvg.Handle.get_geometry_for_element] by passing `NULL` for
  the target element's `id`; this will measure all the elements in the
  document.  This is not expensive for typical SVGs, but it is not
  "almost instantaneous" like just asking for intrinsic dimensions
  would be.

If this is starting to sound too complicated, please remember that
**SVG documents are scalable**.  That's their whole reason for being!
Pick a size for a viewport, and ask librsvg to render the document
within that viewport with [method@Rsvg.Handle.render_document].

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
