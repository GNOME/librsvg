/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "rsvg.h"
#include "test-utils.h"

typedef struct _FixtureData
{
    const gchar *test_name;
    const gchar *file_path;
} FixtureData;

static void
test_crash (FixtureData *fixture)
{
    RsvgHandle *handle;
    gchar *target_file;
    GError *error = NULL;

    target_file = g_build_filename (test_utils_get_test_data_path (),
                                    fixture->file_path, NULL);
    handle = rsvg_handle_new_from_file (target_file, &error);
    g_free (target_file);
    g_assert_no_error (error);

    g_object_unref (handle);
}

static FixtureData fixtures[] =
{
    {"/crash/only style information", "crash/bug620238.svg"}
};

static const gint n_fixtures = G_N_ELEMENTS (fixtures);

int
main (int argc, char *argv[])
{
    gint i;
    int result;

    g_type_init ();
    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < n_fixtures; i++)
        g_test_add_data_func (fixtures[i].test_name, &fixtures[i], (void*)test_crash);

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
