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

#define TEST_LIST_FILENAME  TEST_DATA_DIR"/rsvg-test.txt"

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

static void
rsvg_cairo_check (gconstpointer data)
{
    char const *test_name = data;
    RsvgHandle *rsvg;
    RsvgDimensionData dimensions;
    cairo_t *cr;
    cairo_surface_t *surface_a, *surface_b, *surface_diff;
    buffer_diff_result_t result;
    char *png_filename;
    char *svg_filename;
    char *reference_png_filename;
    char *difference_png_filename;
    unsigned int width_a, height_a, stride_a;
    unsigned int width_b, height_b, stride_b;

    png_filename = g_strdup_printf ("%s-out.png", test_name);
    svg_filename = g_strdup_printf ("%s.svg", test_name);
    reference_png_filename = g_strdup_printf ("%s-ref.png", test_name);
    difference_png_filename = g_strdup_printf ("%s-diff.png", test_name);

    rsvg = rsvg_handle_new_from_file (svg_filename, NULL);
    g_assert (rsvg != NULL);

    rsvg_handle_get_dimensions (rsvg, &dimensions);
    g_assert (dimensions.width > 0);
    g_assert (dimensions.height > 0);
    surface_a = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
					    dimensions.width, dimensions.height);
    cr = cairo_create (surface_a);
    rsvg_handle_render_cairo (rsvg, cr);
    cairo_surface_write_to_png (surface_a, png_filename);

    surface_b = cairo_image_surface_create_from_png (reference_png_filename);
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
	    cairo_surface_write_to_png (surface_diff, difference_png_filename);
	}

	cairo_surface_destroy (surface_diff);
    }

    cairo_surface_destroy (surface_a);
    cairo_surface_destroy (surface_b);
    cairo_destroy (cr);

    g_object_unref (rsvg);

    g_free (svg_filename);
    g_free (reference_png_filename);
}

static void
rsvg_cairo_check_xfail (gconstpointer filename)
{
    g_test_incomplete ("Test is expected to fail.");
    rsvg_cairo_check (filename);
}

int
main (int argc, char **argv)
{
    char *list_content;
    char **list_lines, **strings;
    char *test_name;
    gboolean xfail, ignore;
    int i, j;
    gsize length;
    int result;

    RSVG_G_TYPE_INIT;
    g_test_init (&argc, &argv, NULL);

    if (g_file_get_contents (TEST_LIST_FILENAME, &list_content, &length, NULL)) {
	rsvg_set_default_dpi_x_y (72, 72);

	list_lines = g_strsplit (list_content, "\n", 0);
	for (i = 0; list_lines[i] != NULL; i++) {
	    strings = g_strsplit_set (list_lines[i], " \t", 0);
	    test_name = strings[0];
	    if (test_name != NULL 
		&& strlen (test_name) > 0 
		&& test_name[0] != '#') {

		xfail = FALSE;
		ignore = FALSE;
		for (j = 1; strings[j] != NULL; j++) {
		    if (strcmp (strings[j], "X") == 0)
			xfail = TRUE;
		    else if (strcmp (strings[j], "I") == 0)
			ignore = TRUE;
		}
		if (!ignore)
                {
		    char * test_filename, * test_testname;

                    test_testname = g_strconcat ("/rsvg/reftest/", test_name, NULL);
		    test_filename = g_build_filename (TEST_DATA_DIR, test_name, NULL);

                    if (xfail)
                      g_test_add_data_func_full (test_testname, test_filename, rsvg_cairo_check_xfail, g_free);
                    else
                      g_test_add_data_func_full (test_testname, test_filename, rsvg_cairo_check, g_free);

                    g_free (test_testname);
                }
	    }
	    g_strfreev (strings);
	}
	g_strfreev (list_lines);
	g_free (list_content);
    } else {
	g_test_message ("Error opening test list file "TEST_LIST_FILENAME"\n");
        g_assert_not_reached ();
    }

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}

