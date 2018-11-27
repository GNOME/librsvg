/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>

#define RSVG_DISABLE_DEPRECATION_WARNINGS /* so we can test deprecated API */
#include "librsvg/rsvg.h"
#include "test-utils.h"

/*
rsvg_handle_get_base_uri
rsvg_handle_set_base_uri
rsvg_handle_set_size_callback
rsvg_handle_internal_set_testing
rsvg_handle_set_base_gfile
rsvg_handle_get_title
rsvg_handle_get_desc
rsvg_handle_get_metadata
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

#define EXAMPLE_ONE_ID "#one"
#define EXAMPLE_TWO_ID "#two"
#define EXAMPLE_NONEXISTENT_ID "#nonexistent"

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

static void
handle_write_close_free (void)
{
    char *filename = get_test_filename ("dpi.svg");
    char *data;
    gsize length;
    gsize i;
    GError *error = NULL;

    g_assert (g_file_get_contents (filename, &data, &length, &error));
    g_assert (data != NULL);
    g_assert (error == NULL);

    RsvgHandle *handle = rsvg_handle_new_with_flags (RSVG_HANDLE_FLAGS_NONE);

    for (i = 0; i < length; i++) {
        g_assert (rsvg_handle_write (handle, (guchar *) &data[i], 1, &error));
        g_assert (error == NULL);
    }

    g_assert (rsvg_handle_close (handle, &error));
    g_assert (error == NULL);

    rsvg_handle_free (handle);
    g_free (data);
}

static void
handle_new_from_file (void)
{
    char *filename = get_test_filename ("dpi.svg");
    char *uri = g_strconcat ("file://", filename, NULL);

    RsvgHandle *handle;
    GError *error = NULL;

    /* rsvg_handle_new_from_file() can take both filenames and URIs */

    handle = rsvg_handle_new_from_file (filename, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);
    g_object_unref (handle);

    handle = rsvg_handle_new_from_file (uri, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);
    g_object_unref (handle);

    g_free (filename);
    g_free (uri);
}

static void
handle_new_from_data (void)
{
    char *filename = get_test_filename ("dpi.svg");
    char *data;
    gsize length;
    GError *error = NULL;

    g_assert (g_file_get_contents (filename, &data, &length, &error));
    g_assert (data != NULL);
    g_assert (error == NULL);

    RsvgHandle *handle = rsvg_handle_new_from_data ((guint8 *) data, length, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    g_object_unref (handle);
    g_free (data);
}

static void
handle_new_from_gfile_sync (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    GFile *file = g_file_new_for_path (filename);
    g_assert (file != NULL);

    g_free (filename);

    RsvgHandle *handle = rsvg_handle_new_from_gfile_sync (file,
                                                          RSVG_HANDLE_FLAGS_NONE,
                                                          NULL,
                                                          &error);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    g_object_unref (handle);
    g_object_unref (file);
}

static void
handle_new_from_stream_sync (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    GFile *file = g_file_new_for_path (filename);
    g_assert (file != NULL);

    g_free (filename);

    GFileInputStream *stream = g_file_read (file, NULL, &error);
    g_assert (stream != NULL);
    g_assert (error == NULL);

    RsvgHandle *handle = rsvg_handle_new_from_stream_sync (G_INPUT_STREAM (stream),
                                                           file,
                                                           RSVG_HANDLE_FLAGS_NONE,
                                                           NULL,
                                                           &error);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    g_object_unref (handle);
    g_object_unref (file);
    g_object_unref (stream);
}

static void
handle_read_stream_sync (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    GFile *file = g_file_new_for_path (filename);
    g_assert (file != NULL);

    g_free (filename);

    GFileInputStream *stream = g_file_read (file, NULL, &error);
    g_assert (stream != NULL);
    g_assert (error == NULL);

    RsvgHandle *handle = rsvg_handle_new ();

    g_assert (rsvg_handle_read_stream_sync (handle, G_INPUT_STREAM (stream), NULL, &error));
    g_assert (error == NULL);

    g_object_unref (handle);
    g_object_unref (file);
    g_object_unref (stream);
}

static void
handle_has_sub (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_ONE_ID));
    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_TWO_ID));
    g_assert (!rsvg_handle_has_sub (handle, "#foo"));
}

static void
handle_get_pixbuf (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    GdkPixbuf *pixbuf = rsvg_handle_get_pixbuf (handle);
    g_assert (pixbuf != NULL);

    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, EXAMPLE_WIDTH);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, EXAMPLE_HEIGHT);

    g_object_unref (pixbuf);
    g_object_unref (handle);
}

static void
handle_get_pixbuf_sub (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    GdkPixbuf *pixbuf = rsvg_handle_get_pixbuf_sub (handle, EXAMPLE_ONE_ID);
    g_assert (pixbuf != NULL);

    /* Note that rsvg_handle_get_pixbuf_sub() creates a surface the size of the
     * whole SVG, not just the size of the sub-element.
     */
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, EXAMPLE_WIDTH);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, EXAMPLE_HEIGHT);

    g_object_unref (pixbuf);
    g_object_unref (handle);
}

static void
dimensions_and_position (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert (handle != NULL);
    g_assert (error == NULL);

    RsvgDimensionData dim;

    g_assert (rsvg_handle_get_dimensions_sub (handle, &dim, EXAMPLE_TWO_ID));
    g_assert_cmpint (dim.width,  ==, EXAMPLE_TWO_W);
    g_assert_cmpint (dim.height, ==, EXAMPLE_TWO_H);

    RsvgPositionData pos;
    g_assert (rsvg_handle_get_position_sub (handle, &pos, EXAMPLE_TWO_ID));
    g_assert_cmpint (pos.x, ==, EXAMPLE_TWO_X);
    g_assert_cmpint (pos.y, ==, EXAMPLE_TWO_Y);

    g_assert (!rsvg_handle_get_position_sub (handle, &pos, EXAMPLE_NONEXISTENT_ID));
    g_assert (!rsvg_handle_get_dimensions_sub (handle, &dim, EXAMPLE_NONEXISTENT_ID));

    g_object_unref (handle);
}

static void
detects_cairo_context_in_error (void)
{
    if (g_test_subprocess ()) {
        char *filename = get_test_filename ("example.svg");
        GError *error = NULL;

        RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
        g_assert (handle != NULL);
        g_assert (error == NULL);

        /* this is wrong; it is to simulate creating a surface and a cairo_t in error */
        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, -1, -1);
        cairo_t *cr = cairo_create (surf);

        /* rsvg_handle_render_cairo() should return FALSE when it gets a cr in an error state */
        g_assert (!rsvg_handle_render_cairo (handle, cr));

        return;
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_failed ();
    g_test_trap_assert_stderr ("*WARNING*cannot render on a cairo_t with a failure status*");
}

static gboolean
matrixes_are_equal (cairo_matrix_t *a, cairo_matrix_t *b)
{
    return (a->xx == b->xx &&
            a->yx == b->yx &&
            a->xy == b->xy &&
            a->yy == b->yy &&
            a->x0 == b->x0 &&
            a->y0 == b->y0);
}

static void
can_draw_to_non_image_surface (void)
{
    cairo_rectangle_t rect;
    cairo_surface_t *surface;
    cairo_t *cr;

    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    rect.x = 0.0;
    rect.y = 0.0;
    rect.width = 100.0;
    rect.height = 100.0;

    /* We create a surface that is not a Cairo image surface,
     * so we can test that in fact we can render to non-image surfaces.
     */
    surface = cairo_recording_surface_create (CAIRO_CONTENT_COLOR_ALPHA, &rect);
    cr = cairo_create (surface);

    cairo_translate (cr, 42.0, 42.0);

    cairo_matrix_t original_affine;
    cairo_get_matrix (cr, &original_affine);

    g_assert (rsvg_handle_render_cairo (handle, cr));

    cairo_matrix_t new_affine;
    cairo_get_matrix (cr, &new_affine);

    g_assert (matrixes_are_equal (&original_affine, &new_affine));

    g_object_unref (handle);

    cairo_destroy (cr);
}

/* Test that we preserve the affine transformation in the cr during a call
 * to rsvg_handle_render_cairo_sub().
 */
static void
render_cairo_sub (void)
{
    char *filename = get_test_filename ("334-element-positions.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_assert (handle != NULL);
    g_assert (error == NULL);

    cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 200, 200);
    cairo_t *cr = cairo_create (surf);

    cairo_translate (cr, 42.0, 42.0);

    cairo_matrix_t original_affine;
    cairo_get_matrix (cr, &original_affine);

    g_assert (rsvg_handle_render_cairo_sub (handle, cr, "#button5-leader"));

    cairo_matrix_t new_affine;
    cairo_get_matrix (cr, &new_affine);

    g_assert (matrixes_are_equal (&original_affine, &new_affine));

    g_object_unref (handle);
    cairo_destroy (cr);
}

/* https://gitlab.gnome.org/GNOME/librsvg/issues/385 */
static void
no_write_before_close (void)
{
    RsvgHandle *handle = rsvg_handle_new();
    GError *error = NULL;

    g_assert (rsvg_handle_close (handle, &error) == FALSE);
    g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);
    g_error_free (error);

    g_object_unref (handle);
}

static void
empty_write_close (void)
{
    RsvgHandle *handle = rsvg_handle_new();
    GError *error = NULL;
    guchar buf = 0;

    g_assert (rsvg_handle_write (handle, &buf, 0, &error) == TRUE);
    g_assert_no_error (error);

    g_assert (rsvg_handle_close (handle, &error) == FALSE);
    g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);

    g_error_free (error);

    g_object_unref (handle);
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
    g_test_add_func ("/api/handle_write_close_free", handle_write_close_free);
    g_test_add_func ("/api/handle_new_from_file", handle_new_from_file);
    g_test_add_func ("/api/handle_new_from_data", handle_new_from_data);
    g_test_add_func ("/api/handle_new_from_gfile_sync", handle_new_from_gfile_sync);
    g_test_add_func ("/api/handle_new_from_stream_sync", handle_new_from_stream_sync);
    g_test_add_func ("/api/handle_read_stream_sync", handle_read_stream_sync);
    g_test_add_func ("/api/handle_has_sub", handle_has_sub);
    g_test_add_func ("/api/handle_get_pixbuf", handle_get_pixbuf);
    g_test_add_func ("/api/handle_get_pixbuf_sub", handle_get_pixbuf_sub);
    g_test_add_func ("/api/dimensions_and_position", dimensions_and_position);
    g_test_add_func ("/api/detects_cairo_context_in_error", detects_cairo_context_in_error);
    g_test_add_func ("/api/can_draw_to_non_image_surface", can_draw_to_non_image_surface);
    g_test_add_func ("/api/render_cairo_sub", render_cairo_sub);
    g_test_add_func ("/api/no_write_before_close", no_write_before_close);
    g_test_add_func ("/api/empty_write_close", empty_write_close);

    return g_test_run ();
}
