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
#include <glib/gstdio.h>

#include "librsvg/rsvg.h"

#include "test-utils.h"

static char *_output_dir;

static char *
get_output_dir (void)
{
    if (_output_dir == NULL) {
	g_autofree char *cwd = g_get_current_dir ();
        _output_dir = g_strconcat (cwd, G_DIR_SEPARATOR_S, "output", NULL);
        g_mkdir (_output_dir, 0777);
    }
    return _output_dir;
}

static char *
get_output_file (const char *test_file,
                 const char *extension)
{
    const char *output_dir = get_output_dir ();
    g_autofree char *base;

    base = g_path_get_basename (test_file);

    if (g_str_has_suffix (base, ".svg"))
	base[strlen (base) - strlen (".svg")] = '\0';

    return g_strconcat (output_dir, G_DIR_SEPARATOR_S, base, extension, NULL);
}

static void
save_image (cairo_surface_t *surface,
            const char      *test_name,
            const char      *extension)
{
    g_autofree char *filename = get_output_file (test_name, extension);

    g_test_message ("Storing test result image at %s", filename);
    g_assert (cairo_surface_write_to_png (surface, filename) == CAIRO_STATUS_SUCCESS);
}

static gboolean
is_svg_or_subdir (GFile *file)
{
    g_autofree char *basename = g_file_get_basename (file);

    if (g_str_has_prefix (basename, "ignore") || strcmp (basename, "resources") == 0)
        return FALSE;

    if (g_file_query_file_type (file, 0, NULL) == G_FILE_TYPE_DIRECTORY)
	return TRUE;

    return g_str_has_suffix (basename, ".svg");
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
    g_assert_cmpint (result, ==, length);

    return CAIRO_STATUS_SUCCESS;
}

static cairo_surface_t *
read_png (const char *test_name)
{
    g_autofree char *reference_uri = NULL;
    GFileInputStream *stream;
    GFile *file;
    GError *error = NULL;
    cairo_surface_t *surface;

    reference_uri = g_strconcat (test_name, "-ref.png", NULL);
    file = g_file_new_for_uri (reference_uri);

    stream = g_file_read (file, NULL, &error);
    g_assert_no_error (error);
    g_assert_nonnull (stream);

    surface = cairo_image_surface_create_from_png_stream (read_from_stream, stream);

    g_object_unref (stream);
    g_object_unref (file);

    return surface;
}

static cairo_surface_t *
extract_rectangle (cairo_surface_t *source,
		   int x,
		   int y,
		   int w,
		   int h)
{
    cairo_surface_t *dest;
    cairo_t *cr;

    dest = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, w, h);
    cr = cairo_create (dest);
    cairo_set_source_surface (cr, source, -x, -y);

    cairo_paint (cr);
    cairo_destroy (cr);

    return dest;
}

/*
 * Report that a test would have failed if we used stricter criteria,
 * but that we are tolerating it for a reason given in @message.
 *
 * This is the same as g_test_incomplete(), but with a workaround for
 * GNOME/glib#1474 so that we don't fail tests on older GLib.
 */
static void
test_tolerate (const gchar *message)
{
    if (glib_check_version (2, 57, 3)) {
        /* In GLib >= 2.57.3, g_test_incomplete() behaves as intended:
         * the test result is reported as an expected failure and the
         * overall test exits 0 */
        g_test_incomplete (message);
    }
    else {
        /* In GLib < 2.57.3, g_test_incomplete() reported the wrong TAP
         * result (an unexpected success) and the overall test exited 1,
         * which would break "make check". g_test_skip() is the next
         * best thing available. */
        g_test_skip (message);
    }
}

// https://gitlab.gnome.org/GNOME/librsvg/issues/91
//
// We were computing some offsets incorrectly if the initial transformation matrix
// passed to rsvg_handle_render_cairo() was not the identity matrix.  So,
// we create a surface with a "frame" around the destination for the image,
// and then only consider the pixels inside the frame.  This will require us
// to have a non-identity transformation (i.e. a translation matrix), which
// will test for this bug.
//
// The frame size is meant to be a ridiculous number to simulate an arbitrary
// offset.
#define FRAME_SIZE 47

static void
rsvg_cairo_check (gconstpointer data)
{
    GFile *test_file = G_FILE (data);
    RsvgHandle *rsvg;
    RsvgDimensionData dimensions;
    cairo_t *cr;
    cairo_surface_t *render_surface;
    cairo_surface_t *surface_a, *surface_b, *surface_diff;
    TestUtilsBufferDiffResult result;
    g_autofree char *test_file_base = NULL;
    unsigned int width_a, height_a, stride_a;
    unsigned int width_b, height_b, stride_b;
    GError *error = NULL;

    test_file_base = g_file_get_uri (test_file);
    if (g_str_has_suffix (test_file_base, ".svg"))
      test_file_base[strlen (test_file_base) - strlen (".svg")] = '\0';

    rsvg = rsvg_handle_new_from_gfile_sync (test_file, 0, NULL, &error);
    g_assert_no_error (error);
    g_assert_nonnull (rsvg);

    rsvg_handle_internal_set_testing (rsvg, TRUE);

    if (g_str_has_suffix (test_file_base, "-48dpi"))
      rsvg_handle_set_dpi_x_y (rsvg, 48.0, 48.0);
    else
      rsvg_handle_set_dpi_x_y (rsvg, 72.0, 72.0);
    rsvg_handle_get_dimensions (rsvg, &dimensions);
    g_assert_cmpint (dimensions.width, >, 0);
    g_assert_cmpint (dimensions.height, >, 0);

    render_surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
						 dimensions.width + 2 * FRAME_SIZE,
						 dimensions.height + 2 * FRAME_SIZE);
    cr = cairo_create (render_surface);
    cairo_translate (cr, FRAME_SIZE, FRAME_SIZE);
    g_assert (rsvg_handle_render_cairo (rsvg, cr));

    surface_a = extract_rectangle (render_surface,
				   FRAME_SIZE,
				   FRAME_SIZE,
				   dimensions.width,
				   dimensions.height);
    cairo_surface_destroy (render_surface);

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
#ifdef __x86_64__
	const unsigned int MAX_DIFF = 2;
#else
        /* https://gitlab.gnome.org/GNOME/librsvg/issues/178,
         * https://gitlab.gnome.org/GNOME/librsvg/issues/366 */
	const unsigned int MAX_DIFF = 20;
#endif
	const unsigned int WARN_DIFF = 2;

	surface_diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
						   dimensions.width, dimensions.height);

	test_utils_compare_surfaces (surface_a, surface_b, surface_diff, &result);

	if (result.pixels_changed && result.max_diff > MAX_DIFF) {
            g_test_fail ();
            save_image (surface_diff, test_file_base, "-diff.png");
	}
        else if (result.pixels_changed && result.max_diff > WARN_DIFF) {
            test_tolerate ("not the same as x86_64, but giving it the benefit of the doubt");
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

    /* For systemLanguage attribute tests.
     * The trailing ":" is intentional to test gitlab#425.
     */
    g_setenv ("LANGUAGE", "de:en_US:en:", TRUE);
    g_setenv ("LC_ALL", "de:en_US:en:", TRUE);

    g_test_init (&argc, &argv, NULL);

    test_utils_print_dependency_versions ();

    test_utils_setup_font_map ();

    if (argc < 2) {
        GFile *base, *tests;

        base = g_file_new_for_path (test_utils_get_test_data_path ());
        tests = g_file_get_child (base, "reftests");
        test_utils_add_test_for_all_files ("/rsvg-test/reftests", tests, tests, rsvg_cairo_check, is_svg_or_subdir);
        g_object_unref (tests);
        g_object_unref (base);
    } else {
        guint i;

        for (i = 1; i < argc; i++) {
            GFile *file = g_file_new_for_commandline_arg (argv[i]);

            test_utils_add_test_for_all_files ("/rsvg-test/reftests", NULL, file, rsvg_cairo_check, is_svg_or_subdir);

            g_object_unref (file);
        }
    }

    result = g_test_run ();

    return result;
}
