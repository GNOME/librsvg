/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>

#include "librsvg/rsvg.h"
#include "test-utils.h"

/* These tests are meant to test the error handlers in librsvg.  As of 2.44.x we
 * don't have a public API that can actually report detailed errors; we just
 * report a boolean success value from the rendering functions.  In time, we can
 * add a richer API and test for specific errors here.
 */

static char *
get_test_filename (const char *basename) {
    return g_build_filename (test_utils_get_test_data_path (),
                             "errors",
                             basename,
                             NULL);
}

static void
test_non_svg_element (void)
{
    char *filename = get_test_filename ("335-non-svg-element.svg");
    RsvgHandle *handle;
    GError *error = NULL;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert (handle == NULL);
    g_assert (g_error_matches (error, RSVG_ERROR, RSVG_ERROR_FAILED));
}

static void
test_instancing_limit (void)
{
    char *filename = get_test_filename ("323-nested-use.svg");
    RsvgHandle *handle;
    GError *error = NULL;
    cairo_surface_t *surf;
    cairo_t *cr;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 1, 11);
    cr = cairo_create (surf);

    g_assert (!rsvg_handle_render_cairo (handle, cr));

    g_object_unref (handle);
}

int
main (int argc, char **argv)
{
    g_test_init (&argc, &argv, NULL);

    g_test_add_func ("/errors/non_svg_element", test_non_svg_element);
    g_test_add_func ("/errors/instancing_limit", test_instancing_limit);

    return g_test_run ();
}
