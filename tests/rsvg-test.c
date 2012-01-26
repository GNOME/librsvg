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

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif

#include "rsvg.h"
#include "rsvg-cairo.h"
#include "rsvg-private.h"
#include "rsvg-size-callback.h"

#include "pdiff.h"

typedef enum {
    RSVG_TEST_SUCCESS = 0,
    RSVG_TEST_FAILURE,
    RSVG_TEST_CRASHED
} RsvgTestStatus;	

static const char *fail_face = "", *normal_face = "";
FILE *rsvg_test_log_file = NULL;
FILE *rsvg_test_html_file = NULL;

static void
rsvg_test_log (const char *fmt, ...)
{
    va_list va;
    FILE *file = rsvg_test_log_file ? rsvg_test_log_file : stderr;

    va_start (va, fmt);
    vfprintf (file, fmt, va);
    va_end (va);
}

static void
rsvg_test_html (const char *fmt, ...)
{
    va_list va;
    FILE *file = rsvg_test_html_file ? rsvg_test_html_file : stdout;

    va_start (va, fmt);
    vfprintf (file, fmt, va);
    va_end (va);
}

#define TEST_WIDTH 480
#define TEST_LIST_FILENAME  TEST_DATA_DIR"/rsvg-test.txt"
#define TEST_LOG_FILENAME   "rsvg-test.log"
#define HTML_FILENAME	    "rsvg-test.html"

#if   HAVE_STDINT_H
# include <stdint.h>
#elif HAVE_INTTYPES_H
# include <inttypes.h>
#elif HAVE_SYS_INT_TYPES_H
# include <sys/int_types.h>
#elif defined(_MSC_VER)
  typedef __int8 int8_t;
  typedef unsigned __int8 uint8_t;
  typedef __int16 int16_t;
  typedef unsigned __int16 uint16_t;
  typedef __int32 int32_t;
  typedef unsigned __int32 uint32_t;
  typedef __int64 int64_t;
  typedef unsigned __int64 uint64_t;
#else
#error Cannot find definitions for fixed-width integral types (uint8_t, uint32_t, etc.)
#endif

typedef uint32_t pixman_bits_t;

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
		  pixman_bits_t mask,
		  buffer_diff_result_t *result_ret)
{
    int x, y;
    pixman_bits_t *row_a, *row_b, *row;
    buffer_diff_result_t result = {0, 0};
    pixman_bits_t *buf_a = (pixman_bits_t*)_buf_a;
    pixman_bits_t *buf_b = (pixman_bits_t*)_buf_b;
    pixman_bits_t *buf_diff = (pixman_bits_t*)_buf_diff;

    stride /= sizeof(pixman_bits_t);
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
		pixman_bits_t diff_pixel = 0;

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
    /* These default values were taken straight from the
     * perceptualdiff program. We'll probably want to tune these as
     * necessary. */
    double gamma = 2.2;
    double luminance = 100.0;
    double field_of_view = 45.0;
    int discernible_pixels_changed;

    /* First, we run cairo's old buffer_diff algorithm which looks for
     * pixel-perfect images, (we do this first since the test suite
     * runs about 3x slower if we run pdiff_compare first).
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

    rsvg_test_log ("%d pixels differ (with maximum difference of %d) from reference image\n",
		   result->pixels_changed, result->max_diff);

    /* Then, if there are any different pixels, we give the pdiff code
     * a crack at the images. If it decides that there are no visually
     * discernible differences in any pixels, then we accept this
     * result as good enough. */
    discernible_pixels_changed = pdiff_compare (surface_a, surface_b,
						gamma, luminance, field_of_view);
    if (discernible_pixels_changed == 0) {
	result->pixels_changed = 0;
	rsvg_test_log ("But perceptual diff finds no visually discernible difference.\n"
		       "Accepting result.\n");
    }
}

static void
rsvg_cairo_size_callback (int *width, int *height, gpointer data)
{
    RsvgDimensionData *dimensions = data;
    *width = dimensions->width;
    *height = dimensions->height;
}

static RsvgTestStatus
rsvg_cairo_check (char const *test_name, gboolean xfail)
{
    RsvgHandle *rsvg;
    RsvgDimensionData dimensions;
    RsvgTestStatus status = RSVG_TEST_SUCCESS;
    struct RsvgSizeCallbackData size_data;
    cairo_t *cr;
    cairo_surface_t *surface_a, *surface_b, *surface_diff;
    buffer_diff_result_t result;
    char *png_filename;
    char *svg_filename;
    char *reference_png_filename;
    char *difference_png_filename;
    unsigned int width_a, height_a, stride_a;
    unsigned int width_b, height_b, stride_b;

    rsvg_test_log ("%s%s\n",test_name, xfail ? " X" : "");

    png_filename = g_strdup_printf ("%s-out.png", test_name);
    svg_filename = g_strdup_printf ("%s.svg", test_name);
    reference_png_filename = g_strdup_printf ("%s-ref.png", test_name);
    difference_png_filename = g_strdup_printf ("%s-diff.png", test_name);

    rsvg = rsvg_handle_new_from_file (svg_filename, NULL);
    if (rsvg == NULL)
	fprintf (stderr, "Cannot open input file %s\n", svg_filename);

    rsvg_handle_set_size_callback (rsvg, rsvg_cairo_size_callback, &dimensions, NULL);
    rsvg_handle_get_dimensions (rsvg, &dimensions);
    size_data.type = RSVG_SIZE_WH_MAX;
    size_data.height = -1;
    size_data.width = TEST_WIDTH;
    size_data.keep_aspect_ratio = FALSE;
    _rsvg_size_callback (&dimensions.width, &dimensions.height, &size_data);

    surface_a = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
					    dimensions.width, dimensions.height);
    cr = cairo_create (surface_a);
    cairo_set_source_rgb (cr, 1, 1, 1);
    cairo_paint (cr);
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
	if (xfail) {
	    printf ("%s:\tXFAIL\n", test_name);
	    status = RSVG_TEST_SUCCESS;
	} else {
	    status = RSVG_TEST_FAILURE;
	    rsvg_test_log ("Image size mismatch (%dx%d != %dx%d)\n",
			   width_a, height_a, width_b, height_b); 
	    fprintf (stderr, "%s:\t%sFAIL%s\n",
		     test_name, fail_face, normal_face);
	}
    }
    else {
	surface_diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
						   dimensions.width, dimensions.height);

	compare_surfaces (surface_a, surface_b, surface_diff, &result);

	if (result.pixels_changed && result.max_diff > 1) {
	    status = RSVG_TEST_FAILURE;
	    fprintf (stderr, "%s:\t%sFAIL%s\n",
		     test_name, fail_face, normal_face);
	    cairo_surface_write_to_png (surface_diff, difference_png_filename);
	} else {
	    status = RSVG_TEST_SUCCESS;
	    if (xfail)
		fprintf (stderr, "%s:\t%sUNEXPECTD PASS%s\n",
			 test_name, fail_face, normal_face);
	    else
		printf ("%s:\tPASS\n", test_name);
	}

	cairo_surface_destroy (surface_diff);
    }

    cairo_surface_destroy (surface_a);
    cairo_surface_destroy (surface_b);
    cairo_destroy (cr);

    g_object_unref (rsvg);

    if (status == RSVG_TEST_FAILURE) {
	rsvg_test_html ("<tr>");
	rsvg_test_html ("<td><img src=\"%s\"/></td>", png_filename);
	rsvg_test_html ("<td><img src=\"%s\"/></td>", reference_png_filename);
	rsvg_test_html ("<td><img src=\"%s\"/></td>", difference_png_filename);
	rsvg_test_html ("</tr>\n");
    }

    g_free (png_filename);
    g_free (svg_filename);
    g_free (reference_png_filename);
    g_free (difference_png_filename);

    return status;
}

int
main (int argc, char **argv)
{
    RsvgTestStatus status = RSVG_TEST_SUCCESS;
    char *list_content;
    char **list_lines, **strings;
    char *test_name;
    gboolean xfail, ignore;
    int i, j;
    gsize length;

    g_type_init ();

    printf ("===============\n"
	    "Rendering tests\n"
	    "===============\n");

#ifdef HAVE_UNISTD_H
    if (isatty (2)) {
	fail_face = "\033[41m\033[37m\033[1m";
	normal_face = "\033[m";
    }
#endif

    rsvg_test_log_file = fopen (TEST_LOG_FILENAME, "w");
    rsvg_test_html_file = fopen (HTML_FILENAME, "w");

    rsvg_test_html ("<!DOCTYPE html PUBLIC \"-//W3C//DTD HTML 4.01 Transitional//EN\""
		    "\"http://www.w3.org/TR/html4/loose.dtd\"/>\n");
    rsvg_test_html ("<html>\n");
    rsvg_test_html ("<table>\n");
    
    if (g_file_get_contents (TEST_LIST_FILENAME, &list_content, &length, NULL)) {
	rsvg_set_default_dpi_x_y (72, 72);

	list_lines = g_strsplit (list_content, "\n", 0);
	for (i = 0; list_lines[i] != NULL; i++) {
	    strings = g_strsplit_set (list_lines[i], " \t", 0);
	    test_name = strings[0];
	    if (test_name != NULL 
		&& strlen (test_name) > 0 
		&& test_name[0] != '#') {
		char * test_filename;

		test_filename = g_build_filename (TEST_DATA_DIR, test_name, NULL);

		xfail = FALSE;
		ignore = FALSE;
		for (j = 1; strings[j] != NULL; j++) {
		    if (strcmp (strings[j], "X") == 0)
			xfail = TRUE;
		    else if (strcmp (strings[j], "I") == 0)
			ignore = TRUE;
		}
		if (!ignore && rsvg_cairo_check (test_filename, xfail) != RSVG_TEST_SUCCESS)
		    status = RSVG_TEST_FAILURE;

		g_free (test_filename);
	    }
	    g_strfreev (strings);
	}
	g_strfreev (list_lines);
	g_free (list_content);
    } else 	
	fprintf (stderr, "Error opening test list file "TEST_LIST_FILENAME"\n");

    rsvg_test_html ("</table>\n");
    rsvg_test_html ("</html>\n");

    if (rsvg_test_html_file != NULL)
	fclose (rsvg_test_html_file);

    if (rsvg_test_log_file != NULL)
	fclose (rsvg_test_log_file);

    rsvg_cleanup ();

    return status;
}

