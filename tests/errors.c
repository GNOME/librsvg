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
test_loading_error (gconstpointer data)
{
    const char *basename = data;
    char *filename = get_test_filename (basename);
    RsvgHandle *handle;
    GError *error = NULL;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_null (handle);
    g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);

    g_error_free (error);
}

static void
test_instancing_limit (gconstpointer data)
{
    const char *basename = data;
    char *filename = get_test_filename (basename);
    RsvgHandle *handle;
    GError *error = NULL;
    cairo_surface_t *surf;
    cairo_t *cr;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 1, 11);
    cr = cairo_create (surf);

    g_assert_false (rsvg_handle_render_cairo (handle, cr));

    g_object_unref (handle);
}

int
main (int argc, char **argv)
{
    g_test_init (&argc, &argv, NULL);

    g_test_add_data_func_full ("/errors/non_svg_element",
                               "335-non-svg-element.svg",
                               test_loading_error,
                               NULL);

    g_test_add_data_func_full ("/errors/instancing_limit/323-nested-use.svg",
                               "323-nested-use.svg",
                               test_instancing_limit,
                               NULL);

    g_test_add_data_func_full ("/errors/instancing_limit/515-pattern-billion-laughs.svg",
                               "515-pattern-billion-laughs.svg",
                               test_instancing_limit,
                               NULL);

    g_test_add_data_func_full ("/errors/instancing_limit/308-use-self-ref.svg",
                               "308-use-self-ref.svg",
                               test_instancing_limit,
                               NULL);
    g_test_add_data_func_full ("/errors/instancing_limit/308-recursive-use.svg",
                               "308-recursive-use.svg",
                               test_instancing_limit,
                               NULL);
    g_test_add_data_func_full ("/errors/instancing_limit/308-doubly-recursive-use.svg",
                               "308-doubly-recursive-use.svg",
                               test_instancing_limit,
                               NULL);

    g_test_add_data_func_full ("/errors/515-too-many-elements.svgz",
                               "515-too-many-elements.svgz",
                               test_loading_error,
                               NULL);


    return g_test_run ();
}
