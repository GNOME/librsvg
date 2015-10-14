/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "rsvg.h"
#include "rsvg-compat.h"
#include "test-utils.h"

static void
test_crash (gconstpointer data)
{
    GFile *file = G_FILE (data);
    RsvgHandle *handle;
    GError *error = NULL;

    handle = rsvg_handle_new_from_gfile_sync (file, RSVG_HANDLE_FLAGS_NONE, NULL, &error);
    g_assert_no_error (error);
    g_assert (handle != NULL);

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
        crash = g_file_get_child (base, "crash");
        test_utils_add_test_for_all_files ("/crash", crash, crash, test_crash, NULL);
        g_object_unref (base);
        g_object_unref (crash);
    } else {
        guint i;

        for (i = 1; i < argc; i++) {
            GFile *file = g_file_new_for_commandline_arg (argv[i]);

            test_utils_add_test_for_all_files ("/crash", NULL, file, test_crash, NULL);

            g_object_unref (file);
        }
    }

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
