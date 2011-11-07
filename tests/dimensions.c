/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "rsvg.h"
#include "test-utils.h"

typedef struct _FixtureData
{
    const gchar *test_name;
    const gchar *file_path;
    const gchar *id;
    gint width;
    gint height;
} FixtureData;

static void
test_dimensions (FixtureData *fixture)
{
    RsvgHandle *handle;
    RsvgDimensionData dimension;
    gchar *target_file;
    GError *error = NULL;

    target_file = g_build_filename (test_utils_get_test_data_path (),
                                    fixture->file_path, NULL);
    handle = rsvg_handle_new_from_file (target_file, &error);
    g_free (target_file);
    g_assert_no_error (error);

    if (fixture->id)
        rsvg_handle_get_dimensions_sub (handle, &dimension, fixture->id);
    else
        rsvg_handle_get_dimensions (handle, &dimension);
    g_assert_cmpint (fixture->width,  ==, dimension.width);
    g_assert_cmpint (fixture->height, ==, dimension.height);

    g_object_unref (handle);
}

static FixtureData fixtures[] =
{
    {"/dimensions/no viewbox, width and height", "dimensions/bug608102.svg", NULL, 16, 16},
    {"/dimensions/100% width and height", "dimensions/bug612951.svg", NULL, 45, 45},
    {"/dimensions/viewbox only", "dimensions/bug614018.svg", NULL, 3, 2},
    {"/dimensions/sub/rect no unit", "dimensions/sub-rect-no-unit.svg", "#rect-no-unit", 44, 45},
    {"/dimensions/sub/rect with transform", "dimensions/bug564527.svg", "#back", 144, 203}
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
        g_test_add_data_func (fixtures[i].test_name, &fixtures[i], (void*)test_dimensions);

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
