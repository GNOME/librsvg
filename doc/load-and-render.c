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
