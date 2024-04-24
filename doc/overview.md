Title: Overview of Librsvg

# Overview of Librsvg

Librsvg is a library for rendering Scalable Vector Graphics files (SVG).
Specifically, it can take non-animated, non-scripted SVG documents and
render them into a [Cairo](https://www.cairographics.org/) surface.
Normally this means an in-memory raster surface, but it could also be
any of the other surface types that Cairo supports.

Librsvg supports many of the graphic features in the [SVG
1.1](https://www.w3.org/TR/SVG/) and [SVG2](https://www.w3.org/TR/SVG2/)
specifications. The main features of SVG that librsvg does not support
are the following:

* Scripting or animation: Librsvg reads SVG data and renders it to a
  static image. There is no provision to execute scripts that may
  control animation parameters.

* Access to the DOM: Librsvg creates an internal representation of
  the SVG data, but it does not provide outside access to the
  resulting Document Object Model (DOM).

* SVG fonts: Instead, librsvg relies on the system's fonts,
  particularly those that are available through Cairo/Pango.

Librsvg's API is divided into two main parts: one for loading SVG data
and one for rendering it. In the *loading stage*, you create an
[class@Rsvg.Handle] object from SVG data, which can come from a file or from a
stream of bytes. In the *rendering stage*, you take an [class@Rsvg.Handle] and
ask it to render itself to a Cairo context.

## Loading

[class@Rsvg.Handle] is an object that represents SVG data in memory. Your program
creates an [class@Rsvg.Handle] from an SVG file, or from a memory buffer that
contains SVG data, or in the most general form, from a [class@Gio.InputStream] that
will provide SVG data.  At this stage you can get either I/O errors or
parsing errors. If loading completes successfully, the [class@Rsvg.Handle] will
be ready for rendering.

Generally you should use [ctor@Rsvg.Handle.new_from_gfile_sync] or
[ctor@Rsvg.Handle.new_from_stream_sync] to load an SVG document into
an [class@Rsvg.Handle]. There are other convenience functions to load
an SVG document, but these two functions let one set the "[base
file](class.Handle.html#the-base-file-and-resolving-references-to-external-files)"
and the [flags@Rsvg.HandleFlags] in a single call.

## Rendering

Once you have an SVG image loaded into an [class@Rsvg.Handle], you can render it
to a Cairo context any number of times, or to different Cairo contexts,
as needed. As a convenience, you can pick a single element in the SVG by
its `id` attribute and render only that element; this is so that
sub-elements can be extracted conveniently out of a larger SVG.

Generally you should use [method@Rsvg.Handle.render_document] to render the
whole SVG document at any size you choose into a Cairo context.

## Example: simple loading and rendering

The following program loads `hello.svg`, renders it scaled to fit within
640×480 pixels, and writes a `hello.png` file.

Note the following:

* [method@Rsvg.Handle.render_document] will scale the document
   proportionally to fit the viewport you specify, and it will center
   the image within that viewport.

* Librsvg does not paint a background color by default, so in the
  following example all unfilled areas of the SVG will appear as fully
  transparent. If you wish to have a specific background, fill the
  viewport area yourself before rendering the SVG.

```c
/* gcc -Wall -g -O2 -o load-and-render load-and-render.c `pkg-config --cflags --libs rsvg-2.0` */

#include <stdlib.h>
#include <librsvg/rsvg.h>

#define WIDTH 640
#define HEIGHT 480

int
main (void)
{
  /* First, load an SVG document into an RsvgHandle */

  GError *error = NULL;
  GFile *file = g_file_new_for_path ("hello.svg");
  RsvgHandle *handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);

  if (!handle)
    {
      g_printerr ("could not load: %s", error->message);
      exit (1);
    }

  /* Create a Cairo image surface and a rendering context for it */

  cairo_surface_t *surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, WIDTH, HEIGHT);
  cairo_t *cr = cairo_create (surface);

  /* Set the dots-per-inch */

  rsvg_handle_set_dpi (handle, 96.0);

  /* Render the handle scaled proportionally into that whole surface */

  RsvgRectangle viewport = {
    .x = 0.0,
    .y = 0.0,
    .width = WIDTH,
    .height = HEIGHT,
  };

  if (!rsvg_handle_render_document (handle, cr, &viewport, &error))
    {
      g_printerr ("could not render: %s", error->message);
      exit (1);
    }

  /* Write a PNG file */

  if (cairo_surface_write_to_png (surface, "hello.png") != CAIRO_STATUS_SUCCESS)
    {
      g_printerr ("could not write output file");
      exit (1);
    }

  /* Free our memory and we are done! */

  cairo_destroy (cr);
  cairo_surface_destroy (surface);
  g_object_unref (handle);
  g_object_unref (file);
  return 0;
}
```
