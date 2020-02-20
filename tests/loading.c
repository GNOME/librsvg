/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#define RSVG_DISABLE_DEPRECATION_WARNINGS

#include "config.h"

#include <stdio.h>
#include <glib.h>
#include "librsvg/rsvg.h"
#include "test-utils.h"

typedef struct
{
    const char *test_name;
    const char *fixture;
    size_t buf_size;
} TestData;

static void
load_n_bytes_at_a_time (gconstpointer data)
{
    const TestData *fixture_data = data;
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

static TestData tests[] = {
    { "/loading/one-byte-at-a-time", "loading/gnome-cool.svg", 1 },
    { "/loading/compressed-one-byte-at-a-time", "loading/gnome-cool.svgz", 1 },
    { "/loading/compressed-two-bytes-at-a-time", "loading/gnome-cool.svgz", 2 } /* to test reading the entire gzip header */
};

int
main (int argc, char **argv)
{
    int result;
    int i;

    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < G_N_ELEMENTS (tests); i++) {
        g_test_add_data_func (tests[i].test_name, &tests[i], load_n_bytes_at_a_time);
    }

    result = g_test_run ();

    return result;
}
