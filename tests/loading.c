/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>
#include "rsvg.h"
#include "rsvg-compat.h"
#include "test-utils.h"

static void
load_one_byte_at_a_time (gconstpointer data)
{
    const char *fixture = data;
    char *filename = g_build_filename (test_utils_get_test_data_path (), fixture, NULL);
    guchar buf[1];
    gboolean done;

    RsvgHandle *handle;
    FILE *file;

    file = fopen (filename, "rb");
    g_assert (file != NULL);

    handle = rsvg_handle_new_with_flags (RSVG_HANDLE_FLAGS_NONE);

    done = FALSE;

    do {
        if (fread (buf, 1, 1, file) == 1) {
            g_assert (rsvg_handle_write (handle, buf, 1, NULL) != FALSE);
        } else {
            g_assert (ferror (file) == 0);

            if (feof (file)) {
                done = TRUE;
            }
        }
    } while (!done);

    g_assert (rsvg_handle_close (handle, NULL) != FALSE);

    g_object_unref (handle);
}

int
main (int argc, char **argv)
{
    int result;

    RSVG_G_TYPE_INIT;
    g_test_init (&argc, &argv, NULL);

    g_test_add_data_func ("/load-one-byte-at-a-time", "loading/gnome-cool.svg", load_one_byte_at_a_time);

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
