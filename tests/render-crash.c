/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "rsvg.h"
#include "rsvg-compat.h"
#include "test-utils.h"

static void
test_render_crash (gconstpointer data)
{
    GFile *file = G_FILE (data);
    RsvgHandle *handle;
    GError *error = NULL;
    RsvgDimensionData dimensions;
    cairo_surface_t *surface;
    cairo_t *cr;

    handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);
    g_assert_no_error (error);
    g_assert (handle != NULL);

    rsvg_handle_get_dimensions (handle, &dimensions);
    g_assert (dimensions.width > 0);
    g_assert (dimensions.height > 0);
    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
					  dimensions.width, dimensions.height);
    cr = cairo_create (surface);
    g_assert (rsvg_handle_render_cairo (handle, cr));

    cairo_surface_destroy (surface);
    cairo_destroy (cr);

    g_object_unref (handle);
}

int
main (int argc, char *argv[])
{
    GFile *base, *crash;
    int result;

    RSVG_G_TYPE_INIT;
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

    rsvg_cleanup ();

    return result;
}
