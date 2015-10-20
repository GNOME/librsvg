/* vim: set sw=4 sts=4: -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 8 -*-
 *
 * rsvg-test - Regression test utility for librsvg
 *
 * Copyright © 2004 Richard D. Worth
 * Copyright © 2006 Red Hat, Inc.
 * Copyright © 2007 Emmanuel Pacaud
 *
 * Permission to use, copy, modify, distribute, and sell this software
 * and its documentation for any purpose is hereby granted without
 * fee, provided that the above copyright notice appear in all copies
 * and that both that copyright notice and this permission notice
 * appear in supporting documentation, and that the name of the authors
 * not be used in advertising or publicity pertaining to distribution
 * of the software without specific, written prior permission.
 * The authors make no representations about the suitability of this
 * software for any purpose.  It is provided "as is" without express
 * or implied warranty.
 *
 * THE AUTHORS DISCLAIM ALL WARRANTIES WITH REGARD TO THIS SOFTWARE,
 * INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS, IN
 * NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY SPECIAL, INDIRECT OR
 * CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
 * OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT,
 * NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
 * CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 * Authors: Emmanuel Pacaud <emmanuel.pacaud@lapp.in2p3.fr>
 *	    Richard D. Worth <richard@theworths.org>
 *	    Carl Worth <cworth@cworth.org>
 */

#include "config.h"

#include <stdlib.h>
#include <string.h>

#include "rsvg.h"
#include "rsvg-compat.h"

#include "test-utils.h"

typedef struct _buffer_diff_result {
    unsigned int pixels_changed;
    unsigned int max_diff;
} buffer_diff_result_t;

/* Compare two buffers, returning the number of pixels that are
 * different and the maximum difference of any single color channel in
 * result_ret.
 *
 * This function should be rewritten to compare all formats supported by
 * cairo_format_t instead of taking a mask as a parameter.
 */
static void
buffer_diff_core (unsigned char *_buf_a,
		  unsigned char *_buf_b,
		  unsigned char *_buf_diff,
		  int		width,
		  int		height,
		  int		stride,
		  guint32       mask,
		  buffer_diff_result_t *result_ret)
{
    int x, y;
    guint32 *row_a, *row_b, *row;
    buffer_diff_result_t result = {0, 0};
    guint32 *buf_a = (guint32 *) _buf_a;
    guint32 *buf_b = (guint32 *) _buf_b;
    guint32 *buf_diff = (guint32 *) _buf_diff;

    stride /= sizeof(guint32);
    for (y = 0; y < height; y++)
    {
	row_a = buf_a + y * stride;
	row_b = buf_b + y * stride;
	row = buf_diff + y * stride;
	for (x = 0; x < width; x++)
	{
	    /* check if the pixels are the same */
	    if ((row_a[x] & mask) != (row_b[x] & mask)) {
		int channel;
		guint32 diff_pixel = 0;

		/* calculate a difference value for all 4 channels */
		for (channel = 0; channel < 4; channel++) {
		    int value_a = (row_a[x] >> (channel*8)) & 0xff;
		    int value_b = (row_b[x] >> (channel*8)) & 0xff;
		    unsigned int diff;
		    diff = abs (value_a - value_b);
		    if (diff > result.max_diff)
			result.max_diff = diff;
		    diff *= 4;  /* emphasize */
		    if (diff)
		        diff += 128; /* make sure it's visible */
		    if (diff > 255)
		        diff = 255;
		    diff_pixel |= diff << (channel*8);
		}

		result.pixels_changed++;
		if ((diff_pixel & 0x00ffffff) == 0) {
		    /* alpha only difference, convert to luminance */
		    guint8 alpha = diff_pixel >> 24;
		    diff_pixel = alpha * 0x010101;
		}
		row[x] = diff_pixel;
	    } else {
		row[x] = 0;
	    }
	    row[x] |= 0xff000000; /* Set ALPHA to 100% (opaque) */
	}
    }

    *result_ret = result;
}

static void
compare_surfaces (cairo_surface_t	*surface_a,
		  cairo_surface_t	*surface_b,
		  cairo_surface_t	*surface_diff,
		  buffer_diff_result_t	*result)
{
    /* Here, we run cairo's old buffer_diff algorithm which looks for
     * pixel-perfect images.
     */
    buffer_diff_core (cairo_image_surface_get_data (surface_a),
		      cairo_image_surface_get_data (surface_b),
		      cairo_image_surface_get_data (surface_diff),
		      cairo_image_surface_get_width (surface_a),
		      cairo_image_surface_get_height (surface_a),
		      cairo_image_surface_get_stride (surface_a),
		      0xffffffff,
		      result);
    if (result->pixels_changed == 0)
	return;

    g_test_message ("%d pixels differ (with maximum difference of %d) from reference image\n",
		    result->pixels_changed, result->max_diff);
}

static char *
get_output_file (const char *test_file,
                 const char *extension)
{
  const char *output_dir = g_get_tmp_dir ();
  char *result, *base;

  base = g_path_get_basename (test_file);

  if (g_str_has_suffix (base, ".svg"))
    base[strlen (base) - strlen (".svg")] = '\0';

  result = g_strconcat (output_dir, G_DIR_SEPARATOR_S, base, extension, NULL);
  g_free (base);

  return result;
}

static void
save_image (cairo_surface_t *surface,
            const char      *test_name,
            const char      *extension)
{
  char *filename = get_output_file (test_name, extension);

  g_test_message ("Storing test result image at %s", filename);
  g_assert (cairo_surface_write_to_png (surface, filename) == CAIRO_STATUS_SUCCESS);

  g_free (filename);
}

static gboolean
is_svg_or_subdir (GFile *file)
{
  char *uri;
  gboolean result;

  if (g_file_query_file_type (file, 0, NULL) == G_FILE_TYPE_DIRECTORY)
    return TRUE;

  uri = g_file_get_uri (file);
  result = g_str_has_suffix (uri, ".svg");
  g_free (uri);

  return result;
}

static cairo_status_t
read_from_stream (void          *stream,
                  unsigned char *data,
                  unsigned int   length)

{
  gssize result;
  GError *error = NULL;

  result = g_input_stream_read (stream, data, length, NULL, &error);
  g_assert_no_error (error);
  g_assert (result == length);

  return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
read_png (const char *test_name)
{
  char *reference_uri;
  GFileInputStream *stream;
  GFile *file;
  GError *error = NULL;
  cairo_surface_t *surface;

  reference_uri = g_strconcat (test_name, "-ref.png", NULL);
  file = g_file_new_for_uri (reference_uri);
  stream = g_file_read (file, NULL, &error);
  g_assert_no_error (error);
  g_assert (stream);

  surface = cairo_image_surface_create_from_png_stream (read_from_stream, stream);

  g_object_unref (stream);
  g_object_unref (file);

  return surface;
}

static void
rsvg_cairo_check (gconstpointer data)
{
    GFile *test_file = G_FILE (data);
    RsvgHandle *rsvg;
    RsvgDimensionData dimensions;
    cairo_t *cr;
    cairo_surface_t *surface_a, *surface_b, *surface_diff;
    buffer_diff_result_t result;
    char *test_file_base;
    unsigned int width_a, height_a, stride_a;
    unsigned int width_b, height_b, stride_b;
    GError *error = NULL;

    test_file_base = g_file_get_uri (test_file);
    if (g_str_has_suffix (test_file_base, ".svg"))
      test_file_base[strlen (test_file_base) - strlen (".svg")] = '\0';

    rsvg = rsvg_handle_new_from_gfile_sync (test_file, 0, NULL, &error);
    g_assert_no_error (error);
    g_assert (rsvg != NULL);

    rsvg_handle_get_dimensions (rsvg, &dimensions);
    g_assert (dimensions.width > 0);
    g_assert (dimensions.height > 0);
    surface_a = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
					    dimensions.width, dimensions.height);
    cr = cairo_create (surface_a);
    rsvg_handle_render_cairo (rsvg, cr);
    save_image (surface_a, test_file_base, "-out.png");

    surface_b = read_png (test_file_base);
    width_a = cairo_image_surface_get_width (surface_a);
    height_a = cairo_image_surface_get_height (surface_a);
    stride_a = cairo_image_surface_get_stride (surface_a);
    width_b = cairo_image_surface_get_width (surface_b);
    height_b = cairo_image_surface_get_height (surface_b);
    stride_b = cairo_image_surface_get_stride (surface_b);

    if (width_a  != width_b  ||
	height_a != height_b ||
	stride_a != stride_b) {
        g_test_fail ();
        g_test_message ("Image size mismatch (%dx%d != %dx%d)\n",
                        width_a, height_a, width_b, height_b); 
    }
    else {
	surface_diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
						   dimensions.width, dimensions.height);

	compare_surfaces (surface_a, surface_b, surface_diff, &result);

	if (result.pixels_changed && result.max_diff > 1) {
            g_test_fail ();
            save_image (surface_diff, test_file_base, "-diff.png");
	}

	cairo_surface_destroy (surface_diff);
    }

    cairo_surface_destroy (surface_a);
    cairo_surface_destroy (surface_b);
    cairo_destroy (cr);

    g_object_unref (rsvg);
}

int
main (int argc, char **argv)
{
    int result;

    RSVG_G_TYPE_INIT;
    g_test_init (&argc, &argv, NULL);

    rsvg_set_default_dpi_x_y (72, 72);

    if (argc < 2) {
        GFile *base, *tests;

        base = g_file_new_for_path (test_utils_get_test_data_path ());
        tests = g_file_get_child (base, "reftests");
        test_utils_add_test_for_all_files ("/rsvg/reftest", tests, tests, rsvg_cairo_check, is_svg_or_subdir);
        g_object_unref (tests);
        g_object_unref (base);
    } else {
        guint i;

        for (i = 1; i < argc; i++) {
            GFile *file = g_file_new_for_commandline_arg (argv[i]);

            test_utils_add_test_for_all_files ("/rsvg/reftest", NULL, file, rsvg_cairo_check, is_svg_or_subdir);

            g_object_unref (file);
        }
    }

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}

