/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "librsvg/rsvg.h"
#include "test-utils.h"

static void
test_infinite_loop (gconstpointer data)
{
    if (g_test_subprocess ()) {
        GFile *file = G_FILE (data);
        RsvgHandle *handle;
        GError *error = NULL;
        cairo_surface_t *surface;
        cairo_t *cr;

        handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);
        g_assert_no_error (error);
        g_assert (handle != NULL);

        surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 10, 10);
        cr = cairo_create (surface);
        g_assert (rsvg_handle_render_cairo (handle, cr));

        cairo_surface_destroy (surface);
        cairo_destroy (cr);

        g_object_unref (handle);

        return;
    }

    g_test_trap_subprocess (NULL, 5000000, 0);
    g_assert (!g_test_trap_reached_timeout ());
    g_assert (g_test_trap_has_passed ());
}

int
main (int argc, char *argv[])
{
    GFile *base, *crash;
    int result;

    g_test_init (&argc, &argv, NULL);

    if (argc < 2) {
        base = g_file_new_for_path (test_utils_get_test_data_path ());
        crash = g_file_get_child (base, "infinite-loop");
        test_utils_add_test_for_all_files ("/infinite-loop", crash, crash, test_infinite_loop, NULL);
        g_object_unref (base);
        g_object_unref (crash);
    } else {
        guint i;

        for (i = 1; i < argc; i++) {
            GFile *file = g_file_new_for_commandline_arg (argv[i]);

            test_utils_add_test_for_all_files ("/infinite-loop", NULL, file, test_infinite_loop, NULL);

            g_object_unref (file);
        }
    }

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
