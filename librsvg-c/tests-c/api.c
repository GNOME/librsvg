/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

/* These are the C API tests for librsvg.  These test the complete C
 * API, especially its historical peculiarities to ensure ABI
 * compatibility.
 *
 * These tests are not meant to exhaustively test librsvg's features.
 * For those, you should look at the Rust integration tests.  See
 * tests/README.md for details.
 */

#include <stdio.h>
#include <glib.h>
#include <cairo.h>

#define RSVG_DISABLE_DEPRECATION_WARNINGS /* so we can test deprecated API */
#include <librsvg/rsvg.h>
#include "test-utils.h"

/*
  Untested:
  rsvg_handle_internal_set_testing
*/

static void
handle_has_correct_type_info (void)
{
    GTypeQuery q;
    RsvgHandle *handle;

    g_type_query (RSVG_TYPE_HANDLE, &q);
    g_assert (q.type == RSVG_TYPE_HANDLE);
    g_assert (q.type == rsvg_handle_get_type ());

    g_assert_cmpstr (q.type_name, ==, "RsvgHandle");

    /* These test that the sizes of the structs in the header file actually match the
     * sizes of structs and the glib-subclass machinery in the Rust side.
     */
    g_assert (sizeof (RsvgHandleClass) == (gsize) q.class_size);
    g_assert (sizeof (RsvgHandle) == (gsize) q.instance_size);

    handle = rsvg_handle_new();
    g_assert (G_OBJECT_TYPE (handle) == RSVG_TYPE_HANDLE);
    g_object_unref (handle);
}

static void
assert_flags_value_matches (GFlagsValue *v,
                            guint value,
                            const char *value_name,
                            const char *value_nick)
{
    g_assert_cmpint(v->value, ==, value);
    g_assert_cmpstr(v->value_name, ==, value_name);
    g_assert_cmpstr(v->value_nick, ==, value_nick);
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

    assert_flags_value_matches(&flags_class->values[0],
                               RSVG_HANDLE_FLAGS_NONE,
                               "RSVG_HANDLE_FLAGS_NONE",
                               "flags-none");

    assert_flags_value_matches(&flags_class->values[1],
                               RSVG_HANDLE_FLAG_UNLIMITED,
                               "RSVG_HANDLE_FLAG_UNLIMITED",
                               "flag-unlimited");

    assert_flags_value_matches(&flags_class->values[2],
                               RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA,
                               "RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA",
                               "flag-keep-image-data");

    g_type_class_unref (type_class);
}

static void
assert_enum_value_matches (GEnumValue *v,
                           gint value,
                           const char *value_name,
                           const char *value_nick)
{
    g_assert_cmpint (v->value, ==, value);
    g_assert_cmpstr (v->value_name, ==, value_name);
    g_assert_cmpstr (v->value_nick, ==, value_nick);
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

    assert_enum_value_matches (&enum_class->values[0],
                               RSVG_ERROR_FAILED,
                               "RSVG_ERROR_FAILED",
                               "failed");

    g_type_class_unref (type_class);
}

static char *
get_test_filename (const char *basename) {
    return g_build_filename (test_utils_get_test_data_path (),
                             "api",
                             basename,
                             NULL);
}

static RsvgHandle *
load_test_document (const char *basename) {
    char *filename = get_test_filename (basename);
    GError *error = NULL;

    RsvgHandle *handle = rsvg_handle_new_from_file (filename, &error);
    g_free (filename);

    g_assert_nonnull (handle);
    g_assert_no_error (error);

    return handle;
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

#ifdef HAVE_PIXBUF
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
#endif /* defined(HAVE_PIXBUF) */

static void
noops (void)
{
    /* Just to test that these functions are present in the binary, I guess */
    rsvg_init ();
    rsvg_term ();
    rsvg_cleanup ();
}

static void
noops_return_null (void)
{
    RsvgHandle *handle = rsvg_handle_new ();

    g_assert_null (rsvg_handle_get_title (handle));
    g_assert_null (rsvg_handle_get_desc (handle));
    g_assert_null (rsvg_handle_get_metadata (handle));

    g_object_unref (handle);
}

static void
set_dpi (void)
{
    RsvgHandle *handle;
    RsvgDimensionData dim;

    rsvg_set_default_dpi (100.0);

    handle = load_test_document ("dpi.svg");

    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 100);
    g_assert_cmpint (dim.height, ==, 400);

    rsvg_handle_set_dpi (handle, 200.0);
    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 200);
    g_assert_cmpint (dim.height, ==, 800);
    g_object_unref (handle);

    handle = load_test_document ("dpi.svg");

    rsvg_handle_set_dpi_x_y (handle, 400.0, 300.0);
    rsvg_handle_get_dimensions (handle, &dim);
    g_assert_cmpint (dim.width,  ==, 400);
    g_assert_cmpint (dim.height, ==, 1200);
    g_object_unref (handle);
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

    /* Test that close() is idempotent in the happy case */
    g_assert (rsvg_handle_close (handle, &error));
    g_assert_no_error (error);

    rsvg_handle_free (handle);
    g_free (data);
}

static void
handle_new_from_file (void)
{
    char *filename = get_test_filename ("dpi.svg");
    char *abs_path = g_canonicalize_filename(filename, NULL);
    char *uri = g_strconcat ("file://", abs_path, NULL);

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
    g_free (abs_path);
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
    RsvgHandle *handle = load_test_document ("example.svg");

    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_ONE_ID));
    g_assert (rsvg_handle_has_sub (handle, EXAMPLE_TWO_ID));
    g_assert (!rsvg_handle_has_sub (handle, "#foo"));

    g_object_unref (handle);
}

#ifdef HAVE_PIXBUF
static void
test_get_pixbuf (gboolean sub)
{
    RsvgHandle *handle = load_test_document ("example.svg");

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

/* Test that calling rsvg_handle_get_pixbuf() will produce a g_warning if there is a rendering error.
 * This is for the benefit of the C-based gdk-pixbuf loader, which uses rsvg_handle_get_pixbuf() --- with
 * the warning, calling code will at least have a clue that something went wrong, since that function
 * does not return a GError.
 */
static void
handle_get_pixbuf_produces_g_warning (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = load_test_document ("too-big.svg");

        GdkPixbuf *pixbuf = rsvg_handle_get_pixbuf (handle);
        g_assert_null (pixbuf);
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_stderr ("*WARNING*could not render*");
}
#endif /* defined(HAVE_PIXBUF) */

static void
dimensions_and_position (void)
{
    RsvgHandle *handle = load_test_document ("example.svg");
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

    /* Asking for "position of the whole SVG" (id=NULL) always returns (0, 0) */
    g_assert (rsvg_handle_get_position_sub (handle, &pos, NULL));
    g_assert_cmpint (pos.x, ==, 0);
    g_assert_cmpint (pos.y, ==, 0);

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
    RsvgHandle *handle;
    struct size_func_data data;
    RsvgDimensionData dim;

    handle = load_test_document ("example.svg");

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
    RsvgHandle *handle;
    struct size_func_data data_1;
    struct size_func_data data_2;

    handle = load_test_document ("example.svg");

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

#ifdef HAVE_PIXBUF
static void
zero_size_func (gint *width, gint *height, gpointer user_data)
{
    (void) user_data;

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
    RsvgHandle *handle;
    GdkPixbuf *pixbuf;

    handle = load_test_document ("example.svg");

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
    (void) user_data;

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
#endif /* defined(HAVE_PIXBUF) */

static void
detects_cairo_context_in_error (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = load_test_document ("example.svg");

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

    RsvgHandle *handle = load_test_document ("example.svg");

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
    RsvgHandle *handle = load_test_document ("bug334-element-positions.svg");

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
    RsvgHandle *handle = load_test_document ("example.svg");

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
get_intrinsic_dimensions_missing_values (void)
{
    RsvgHandle *handle = load_test_document ("no-viewbox.svg");

    gboolean has_width;
    RsvgLength width;
    gboolean has_height;
    RsvgLength height;
    gboolean has_viewbox;
    RsvgRectangle viewbox;

    rsvg_handle_get_intrinsic_dimensions (handle, &has_width, &width, &has_height, &height, &has_viewbox, &viewbox);
    g_assert_true (has_width);
    g_assert_true (has_height);
    g_assert_false (has_viewbox);
    g_object_unref (handle);
}

static void
get_intrinsic_size_in_pixels_yes (void)
{
    RsvgHandle *handle = load_test_document ("size.svg");
    gdouble width, height;

    rsvg_handle_set_dpi (handle, 96.0);

    /* Test optional parameters */
    g_assert (rsvg_handle_get_intrinsic_size_in_pixels (handle, NULL, NULL));

    /* Test the actual result */
    g_assert (rsvg_handle_get_intrinsic_size_in_pixels (handle, &width, &height));
    g_assert_cmpfloat (width, ==, 192.0);
    g_assert_cmpfloat (height, ==, 288.0);

    g_object_unref (handle);
}

static void
get_intrinsic_size_in_pixels_no (void)
{
    RsvgHandle *handle = load_test_document ("no-size.svg");
    gdouble width, height;

    rsvg_handle_set_dpi (handle, 96.0);
    g_assert (!rsvg_handle_get_intrinsic_size_in_pixels (handle, &width, &height));
    g_assert_cmpfloat (width, ==, 0.0);
    g_assert_cmpfloat (height, ==, 0.0);

    g_object_unref (handle);
}

static void
set_stylesheet (void)
{
    const char *css = "rect { fill: #00ff00; }";

    RsvgHandle *handle = load_test_document ("stylesheet.svg");
    RsvgHandle *ref_handle = load_test_document ("stylesheet-ref.svg");

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 100, 100);
    cairo_surface_t *reference = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 100, 100);

    RsvgRectangle viewport = { 0.0, 0.0, 100.0, 100.0 };

    cairo_t *output_cr = cairo_create (output);
    cairo_t *ref_cr = cairo_create (reference);

    GError *error = NULL;
    g_assert (rsvg_handle_set_stylesheet (handle, (const guint8 *) css, strlen (css), &error));
    g_assert_no_error (error);

    g_assert (rsvg_handle_render_document (handle, output_cr, &viewport, &error));
    g_assert_no_error (error);

    g_assert (rsvg_handle_render_document (ref_handle, ref_cr, &viewport, &error));
    g_assert_no_error (error);

    cairo_surface_t *diff = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 100, 100);

    TestUtilsBufferDiffResult result = {0, 0};
    test_utils_compare_surfaces (output, reference, diff, &result);

    if (result.pixels_changed && result.max_diff > 0) {
        g_test_fail ();
    }

    cairo_surface_destroy (diff);
    cairo_destroy (ref_cr);
    cairo_destroy (output_cr);
    cairo_surface_destroy (reference);
    cairo_surface_destroy (output);
    g_object_unref (ref_handle);
    g_object_unref (handle);
}

static void
render_document (void)
{
    RsvgHandle *handle = load_test_document ("document.svg");

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 150, 150);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 50.0, 50.0, 50.0, 50.0 };

    GError *error = NULL;
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
    RsvgHandle *handle = load_test_document ("geometry.svg");

    RsvgRectangle viewport = { 0.0, 0.0, 100.0, 400.0 };
    RsvgRectangle ink_rect;
    RsvgRectangle logical_rect;

    GError *error = NULL;

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
    RsvgHandle *handle = load_test_document ("layers.svg");

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 100.0, 100.0, 100.0, 100.0 };

    GError *error = NULL;

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
set_cancellable_for_rendering (void)
{
    RsvgHandle *handle = load_test_document ("layers.svg");

    cairo_surface_t *output = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 300, 300);
    cairo_t *cr = cairo_create (output);

    RsvgRectangle viewport = { 100.0, 100.0, 100.0, 100.0 };

    GError *error = NULL;

    /* Same as in the Rust API test, we cancel immediately and then start rendering. */
    GCancellable *cancellable = g_cancellable_new ();
    g_cancellable_cancel (cancellable);

    rsvg_handle_set_cancellable_for_rendering (handle, cancellable);

    g_assert_false (rsvg_handle_render_layer (handle, cr, "#bar", &viewport, &error));
    g_assert_error (error, G_IO_ERROR, G_IO_ERROR_CANCELLED);

    cairo_destroy (cr);
    cairo_surface_destroy (output);
    g_object_unref (handle);
}

static void
untransformed_element (void)
{
    RsvgHandle *handle = load_test_document ("geometry-element.svg");

    RsvgRectangle ink_rect;
    RsvgRectangle logical_rect;

    GError *error = NULL;

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
    error = NULL;

    /* Test that close() is idempotent in the error case */
    g_assert (rsvg_handle_close (handle, &error));
    g_assert_no_error (error);

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

/* Test that trying to render a handle that has not been loaded yet results in a g_critical */
static void
ordering_render_before_load (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = rsvg_handle_new ();

        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 10, 10);
        cairo_t *cr = cairo_create (surf);

        g_assert_false (rsvg_handle_render_cairo (handle, cr));

        return;
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_failed ();
    g_test_trap_assert_stderr ("*CRITICAL*Handle has not been loaded*");
}

/* Test that trying to render a handle that is in the middle of loading results in a g_critical */
static void
ordering_render_while_loading (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = rsvg_handle_new ();
        GError *error = NULL;
        guchar buf = '<'; /* as if we started writing some XML */

        /* push a single byte to the handle to start its loading process */
        g_assert_true (rsvg_handle_write (handle, &buf, 1, &error));
        g_assert_no_error (error);

        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 10, 10);
        cairo_t *cr = cairo_create (surf);

        g_assert_false (rsvg_handle_render_cairo (handle, cr));

        return;
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_failed ();
    g_test_trap_assert_stderr ("*CRITICAL*Handle is still loading*");
}

/* Test that trying to render a handle that was closed with an error results in a g_critical */
static void
rendering_after_close_error (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = rsvg_handle_new ();
        GError *error = NULL;
        guchar buf = 0;

        /* Write nothing to start the loading process */
        g_assert_true (rsvg_handle_write (handle, &buf, 0, &error));
        g_assert_no_error (error);

        /* Close the handle */
        g_assert_false (rsvg_handle_close (handle, &error));
        g_assert_error (error, RSVG_ERROR, RSVG_ERROR_FAILED);

        g_error_free (error);

        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 10, 10);
        cairo_t *cr = cairo_create (surf);

        g_assert_false (rsvg_handle_render_cairo (handle, cr));

        return;
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_failed ();
    g_test_trap_assert_stderr ("*CRITICAL*did you check for errors during the loading stage*");
}

static void
render_cairo_produces_g_warning (void)
{
    if (g_test_subprocess ()) {
        RsvgHandle *handle = load_test_document ("instancing-limit.svg");

        cairo_surface_t *surf = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, 10, 10);
        cairo_t *cr = cairo_create (surf);

        g_assert_false (rsvg_handle_render_cairo (handle, cr));
        return;
    }

    g_test_trap_subprocess (NULL, 0, 0);
    g_test_trap_assert_stderr ("*WARNING*exceeded*");
}

static void
cannot_request_external_elements (void)
{
    /* We want to test that using one of the _sub() functions will fail
     * if the element's id is within an external file.
     */

    RsvgHandle *handle = load_test_document ("example.svg");
    RsvgPositionData pos;

    g_assert_false (rsvg_handle_get_position_sub (handle, &pos, "dpi.svg#one"));

    g_object_unref (handle);
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
    RsvgHandle *handle = load_test_document ("example.svg");

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
    RsvgHandle *handle = load_test_document ("example.svg");

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

static void
library_version_defines (void)
{
    gchar *version = g_strdup_printf ("%u.%u.%u",
                                      LIBRSVG_MAJOR_VERSION, LIBRSVG_MINOR_VERSION, LIBRSVG_MICRO_VERSION);
    g_assert_cmpstr (version, ==, LIBRSVG_VERSION);
    g_free (version);
}

static void
library_version_check (void)
{
    g_assert_true(LIBRSVG_CHECK_VERSION(1, 99, 9));
    g_assert_true(LIBRSVG_CHECK_VERSION(2, 0, 0));
    g_assert_true(LIBRSVG_CHECK_VERSION(2, 50, 7));
    g_assert_false(LIBRSVG_CHECK_VERSION(2, 99, 0));
    g_assert_false(LIBRSVG_CHECK_VERSION(3, 0, 0));
}

static void
library_version_constants (void)
{
    g_assert_cmpuint (rsvg_major_version, ==, LIBRSVG_MAJOR_VERSION);
    g_assert_cmpuint (rsvg_minor_version, ==, LIBRSVG_MINOR_VERSION);
    g_assert_cmpuint (rsvg_micro_version, ==, LIBRSVG_MICRO_VERSION);
}

typedef struct
{
    const gchar *test_name;
    const gchar *file_path;
    const gchar *id;
    gdouble x;
    gdouble y;
    gdouble width;
    gdouble height;
    gboolean has_position;
    gboolean has_dimensions;
} DimensionsFixtureData;

static void
test_dimensions (gconstpointer user_data)
{
    const DimensionsFixtureData *fixture = user_data;
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
        g_assert (rsvg_handle_has_sub (handle, fixture->id));
        g_assert (rsvg_handle_get_position_sub (handle, &position, fixture->id));
        g_assert (rsvg_handle_get_dimensions_sub (handle, &dimension, fixture->id));
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

static DimensionsFixtureData dimensions_fixtures[] =
{
    {
        "/dimensions/viewbox_only",
        "dimensions/bug608102.svg",
        NULL,
        0, 0, 16, 16,
        FALSE, TRUE
    },
    {
        "/dimensions/hundred_percent_width_and_height",
        "dimensions/bug612951.svg",
        NULL,
        0, 0, 47, 47.14,
        FALSE, TRUE
    },
    {
        "/dimensions/viewbox_only_2",
        "dimensions/bug614018.svg",
        NULL,
        0, 0, 972, 546,
        FALSE, TRUE
    },
    {
        "/dimensions/sub/rect_no_unit",
        "dimensions/sub-rect-no-unit.svg",
        "#rect-no-unit",
        0, 0, 44, 45,
        FALSE, TRUE
    },
    {
        "/dimensions/with_viewbox",
        "dimensions/bug521-with-viewbox.svg",
        "#foo",
        50.0, 60.0, 70.0, 80.0,
        TRUE, TRUE
    },
    {
        "/dimensions/sub/823",
        "dimensions/bug823-position-sub.svg",
        "#pad_width",
        444.0, 139.0, 0.0, 0.0,
        TRUE, FALSE
    },
};

typedef struct
{
    const char *test_name;
    const char *fixture;
    size_t buf_size;
} LoadingTestData;

static void
load_n_bytes_at_a_time (gconstpointer data)
{
    const LoadingTestData *fixture_data = data;
    char *filename = g_build_filename (test_utils_get_test_data_path (), fixture_data->fixture, NULL);
    guchar *buf = g_new (guchar, fixture_data->buf_size);
    gboolean done;

    RsvgHandle *handle;
    FILE *file;

    file = fopen (filename, "rb");
    g_assert_nonnull (file);

    handle = rsvg_handle_new_with_flags (RSVG_HANDLE_FLAGS_NONE);

    done = FALSE;

    do {
        size_t num_read;

        num_read = fread (buf, 1, fixture_data->buf_size, file);

        if (num_read > 0) {
            g_assert_true (rsvg_handle_write (handle, buf, num_read, NULL));
        } else {
            g_assert_cmpint (ferror (file), ==, 0);

            if (feof (file)) {
                done = TRUE;
            }
        }
    } while (!done);

    fclose (file);
    g_free (filename);

    g_assert_true (rsvg_handle_close (handle, NULL));

    g_object_unref (handle);

    g_free (buf);
}

static LoadingTestData loading_tests[] = {
    { "/loading/one-byte-at-a-time", "loading/gnome-cool.svg", 1 },
    { "/loading/compressed-one-byte-at-a-time", "loading/gnome-cool.svgz", 1 },
    { "/loading/compressed-two-bytes-at-a-time", "loading/gnome-cool.svgz", 2 } /* to test reading the entire gzip header */
};

#ifdef HAVE_PIXBUF
static void
add_pixbuf_tests (void)
{
    gsize i;

    /* Tests for rsvg_handle_get_pixbuf() and rsvg_handle_get_pixbuf_sub() */
    g_test_add_func ("/api/handle_get_pixbuf", handle_get_pixbuf);
    g_test_add_func ("/api/handle_get_pixbuf_sub", handle_get_pixbuf_sub);
    g_test_add_func ("/api/handle_get_pixbuf_produces_g_warning", handle_get_pixbuf_produces_g_warning);
    g_test_add_func ("/api/get_pixbuf_with_size_callback", get_pixbuf_with_size_callback);

    /* Tests for the deprecated GdkPixbuf-based API */
    for (i = 0; i < G_N_ELEMENTS (pixbuf_tests); i++) {
        g_test_add_data_func (pixbuf_tests[i].test_name, &pixbuf_tests[i], test_pixbuf);
    }

    g_test_add_func ("/api/pixbuf_overflow", pixbuf_overflow);
}
#endif /* defined(HAVE_PIXBUF) */

/* Tests for the C API of librsvg*/
static void
add_api_tests (void)
{
    g_test_add_func ("/api/handle_has_correct_type_info", handle_has_correct_type_info);
    g_test_add_func ("/api/flags_registration", flags_registration);
    g_test_add_func ("/api/error_registration", error_registration);
    g_test_add_func ("/api/noops", noops);
    g_test_add_func ("/api/noops_return_null", noops_return_null);
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
    g_test_add_func ("/api/dimensions_and_position", dimensions_and_position);
    g_test_add_func ("/api/set_size_callback", set_size_callback);
    g_test_add_func ("/api/reset_size_callback", reset_size_callback);
#ifdef HAVE_PIXBUF
    g_test_add_func ("/api/render_with_zero_size_callback", render_with_zero_size_callback);
#endif
    g_test_add_func ("/api/detects_cairo_context_in_error", detects_cairo_context_in_error);
    g_test_add_func ("/api/can_draw_to_non_image_surface", can_draw_to_non_image_surface);
    g_test_add_func ("/api/render_cairo_sub", render_cairo_sub);
    g_test_add_func ("/api/get_intrinsic_dimensions", get_intrinsic_dimensions);
    g_test_add_func ("/api/get_intrinsic_dimensions_missing_values", get_intrinsic_dimensions_missing_values);
    g_test_add_func ("/api/get_intrinsic_size_in_pixels/yes", get_intrinsic_size_in_pixels_yes);
    g_test_add_func ("/api/get_intrinsic_size_in_pixels/no", get_intrinsic_size_in_pixels_no);
    g_test_add_func ("/api/set_stylesheet", set_stylesheet);
    g_test_add_func ("/api/render_document", render_document);
    g_test_add_func ("/api/get_geometry_for_layer", get_geometry_for_layer);
    g_test_add_func ("/api/render_layer", render_layer);
    g_test_add_func ("/api/set_cancellable_for_rendering", set_cancellable_for_rendering);
    g_test_add_func ("/api/untransformed_element", untransformed_element);
    g_test_add_func ("/api/no_write_before_close", no_write_before_close);
    g_test_add_func ("/api/empty_write_close", empty_write_close);
    g_test_add_func ("/api/ordering_render_before_load", ordering_render_before_load);
    g_test_add_func ("/api/ordering_render_while_loading", ordering_render_while_loading);
    g_test_add_func ("/api/rendering_after_close_error", rendering_after_close_error);
    g_test_add_func ("/api/render_cairo_produces_g_warning", render_cairo_produces_g_warning);
    g_test_add_func ("/api/cannot_request_external_elements", cannot_request_external_elements);
    g_test_add_func ("/api/property_flags", property_flags);
    g_test_add_func ("/api/property_dpi", property_dpi);
    g_test_add_func ("/api/property_base_uri", property_base_uri);
    g_test_add_func ("/api/property_dimensions", property_dimensions);
    g_test_add_func ("/api/property_deprecated", property_deprecated);
    g_test_add_func ("/api/return_if_fail", return_if_fail);
    g_test_add_func ("/api/return_if_fail_null_check", return_if_fail_null_check);
    g_test_add_func ("/api/return_if_fail_type_check", return_if_fail_type_check);
    g_test_add_func ("/api/library_version_defines", library_version_defines);
    g_test_add_func ("/api/library_version_check", library_version_check);
    g_test_add_func ("/api/library_version_constants", library_version_constants);
}

/* Tests for the deprecated APIs to get geometries */
static void
add_geometry_tests (void)
{
    gsize i;

    for (i = 0; i < G_N_ELEMENTS (dimensions_fixtures); i++)
        g_test_add_data_func (dimensions_fixtures[i].test_name, &dimensions_fixtures[i], test_dimensions);
}

/* Tests for the deprecated API for loading bytes at a time */
static void
add_loading_tests (void)
{
    gsize i;

    for (i = 0; i < G_N_ELEMENTS (loading_tests); i++) {
        g_test_add_data_func (loading_tests[i].test_name, &loading_tests[i], load_n_bytes_at_a_time);
    }
}

int
main (int argc, char **argv)
{
    g_test_init (&argc, &argv, NULL);

    test_utils_print_dependency_versions ();

#ifdef HAVE_PIXBUF
    add_pixbuf_tests ();
#endif
    add_api_tests ();
    add_geometry_tests ();
    add_loading_tests ();

    return g_test_run ();
}
