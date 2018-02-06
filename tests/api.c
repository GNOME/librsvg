/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>

#define RSVG_DISABLE_DEPRECATION_WARNINGS /* so we can test deprecated API */
#include "rsvg.h"
#include "test-utils.h"

/*
rsvg_handle_free
rsvg_handle_close
rsvg_handle_get_dimensions
rsvg_handle_get_dimensions_sub
rsvg_handle_get_position_sub
rsvg_handle_get_pixbuf
rsvg_handle_get_pixbuf_sub
rsvg_handle_get_base_uri
rsvg_handle_set_base_uri
rsvg_handle_set_size_callback
rsvg_handle_has_sub
rsvg_handle_internal_set_testing
rsvg_handle_new_from_file
rsvg_handle_new_from_gfile_sync
rsvg_handle_new_with_flags
rsvg_handle_new_from_stream_sync
rsvg_handle_new_from_data
rsvg_handle_render_cairo
rsvg_handle_set_base_gfile
rsvg_handle_write
rsvg_handle_read_stream_sync
rsvg_handle_get_title
rsvg_handle_get_desc
rsvg_handle_get_metadata
rsvg_handle_render_cairo_sub
*/

static void
handle_has_gtype (void)
{
    RsvgHandle *handle;

    handle = rsvg_handle_new();
    g_assert (G_OBJECT_TYPE (handle) == rsvg_handle_get_type ());
    g_object_unref (handle);
}

static char *
get_test_filename (const char *basename) {
    return g_build_filename (test_utils_get_test_data_path (),
                             "api",
                             basename,
                             NULL);
}

#define EXAMPLE_WIDTH 100
#define EXAMPLE_HEIGHT 400

#define XZOOM 2
#define YZOOM 3

#define MAX_WIDTH 10
#define MAX_HEIGHT 40

#define MAX_ZOOMED_WIDTH 20
#define MAX_ZOOMED_HEIGHT 120

#define EXAMPLE_ONE_ID "one"
#define EXAMPLE_TWO_ID "two"

#define EXAMPLE_ONE_X 0
#define EXAMPLE_ONE_Y 0
#define EXAMPLE_ONE_W 100
#define EXAMPLE_ONE_H 200

#define EXAMPLE_TWO_X 0
#define EXAMPLE_TWO_Y 200
#define EXAMPLE_TWO_W 100
#define EXAMPLE_TWO_H 200

static GdkPixbuf *
pixbuf_from_file (const char *filename, GError **error)
{
    return rsvg_pixbuf_from_file (filename, error);
}

static GdkPixbuf *
pixbuf_from_file_at_zoom (const char *filename, GError **error)
{
    return rsvg_pixbuf_from_file_at_zoom (filename, (double) XZOOM, (double) YZOOM, error);
}

static GdkPixbuf *
pixbuf_from_file_at_size (const char *filename, GError **error)
{
    return rsvg_pixbuf_from_file_at_size (filename, EXAMPLE_WIDTH * XZOOM, EXAMPLE_HEIGHT * YZOOM, error);
}

static GdkPixbuf *
pixbuf_from_file_at_max_size (const char *filename, GError **error)
{
    return rsvg_pixbuf_from_file_at_max_size (filename, MAX_WIDTH, MAX_HEIGHT, error);
}

static GdkPixbuf *
pixbuf_from_file_at_zoom_with_max (const char *filename, GError **error)
{
    return rsvg_pixbuf_from_file_at_zoom_with_max (filename,
                                                   XZOOM, YZOOM,
                                                   MAX_ZOOMED_WIDTH, MAX_ZOOMED_HEIGHT,
                                                   error);
}

typedef GdkPixbuf *(* PixbufCreateFn) (const char *filename, GError **error);

typedef struct {
    const char *test_name;
    PixbufCreateFn pixbuf_create_fn;
    int expected_width;
    int expected_height;
} PixbufTest;

static const PixbufTest pixbuf_tests[] = {
    {
        "/api/pixbuf_from_file",
        pixbuf_from_file,
        EXAMPLE_WIDTH,
        EXAMPLE_HEIGHT
    },
    {
        "/api/pixbuf_from_file_at_zoom",
        pixbuf_from_file_at_zoom,
        EXAMPLE_WIDTH * XZOOM,
        EXAMPLE_HEIGHT * YZOOM
    },
    {
        "/api/pixbuf_from_file_at_size",
        pixbuf_from_file_at_size,
        EXAMPLE_WIDTH * XZOOM,
        EXAMPLE_HEIGHT * YZOOM
    },
    {
        "/api/pixbuf_from_file_at_max_size",
        pixbuf_from_file_at_max_size,
        MAX_WIDTH,
        MAX_HEIGHT
    },
    {
        "/api/pixbuf_from_file_at_zoom_with_max",
        pixbuf_from_file_at_zoom_with_max,
        MAX_ZOOMED_WIDTH,
        MAX_ZOOMED_HEIGHT
    },
};

static void
test_pixbuf (gconstpointer data) {
    const PixbufTest *test = data;

    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    GdkPixbuf *pixbuf = test->pixbuf_create_fn (filename, &error);

    g_free (filename);

    g_assert (pixbuf != NULL);
    g_assert (error == NULL);
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, test->expected_width);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, test->expected_height);

    g_object_unref (pixbuf);
}

static void
noops (void)
{
    /* Just to test that these functions are present in the binary, I guess */
    rsvg_init ();
    rsvg_term ();
    g_assert (rsvg_cleanup != NULL); /* shouldn't call this one! */
}

static void
set_dpi (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    RsvgHandle *handle;

    /* Get dimensions at default DPI */

    RsvgDimensionData dim_100_dpi;
    RsvgDimensionData dim_200_300_dpi;

    /* Set 100 DPI */

    handle = rsvg_handle_new_from_file (filename, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    rsvg_handle_set_dpi (handle, 100.0);
    rsvg_handle_get_dimensions (handle, &dim_100_dpi);
    g_assert_cmpint (dim_100_dpi.width,  ==, 100);
    g_assert_cmpint (dim_100_dpi.height, ==, 400);
    g_object_unref (handle);

    /* Set 200x300 DPI */

    handle = rsvg_handle_new_from_file (filename, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    rsvg_handle_set_dpi_x_y (handle, 200.0, 300.0);
    rsvg_handle_get_dimensions (handle, &dim_200_300_dpi);
    g_object_unref (handle);

    /* Check! */

    g_assert_cmpint (dim_100_dpi.width * 2,  ==, dim_200_300_dpi.width);
    g_assert_cmpint (dim_100_dpi.height * 3, ==, dim_200_300_dpi.height);
}

static void
error_quark (void)
{
    g_assert_cmpint (rsvg_error_quark(), !=, 0);
}

static void
auto_generated (void)
{
    GTypeQuery q;

    g_type_query (RSVG_TYPE_ERROR, &q);
    g_assert (G_TYPE_IS_ENUM (q.type));
    g_assert_cmpstr (q.type_name, ==, "RsvgError");

    g_type_query (RSVG_TYPE_HANDLE_FLAGS, &q);
    g_assert (G_TYPE_IS_FLAGS (q.type));
    g_assert_cmpstr (q.type_name, ==, "RsvgHandleFlags");
}

int
main (int argc, char **argv)
{
    int i;

    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < G_N_ELEMENTS (pixbuf_tests); i++) {
        g_test_add_data_func (pixbuf_tests[i].test_name, &pixbuf_tests[i], test_pixbuf);
    }

    g_test_add_func ("/api/handle_has_gtype", handle_has_gtype);
    g_test_add_func ("/api/noops", noops);
    g_test_add_func ("/api/set_dpi", set_dpi);
    g_test_add_func ("/api/error_quark", error_quark);
    g_test_add_func ("/api/auto_generated", auto_generated);

    return g_test_run ();
}
