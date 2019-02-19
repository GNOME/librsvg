/* vim: set sw=4 sts=4: -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 8 -*-
 */

#define RSVG_DISABLE_DEPRECATION_WARNINGS

#include <glib.h>
#include "librsvg/rsvg.h"
#include "test-utils.h"

static void
test_render_crash (gconstpointer data)
{
    GFile *file = G_FILE (data);
    RsvgHandle *handle;
    GError *error = NULL;
    cairo_surface_t *surface;
    cairo_t *cr;

    handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);
    g_assert_no_error (error);
    g_assert (handle != NULL);

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 100, 100);
    cr = cairo_create (surface);

    g_assert (cairo_status (cr) == CAIRO_STATUS_SUCCESS);

    /* We don't even check the return value of the following function; we are just
     * trying to check that there are no crashes in the rendering code.
     */
    rsvg_handle_render_cairo (handle, cr);

    cairo_destroy (cr);
    cairo_surface_destroy (surface);

    g_object_unref (handle);
}

int
main (int argc, char *argv[])
{
    GFile *base, *crash;
    int result;

    g_test_init (&argc, &argv, NULL);

    if (argc < 2) {
        base = g_file_new_for_path (test_utils_get_test_data_path ());
        crash = g_file_get_child (base, "render-crash");
        test_utils_add_test_for_all_files ("/render-crash", crash, crash, test_render_crash, NULL);
        g_object_unref (base);
        g_object_unref (crash);
    } else {
        guint i;

        for (i = 1; i < argc; i++) {
            GFile *file = g_file_new_for_commandline_arg (argv[i]);

            test_utils_add_test_for_all_files ("/render-crash", NULL, file, test_render_crash, NULL);

            g_object_unref (file);
        }
    }

    result = g_test_run ();

    return result;
}
