/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>
#include <cairo.h>

#define RSVG_DISABLE_DEPRECATION_WARNINGS /* so we can test deprecated API */
#include "librsvg/rsvg.h"
#include "test-utils.h"

/*
  Untested:
  rsvg_handle_internal_set_testing
*/

static void
handle_has_gtype (void)
{
    RsvgHandle *handle;

    handle = rsvg_handle_new();
    g_assert (G_OBJECT_TYPE (handle) == rsvg_handle_get_type ());
    g_object_unref (handle);
}

static gboolean
flags_value_matches (GFlagsValue *v,
                     guint value,
                     const char *value_name,
                     const char *value_nick)
{
    return (v->value == value
            && strcmp (v->value_name, value_name) == 0
            && strcmp (v->value_nick, value_nick) == 0);
}

static void
flags_registration (void)
{
    GType ty;
    GTypeQuery q;
    GTypeClass *type_class;
    GFlagsClass *flags_class;

    ty = RSVG_TYPE_HANDLE_FLAGS;

    g_assert (ty != G_TYPE_INVALID);

    g_type_query (RSVG_TYPE_HANDLE_FLAGS, &q);
    g_assert (q.type == ty);
    g_assert (G_TYPE_IS_FLAGS (q.type));
    g_assert_cmpstr (q.type_name, ==, "RsvgHandleFlags");

    type_class = g_type_class_ref (ty);
    g_assert (G_IS_FLAGS_CLASS (type_class));
    g_assert (G_FLAGS_CLASS_TYPE (type_class) == ty);

    flags_class = G_FLAGS_CLASS (type_class);
    g_assert_cmpint (flags_class->n_values, ==, 3);

    g_assert (flags_value_matches(&flags_class->values[0],
                                  RSVG_HANDLE_FLAGS_NONE,
                                  "RSVG_HANDLE_FLAGS_NONE",
                                  "flags-none"));

    g_assert (flags_value_matches(&flags_class->values[1],
                                  RSVG_HANDLE_FLAG_UNLIMITED,
                                  "RSVG_HANDLE_FLAG_UNLIMITED",
                                  "flag-unlimited"));

    g_assert (flags_value_matches(&flags_class->values[2],
                                  RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA,
                                  "RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA",
                                  "flag-keep-image-data"));

    g_type_class_unref (type_class);
}

static gboolean
enum_value_matches (GEnumValue *v,
                    gint value,
                    const char *value_name,
                    const char *value_nick)
{
    return (v->value == value
            && strcmp (v->value_name, value_name) == 0
            && strcmp (v->value_nick, value_nick) == 0);
}

static void
error_registration (void)
{
    GType ty;
    GTypeQuery q;
    GTypeClass *type_class;
    GEnumClass *enum_class;

    g_assert_cmpint (RSVG_ERROR, !=, 0);

    ty = RSVG_TYPE_ERROR;

    g_assert (ty != G_TYPE_INVALID);

    g_type_query (ty, &q);
    g_assert (q.type == ty);
    g_assert (G_TYPE_IS_ENUM (q.type));
    g_assert_cmpstr (q.type_name, ==, "RsvgError");

    type_class = g_type_class_ref (ty);
    g_assert (G_IS_ENUM_CLASS (type_class));
    g_assert (G_ENUM_CLASS_TYPE (type_class) == ty);

    enum_class = G_ENUM_CLASS (type_class);
    g_assert_cmpint (enum_class->n_values, ==, 1);

    g_assert (enum_value_matches (&enum_class->values[0],
                                  RSVG_ERROR_FAILED,
                                  "RSVG_ERROR_FAILED",
                                  "failed"));

    g_type_class_unref (type_class);
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
test_pixbuf (gconstpointer data)
{
    const PixbufTest *test = data;

    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    GdkPixbuf *pixbuf = test->pixbuf_create_fn (filename, &error);

    g_free (filename);

    g_assert_nonnull (pixbuf);
    g_assert_no_error (error);
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, test->expected_width);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, test->expected_height);

    g_object_unref (pixbuf);
}

static void
pixbuf_overflow (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    g_assert (!rsvg_pixbuf_from_file_at_zoom (filename, 1000000.0, 1000000.0, &error));
    g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);
    g_error_free (error);
    g_free (filename);
}

static void
noops (void)
{
    /* Just to test that these functions are present in the binary, I guess */
    rsvg_init ();
    rsvg_term ();
    rsvg_cleanup ();

    /* Just test that these are in the binary */
    g_assert_nonnull (rsvg_handle_get_title);
    g_assert_nonnull (rsvg_handle_get_desc);
    g_assert_nonnull (rsvg_handle_get_metadata);
}

static void
set_dpi (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    RsvgHandle *handle;
    RsvgDimensionData dim;

    rsvg_set_default_dpi (100.0);

    handle = rsvg_handle_new_from_file (filename, &error);
    g_assert_nonnull (handle);
    g_assert_no_error (error);

    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 100);
    g_assert_cmpint (dim.height, ==, 400);

    rsvg_handle_set_dpi (handle, 200.0);
    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 200);
    g_assert_cmpint (dim.height, ==, 800);
    g_object_unref (handle);

    handle = rsvg_handle_new_from_file (filename, &error);
    g_assert_nonnull (handle);
    g_assert_no_error (error);

    rsvg_handle_set_dpi_x_y (handle, 400.0, 300.0);
    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 400);
    g_assert_cmpint (dim.height, ==, 1200);
    g_object_unref (handle);

    g_free (filename);
}

static void
base_uri (void)
{
    RsvgHandle *handle = rsvg_handle_new ();
    const char *uri;

    uri = rsvg_handle_get_base_uri (handle);
    g_assert_null (uri);

    rsvg_handle_set_base_uri (handle, "file:///foo/bar.svg");
    uri = rsvg_handle_get_base_uri (handle);

    g_assert_cmpstr (uri, ==, "file:///foo/bar.svg");

    g_object_unref (handle);
}

static void
base_gfile (void)
{
    RsvgHandle *handle = rsvg_handle_new ();
    GFile *file;
    const char *uri;

    uri = rsvg_handle_get_base_uri (handle);
    g_assert_null (uri);

    file = g_file_new_for_uri ("file:///foo/bar.svg");

    rsvg_handle_set_base_gfile (handle, file);
    uri = rsvg_handle_get_base_uri (handle);

    g_assert_cmpstr (uri, ==, "file:///foo/bar.svg");

    g_object_unref (file);
    g_object_unref (handle);
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
    g_free (filename);

    g_assert_nonnull (data);
    g_assert_no_error (error);

    RsvgHandle *handle = rsvg_handle_new_with_flags (RSVG_HANDLE_FLAGS_NONE);

    for (i = 0; i < length; i++) {
        g_assert (rsvg_handle_write (handle, (guchar *) &data[i], 1, &error));
        g_assert_no_error (error);
    }

    g_assert (rsvg_handle_close (handle, &error));
    g_assert_no_error (error);

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
    g_assert_nonnull (handle);
    g_assert_no_error (error);
    g_object_unref (handle);

    handle = rsvg_handle_new_from_file (uri, &error);
    g_assert_nonnull (handle);
    g_assert_no_error (error);
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
    g_free (filename);

    g_assert_nonnull (data);
    g_assert_no_error (error);

    RsvgHandle *handle = rsvg_handle_new_from_data ((guint8 *) data, length, &error);
    g_assert_nonnull (handle);
    g_assert_no_error (error);

    g_object_unref (handle);
    g_free (data);
}

static void
handle_new_from_gfile_sync (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    GFile *file = g_file_new_for_path (filename);
    g_assert_nonnull (file);

    g_free (filename);

    RsvgHandle *handle = rsvg_handle_new_from_gfile_sync (file,
                                                          RSVG_HANDLE_FLAGS_NONE,
                                                          NULL,
                                                          &error);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    g_object_unref (handle);
    g_object_unref (file);
}

static void
handle_new_from_stream_sync (void)
{
    char *filename = get_test_filename ("dpi.svg");
    GError *error = NULL;
    GFile *file = g_file_new_for_path (filename);
    g_assert_nonnull (file);

    g_free (filename);

    GFileInputStream *stream = g_file_read (file, NULL, &error);
    g_assert (stream != NULL);
    g_assert_no_error (error);

    RsvgHandle *handle = rsvg_handle_new_from_stream_sync (G_INPUT_STREAM (stream),
                                                           file,
                                                           RSVG_HANDLE_FLAGS_NONE,
                                                           NULL,
                                                           &error);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

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
    g_assert_nonnull (file);

    g_free (filename);

    GFileInputStream *stream = g_file_read (file, NULL, &error);
    g_assert_nonnull (stream);
    g_assert_no_error (error);

    RsvgHandle *handle = rsvg_handle_new ();

    g_assert (rsvg_handle_read_stream_sync (handle, G_INPUT_STREAM (stream), NULL, &error));
    g_assert_no_error (error);

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

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_ONE_ID));
    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_TWO_ID));
    g_assert (!rsvg_handle_has_sub (handle, "#foo"));

    g_object_unref (handle);
}

static void
test_get_pixbuf (gboolean sub)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    GdkPixbuf *pixbuf;
    if (sub) {
        pixbuf = rsvg_handle_get_pixbuf_sub (handle, EXAMPLE_ONE_ID);
    } else {
        pixbuf = rsvg_handle_get_pixbuf (handle);
    }

    g_assert_nonnull (pixbuf);

    /* Note that rsvg_handle_get_pixbuf_sub() creates a surface the size of the
     * whole SVG, not just the size of the sub-element.
     */
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, EXAMPLE_WIDTH);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, EXAMPLE_HEIGHT);

    cairo_surface_t *surface_a = test_utils_cairo_surface_from_pixbuf (pixbuf);
    cairo_surface_t *surface_b = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, EXAMPLE_WIDTH, EXAMPLE_HEIGHT);
    cairo_surface_t *surface_diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, EXAMPLE_WIDTH, EXAMPLE_HEIGHT);

    g_object_unref (pixbuf);

    g_assert_nonnull (surface_a);
    g_assert_nonnull (surface_b);
    g_assert_nonnull (surface_diff);

    cairo_t *cr = cairo_create (surface_b);
    if (sub) {
        g_assert (rsvg_handle_render_cairo_sub (handle, cr, EXAMPLE_ONE_ID));
    } else {
        g_assert (rsvg_handle_render_cairo (handle, cr));
    }
    cairo_destroy (cr);

    g_object_unref (handle);

    TestUtilsBufferDiffResult result = {0, 0};
    test_utils_compare_surfaces (surface_a, surface_b, surface_diff, &result);

    if (result.pixels_changed && result.max_diff > 0) {
        g_test_fail ();
    }

    cairo_surface_destroy (surface_a);
    cairo_surface_destroy (surface_b);
    cairo_surface_destroy (surface_diff);
}

static void
handle_get_pixbuf (void)
{
    test_get_pixbuf (FALSE);
}

static void
handle_get_pixbuf_sub (void)
{
    test_get_pixbuf (TRUE);
}

static void
dimensions_and_position (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    RsvgDimensionData dim;

    g_assert (rsvg_handle_get_dimensions_sub (handle, &dim, EXAMPLE_TWO_ID));
    g_assert_cmpint (dim.width,  ==, EXAMPLE_TWO_W);
    g_assert_cmpint (dim.height, ==, EXAMPLE_TWO_H);

    RsvgPositionData pos;
    g_assert (rsvg_handle_get_position_sub (handle, &pos, EXAMPLE_TWO_ID));
    g_assert_cmpint (pos.x, ==, EXAMPLE_TWO_X);
    g_assert_cmpint (pos.y, ==, EXAMPLE_TWO_Y);

    g_assert_false (rsvg_handle_get_position_sub (handle, &pos, EXAMPLE_NONEXISTENT_ID));
    g_assert_false (rsvg_handle_get_dimensions_sub (handle, &dim, EXAMPLE_NONEXISTENT_ID));

    g_object_unref (handle);
}

struct size_func_data
{
    gboolean called;
    gboolean destroyed;
    gboolean testing_size_func_calls;
};

static void
size_func (gint *width, gint *height, gpointer user_data)
{
    struct size_func_data *data = user_data;

    if (data->testing_size_func_calls) {
        g_assert_false (data->called);
        data->called = TRUE;

        g_assert_false (data->destroyed);
    }

    *width = 42;
    *height = 43;
}

static void
size_func_destroy (gpointer user_data)
{
    struct size_func_data *data = user_data;

    if (data->testing_size_func_calls) {
        g_assert_false (data->destroyed);
        data->destroyed = TRUE;
    }
}

static void
set_size_callback (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;
    RsvgHandle *handle;
    struct size_func_data data;
    RsvgDimensionData dim;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    data.called = FALSE;
    data.destroyed = FALSE;
    data.testing_size_func_calls = TRUE;

    rsvg_handle_set_size_callback (handle, size_func, &data, size_func_destroy);

    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 42);
    g_assert_cmpint (dim.height, ==, 43);

    g_object_unref (handle);

    g_assert_true (data.called);
    g_assert_true (data.destroyed);
}

static void
reset_size_callback (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;
    RsvgHandle *handle;
    struct size_func_data data_1;
    struct size_func_data data_2;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    data_1.called = FALSE;
    data_1.destroyed = FALSE;
    data_1.testing_size_func_calls = TRUE;

    rsvg_handle_set_size_callback (handle, size_func, &data_1, size_func_destroy);

    data_2.called = FALSE;
    data_2.destroyed = FALSE;
    data_2.testing_size_func_calls = TRUE;

    rsvg_handle_set_size_callback (handle, size_func, &data_2, size_func_destroy);
    g_assert_true (data_1.destroyed);

    g_object_unref (handle);

    g_assert_true (data_2.destroyed);
}

static void
zero_size_func (gint *width, gint *height, gpointer user_data)
{
    *width = 0;
    *height = 0;
}

static void
render_with_zero_size_callback (void)
{
    /* gdk_pixbuf_get_file_info() uses a GdkPixbufLoader, but in its
     * "size-prepared" callback it saves the computed size, and then calls
     * gdk_pixbuf_loader_set_size(loader, 0, 0).  Presumably it does to tell
     * loaders that it only wanted to know the size, but that they shouldn't
     * decode or render the image to a pixbuf buffer.
     *
     * Librsvg used to panic when getting (0, 0) from the size_callback; this
     * test is to check that there is no such crash now.  Instead, librsvg
     * will return a 1x1 transparent pixbuf.
     */
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;
    RsvgHandle *handle;
    GdkPixbuf *pixbuf;

    handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    rsvg_handle_set_size_callback (handle, zero_size_func, NULL, NULL);

    pixbuf = rsvg_handle_get_pixbuf (handle);
    g_assert_nonnull (pixbuf);
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, 1);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, 1);

    g_object_unref (pixbuf);
    g_object_unref (handle);
}

static void
pixbuf_size_func (gint *width, gint *height, gpointer user_data)
{
    *width = 420;
    *height = 430;
}

static void
get_pixbuf_with_size_callback (void)
{
    RsvgHandle *handle = rsvg_handle_new ();

    rsvg_handle_set_size_callback (handle, pixbuf_size_func, NULL, NULL);

    char *filename = get_test_filename ("example.svg");
    guchar *data = NULL;
    gsize length;
    GError *error = NULL;

    g_assert (g_file_get_contents (filename, (gchar **) &data, &length, &error));
    g_assert_nonnull (data);

    g_free (filename);

    g_assert (rsvg_handle_write (handle, data, length, &error));
    g_assert_no_error (error);

    g_assert (rsvg_handle_close (handle, &error));
    g_assert_no_error (error);

    GdkPixbuf *pixbuf = rsvg_handle_get_pixbuf (handle);
    g_assert_nonnull (pixbuf);
    g_assert_cmpint (gdk_pixbuf_get_width (pixbuf), ==, 420);
    g_assert_cmpint (gdk_pixbuf_get_height (pixbuf), ==, 430);

    g_object_unref (pixbuf);
    g_free (data);
    g_object_unref (handle);
}

static void
detects_cairo_context_in_error (void)
{
    if (g_test_subprocess ()) {
        char *filename = get_test_filename ("example.svg");
        GError *error = NULL;

        RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
        g_free (filename);

        g_assert_nonnull (handle);
        g_assert_no_error (error);

        /* this is wrong; it is to simulate creating a surface and a cairo_t in error */
        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, -1, -1);
        cairo_t *cr = cairo_create (surf);
        /* rsvg_handle_render_cairo() should return FALSE when it gets a cr in an error state */
        g_assert_false (rsvg_handle_render_cairo (handle, cr));

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
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

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
    cairo_surface_destroy (surface);
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
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

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
    cairo_surface_destroy (surf);
}

static void
get_intrinsic_dimensions (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    gboolean has_width;
    RsvgLength width;
    gboolean has_height;
    RsvgLength height;
    gboolean has_viewbox;
    RsvgRectangle viewbox;

    rsvg_handle_get_intrinsic_dimensions (handle, &has_width, &width, &has_height, &height, &has_viewbox, &viewbox);

    g_assert (has_width);
    g_assert_cmpfloat (width.length, ==, 100.0);
    g_assert (width.unit == RSVG_UNIT_PX);

    g_assert (has_height);
    g_assert_cmpfloat (height.length, ==, 400.0);
    g_assert (height.unit == RSVG_UNIT_PX);

    g_assert (has_viewbox);
    g_assert_cmpfloat (viewbox.x, ==, 0.0);
    g_assert_cmpfloat (viewbox.y, ==, 0.0);
    g_assert_cmpfloat (viewbox.width, ==, 100.0);
    g_assert_cmpfloat (viewbox.height, ==, 400.0);

    g_object_unref (handle);
}

static void
render_document (void)
{
    char *filename = get_test_filename ("document.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 150, 150);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 50.0, 50.0, 50.0, 50.0 };

    g_assert (rsvg_handle_render_document (handle, cr, &viewport, &error));
    g_assert_no_error (error);

    cairo_destroy (cr);

    cairo_surface_t *expected = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 150, 150);
    cr = cairo_create (expected);

    cairo_translate (cr, 50.0, 50.0);
    cairo_rectangle (cr, 10.0, 10.0, 30.0, 30.0);
    cairo_set_source_rgba (cr, 0.0, 0.0, 1.0, 0.5);
    cairo_fill (cr);
    cairo_destroy (cr);

    cairo_surface_t *diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 150, 150);

    TestUtilsBufferDiffResult result = {0, 0};
    test_utils_compare_surfaces (output, expected, diff, &result);

    if (result.pixels_changed && result.max_diff > 0) {
        g_test_fail ();
    }

    cairo_surface_destroy (diff);
    cairo_surface_destroy (expected);
    cairo_surface_destroy (output);
    g_object_unref (handle);
}

static void
get_geometry_for_layer (void)
{
    char *filename = get_test_filename ("geometry.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    RsvgRectangle viewport = { 0.0, 0.0, 100.0, 400.0 };
    RsvgRectangle ink_rect;
    RsvgRectangle logical_rect;

    g_assert_false (rsvg_handle_get_geometry_for_layer (handle, "#nonexistent", &viewport,
                                                        &ink_rect, &logical_rect, &error));
    g_assert_nonnull (error);

    g_clear_error (&error);

    g_assert (rsvg_handle_get_geometry_for_layer (handle, "#two", &viewport,
                                                  &ink_rect, &logical_rect, &error));
    g_assert_no_error (error);

    g_assert_cmpfloat (ink_rect.x, ==, 5.0);
    g_assert_cmpfloat (ink_rect.y, ==, 195.0);
    g_assert_cmpfloat (ink_rect.width, ==, 90.0);
    g_assert_cmpfloat (ink_rect.height, ==, 110.0);

    g_assert_cmpfloat (logical_rect.x, ==, 10.0);
    g_assert_cmpfloat (logical_rect.y, ==, 200.0);
    g_assert_cmpfloat (logical_rect.width, ==, 80.0);
    g_assert_cmpfloat (logical_rect.height, ==, 100.0);

    g_object_unref (handle);
}

static void
render_layer (void)
{
    char *filename = get_test_filename ("layers.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 100.0, 100.0, 100.0, 100.0 };

    g_assert (rsvg_handle_render_layer (handle, cr, "#bar", &viewport, &error));
    g_assert_no_error (error);

    cairo_destroy (cr);

    cairo_surface_t *expected = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cr = cairo_create (expected);

    cairo_translate (cr, 100.0, 100.0);
    cairo_rectangle (cr, 20.0, 20.0, 30.0, 30.0);
    cairo_set_source_rgba (cr, 0.0, 0.0, 1.0, 1.0);
    cairo_fill (cr);
    cairo_destroy (cr);

    cairo_surface_t *diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);

    TestUtilsBufferDiffResult result = {0, 0};
    test_utils_compare_surfaces (output, expected, diff, &result);

    if (result.pixels_changed && result.max_diff > 0) {
        g_test_fail ();
    }

    cairo_surface_destroy (diff);
    cairo_surface_destroy (expected);
    cairo_surface_destroy (output);
    g_object_unref (handle);
}

static void
untransformed_element (void)
{
    char *filename = get_test_filename ("geometry-element.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    RsvgRectangle ink_rect;
    RsvgRectangle logical_rect;

    g_assert (!rsvg_handle_get_geometry_for_element (handle, "#nonexistent",
                                                     &ink_rect, &logical_rect, &error));
    g_assert_nonnull (error);

    g_clear_error (&error);

    g_assert (rsvg_handle_get_geometry_for_element (handle, "#foo",
                                                    &ink_rect, &logical_rect, &error));
    g_assert_no_error (error);

    g_assert_cmpfloat (ink_rect.x, ==, 0.0);
    g_assert_cmpfloat (ink_rect.y, ==, 0.0);
    g_assert_cmpfloat (ink_rect.width, ==, 40.0);
    g_assert_cmpfloat (ink_rect.height, ==, 50.0);

    g_assert_cmpfloat (logical_rect.x, ==, 5.0);
    g_assert_cmpfloat (logical_rect.y, ==, 5.0);
    g_assert_cmpfloat (logical_rect.width, ==, 30.0);
    g_assert_cmpfloat (logical_rect.height, ==, 40.0);

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 100.0, 100.0, 100.0, 100.0 };

    g_assert (rsvg_handle_render_element (handle, cr, "#foo", &viewport, &error));
    g_assert_no_error (error);

    cairo_destroy (cr);

    cairo_surface_t *expected = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cr = cairo_create (expected);

    cairo_translate (cr, 100.0, 100.0);
    cairo_rectangle (cr, 10.0, 10.0, 60.0, 80.0);
    cairo_set_source_rgba (cr, 0.0, 0.0, 1.0, 1.0);
    cairo_fill_preserve (cr);

    cairo_set_line_width (cr, 20.0);
    cairo_set_source_rgba (cr, 0.0, 0.0, 0.0, 1.0);
    cairo_stroke (cr);

    cairo_destroy (cr);

    cairo_surface_t *diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);

    TestUtilsBufferDiffResult result = {0, 0};
    test_utils_compare_surfaces (output, expected, diff, &result);

    if (result.pixels_changed && result.max_diff > 0) {
        g_test_fail ();
    }

    cairo_surface_destroy (diff);
    cairo_surface_destroy (expected);
    cairo_surface_destroy (output);
    g_object_unref (handle);
}

/* https://gitlab.gnome.org/GNOME/librsvg/issues/385 */
static void
no_write_before_close (void)
{
    RsvgHandle *handle = rsvg_handle_new();
    GError *error = NULL;

    g_assert_false (rsvg_handle_close (handle, &error));
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

    g_assert_true (rsvg_handle_write (handle, &buf, 0, &error));
    g_assert_no_error (error);

    g_assert_false (rsvg_handle_close (handle, &error));
    g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);

    g_error_free (error);

    g_object_unref (handle);
}

static void
cannot_request_external_elements (void)
{
    if (g_test_subprocess ()) {
        /* We want to test that using one of the _sub() functions will fail
         * if the element's id is within an external file.  First, ensure
         * that the main file and the external file actually exist.
         */

        char *filename = get_test_filename ("example.svg");

        RsvgHandle *handle;
        GError *error = NULL;
        RsvgPositionData pos;

        handle = rsvg_handle_new_from_file (filename, &error);
        g_free (filename);

        g_assert_nonnull (handle);
        g_assert_no_error (error);

        g_assert_false (rsvg_handle_get_position_sub (handle, &pos, "dpi.svg#one"));

        g_object_unref (handle);
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_failed ();
    g_test_trap_assert_stderr ("*WARNING*the public API is not allowed to look up external references*");
}

static void
test_flags (RsvgHandleFlags flags)
{
    guint read_flags;

    RsvgHandle *handle = g_object_new (RSVG_TYPE_HANDLE,
                                       "flags", flags,
                                       NULL);
    g_object_get (handle, "flags", &read_flags, NULL);
    g_assert (read_flags == flags);

    g_object_unref (handle);
}

static void
property_flags (void)
{
    test_flags (RSVG_HANDLE_FLAGS_NONE);
    test_flags (RSVG_HANDLE_FLAG_UNLIMITED);
    test_flags (RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA);
    test_flags (RSVG_HANDLE_FLAG_UNLIMITED | RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA);
}

static void
property_dpi (void)
{
    RsvgHandle *handle = g_object_new (RSVG_TYPE_HANDLE,
                                       "dpi-x", 42.0,
                                       "dpi-y", 43.0,
                                       NULL);
    double x, y;

    g_object_get (handle,
                  "dpi-x", &x,
                  "dpi-y", &y,
                  NULL);

    g_assert_cmpfloat (x, ==, 42.0);
    g_assert_cmpfloat (y, ==, 43.0);

    g_object_unref (handle);
}

static void
property_base_uri (void)
{
    RsvgHandle *handle = g_object_new (RSVG_TYPE_HANDLE,
                                       "base-uri", "file:///foo/bar.svg",
                                       NULL);
    char *uri;

    g_object_get (handle,
                  "base-uri", &uri,
                  NULL);

    g_assert_cmpstr (uri, ==, "file:///foo/bar.svg");
    g_free (uri);

    g_object_unref (handle);
}

static void
property_dimensions (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    int width;
    int height;
    double em;
    double ex;

    g_object_get (handle,
                  "width", &width,
                  "height", &height,
                  "em", &em,
                  "ex", &ex,
                  NULL);

    g_assert_cmpint (width,  ==, EXAMPLE_WIDTH);
    g_assert_cmpint (height, ==, EXAMPLE_HEIGHT);

    g_assert_cmpfloat (em, ==, (double) EXAMPLE_WIDTH);
    g_assert_cmpfloat (ex, ==, (double) EXAMPLE_HEIGHT);

    g_object_unref (handle);
}

static void
property_deprecated (void)
{
    char *filename = get_test_filename ("example.svg");
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    char *title;
    char *desc;
    char *metadata;

    g_object_get (handle,
                  "title", &title,
                  "desc", &desc,
                  "metadata", &metadata,
                  NULL);

    g_assert_null (title);
    g_assert_null (desc);
    g_assert_null (metadata);

    g_object_unref (handle);
}

static void
return_if_fail (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle;

        handle = rsvg_handle_new();
        g_assert_nonnull (handle);

        /* NULL is an invalid argument... */
        rsvg_handle_set_base_uri (handle, NULL);
        g_object_unref (handle);
    }

    g_test_trap_subprocess (NULL, 0, 0);
    /* ... and here we catch that it was validated */
    g_test_trap_assert_stderr ("*rsvg_handle_set_base_uri*assertion*failed*");
}

static void
return_if_fail_null_check (void)
{
    if (g_test_subprocess ()) {
        /* Pass NULL as an argument, incorrectly... */
        g_assert_null (rsvg_handle_get_base_uri (NULL));
    }

    g_test_trap_subprocess (NULL, 0, 0);
    /* ... and here we catch that it was validated */
    g_test_trap_assert_stderr ("*rsvg_handle_get_base_uri*assertion*handle*failed*");
}

static void
return_if_fail_type_check (void)
{
    if (g_test_subprocess ()) {
        /* Create a random GObject that is not an RsvgHandle... */
        GInputStream *stream = g_memory_input_stream_new();

        /* Feed it to an RsvgHandle function so it will bail out */
        g_assert_null (rsvg_handle_get_base_uri ((RsvgHandle *) stream));

        g_object_unref (stream);
    }

    g_test_trap_subprocess (NULL, 0, 0);
    /* ... and here we catch that it was validated */
    g_test_trap_assert_stderr ("*rsvg_handle_get_base_uri*assertion*handle*failed*");
}

int
main (int argc, char **argv)
{
    int i;

    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < G_N_ELEMENTS (pixbuf_tests); i++) {
        g_test_add_data_func (pixbuf_tests[i].test_name, &pixbuf_tests[i], test_pixbuf);
    }

    g_test_add_func ("/api/pixbuf_overflow", pixbuf_overflow);

    g_test_add_func ("/api/handle_has_gtype", handle_has_gtype);
    g_test_add_func ("/api/flags_registration", flags_registration);
    g_test_add_func ("/api/error_registration", error_registration);
    g_test_add_func ("/api/noops", noops);
    g_test_add_func ("/api/set_dpi", set_dpi);
    g_test_add_func ("/api/base_uri", base_uri);
    g_test_add_func ("/api/base_gfile", base_gfile);
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
    g_test_add_func ("/api/set_size_callback", set_size_callback);
    g_test_add_func ("/api/reset_size_callback", reset_size_callback);
    g_test_add_func ("/api/render_with_zero_size_callback", render_with_zero_size_callback);
    g_test_add_func ("/api/get_pixbuf_with_size_callback", get_pixbuf_with_size_callback);
    g_test_add_func ("/api/detects_cairo_context_in_error", detects_cairo_context_in_error);
    g_test_add_func ("/api/can_draw_to_non_image_surface", can_draw_to_non_image_surface);
    g_test_add_func ("/api/render_cairo_sub", render_cairo_sub);
    g_test_add_func ("/api/get_intrinsic_dimensions", get_intrinsic_dimensions);
    g_test_add_func ("/api/render_document", render_document);
    g_test_add_func ("/api/get_geometry_for_layer", get_geometry_for_layer);
    g_test_add_func ("/api/render_layer", render_layer);
    g_test_add_func ("/api/untransformed_element", untransformed_element);
    g_test_add_func ("/api/no_write_before_close", no_write_before_close);
    g_test_add_func ("/api/empty_write_close", empty_write_close);
    g_test_add_func ("/api/cannot_request_external_elements", cannot_request_external_elements);
    g_test_add_func ("/api/property_flags", property_flags);
    g_test_add_func ("/api/property_dpi", property_dpi);
    g_test_add_func ("/api/property_base_uri", property_base_uri);
    g_test_add_func ("/api/property_dimensions", property_dimensions);
    g_test_add_func ("/api/property_deprecated", property_deprecated);
    g_test_add_func ("/api/return_if_fail", return_if_fail);
    g_test_add_func ("/api/return_if_fail_null_check", return_if_fail_null_check);
    g_test_add_func ("/api/return_if_fail_type_check", return_if_fail_type_check);

    return g_test_run ();
}
