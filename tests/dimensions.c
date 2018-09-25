/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "librsvg/rsvg.h"
#include "test-utils.h"

typedef struct
{
    const gchar *test_name;
    const gchar *file_path;
    const gchar *id;
    gint x;
    gint y;
    gint width;
    gint height;
    gboolean has_position;
    gboolean has_dimensions;
} FixtureData;

static void
test_dimensions (FixtureData *fixture)
{
    RsvgHandle *handle;
    RsvgPositionData position;
    RsvgDimensionData dimension;
    gchar *target_file;
    GError *error = NULL;

    target_file = g_build_filename (test_utils_get_test_data_path (),
                                    fixture->file_path, NULL);
    handle = rsvg_handle_new_from_file (target_file, &error);
    g_free (target_file);
    g_assert_no_error (error);

    if (fixture->id) {
        gboolean got_sub;

        got_sub = rsvg_handle_has_sub (handle, fixture->id);
        g_assert (got_sub);

        got_sub = rsvg_handle_get_position_sub (handle, &position, fixture->id);
        g_assert (got_sub);

        got_sub = rsvg_handle_get_dimensions_sub (handle, &dimension, fixture->id);
        g_assert (got_sub);

        g_message ("w=%d h=%d", dimension.width, dimension.height);
    } else {
        rsvg_handle_get_dimensions (handle, &dimension);
    }

    if (fixture->has_position) {
        g_assert_cmpint (fixture->x, ==, position.x);
        g_assert_cmpint (fixture->y, ==, position.y);
    }

    if (fixture->has_dimensions) {
        g_assert_cmpint (fixture->width,  ==, dimension.width);
        g_assert_cmpint (fixture->height, ==, dimension.height);
    }

    g_object_unref (handle);
}

static FixtureData fixtures[] =
{
    {
        "/dimensions/no viewbox, width and height",
        "dimensions/bug608102.svg",
        NULL,
        0, 0, 16, 16,
        FALSE, TRUE
    },
    {
        "/dimensions/100% width and height",
        "dimensions/bug612951.svg",
        NULL,
        0, 0, 47, 47,
        FALSE, TRUE
    },
    {
        "/dimensions/viewbox only",
        "dimensions/bug614018.svg",
        NULL,
        0, 0, 972, 546,
        FALSE, TRUE
    },
    {
        "/dimensions/sub/rect no unit",
        "dimensions/sub-rect-no-unit.svg",
        "#rect-no-unit",
        0, 0, 44, 45,
        FALSE, TRUE
    },
    {
        "/dimensions/sub/text_position",
        "dimensions/347-wrapper.svg",
        "#LabelA",
        80, 48, 0, 0,
        TRUE, FALSE
    }
    /* {"/dimensions/sub/rect with transform", "dimensions/bug564527.svg", "#back", 0, 0, 144, 203} */
};

static const gint n_fixtures = G_N_ELEMENTS (fixtures);

int
main (int argc, char *argv[])
{
    gint i;
    int result;

    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < n_fixtures; i++)
        g_test_add_data_func (fixtures[i].test_name, &fixtures[i], (void*)test_dimensions);

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
