/*
   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>
   Copyright © 2011, 2012 Christian Persch

   This program is free software; you can redistribute it and/or
   modify it under the terms of the GNU Library General Public License as
   published by the Free Software Foundation; either version 2 of the
   License, or (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Library General Public License for more details.

   You should have received a copy of the GNU Library General Public
   License along with this program; if not, write to the
   Free Software Foundation, Inc., 59 Temple Place - Suite 330,
   Boston, MA 02111-1307, USA.
*/

#include "config.h"

#include "rsvg-io.h"
#include "rsvg-private.h"

#include <string.h>

static guint8 *
rsvg_acquire_base64_data (const char *data, 
                          const char *base_uri, 
                          gsize *len,
                          GError **error)
{
    guint8 *bytes;
    gsize data_len, written_len;
    int state = 0;
    guint save = 0;

    /* FIXME: be more correct! Check that is indeed a base64 data: URI */
    while (*data)
        if (*data++ == ',')
            break;

    data_len = strlen (data);
    bytes = g_try_malloc (data_len / 4 * 3);
    if (bytes == NULL)
        return NULL;

    written_len = g_base64_decode_step (data, data_len, bytes, &state, &save);

    *len = written_len;

    return bytes;
}

gchar *
rsvg_get_file_path (const gchar * filename, const gchar * base_uri)
{
    gchar *absolute_filename;

    if (g_file_test (filename, G_FILE_TEST_EXISTS) || g_path_is_absolute (filename)) {
        absolute_filename = g_strdup (filename);
    } else {
        gchar *tmpcdir;
        gchar *base_filename;

        if (base_uri) {
            base_filename = g_filename_from_uri (base_uri, NULL, NULL);
            if (base_filename != NULL) {
                tmpcdir = g_path_get_dirname (base_filename);
                g_free (base_filename);
            } else 
                return NULL;
        } else
            tmpcdir = g_get_current_dir ();

        absolute_filename = g_build_filename (tmpcdir, filename, NULL);
        g_free (tmpcdir);
    }

    return absolute_filename;
}

static guint8 *
rsvg_acquire_file_data (const char *filename,
                        const char *base_uri,
                        gsize *len,
                        GError **error)
{
    GFile *file;
    gchar *path, *data;
    GInputStream *stream;
    gboolean res;

    rsvg_return_val_if_fail (filename != NULL, NULL, error);

    path = rsvg_get_file_path (filename, base_uri);
    if (path == NULL)
        return NULL;

    res = g_file_get_contents (path, &data, len, error);
    g_free (path);

    return res ? data : NULL;
}

static GInputStream *
rsvg_acquire_gvfs_stream (const char *uri, 
                          const char *base_uri, 
                          GError **error)
{
    GFile *base, *file;
    GInputStream *stream;
    GError *err = NULL;
    gchar *data;

    file = g_file_new_for_uri (uri);

    stream = (GInputStream *) g_file_read (file, NULL /* cancellable */, &err);
    g_object_unref (file);

    if (stream == NULL &&
        g_error_matches (err, G_IO_ERROR, G_IO_ERROR_NOT_FOUND)) {
        g_clear_error (&err);

        base = g_file_new_for_uri (base_uri);
        file = g_file_resolve_relative_path (base, uri);
        g_object_unref (base);

        stream = (GInputStream *) g_file_read (file, NULL /* cancellable */, &err);
        g_object_unref (file);
    }

    if (stream == NULL)
        g_propagate_error (error, err);

    return stream;
}

static guint8 *
rsvg_acquire_gvfs_data (const char *uri, 
                        const char *base_uri, 
                        gsize *len,
                        GError **error)
{
    GFile *base, *file;
    GInputStream *stream;
    GError *err;
    gchar *data;
    gboolean res;

    file = g_file_new_for_uri (uri);

    err = NULL;
    data = NULL;
    if (!(res = g_file_load_contents (file, NULL, &data, len, NULL, &err)) &&
        g_error_matches (err, G_IO_ERROR, G_IO_ERROR_NOT_FOUND) &&
        base_uri != NULL) {
        g_clear_error (&err);

        base = g_file_new_for_uri (base_uri);
        file = g_file_resolve_relative_path (base, uri);
        g_object_unref (base);

        res = g_file_load_contents (file, NULL, &data, len, NULL, &err);
    }

    g_object_unref (file);

    if (err == NULL)
        return data;

    g_propagate_error (error, err);
    return NULL;
}

guint8 *
_rsvg_io_acquire_data (const char *href, 
                       const char *base_uri, 
                       gsize *len,
                       GError **error)
{
    guint8 *data;

    if (!(href && *href)) {
        g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_FAILED,
                            "Invalid URI");
        return NULL;
    }

    if (strncmp (href, "data:", 5) == 0)
      return rsvg_acquire_base64_data (href, NULL, len, error);

    if ((data = rsvg_acquire_file_data (href, base_uri, len, NULL)))
      return data;

    if ((data = rsvg_acquire_gvfs_data (href, base_uri, len, error)))
      return data;

    return NULL;
}

GInputStream *
_rsvg_io_acquire_stream (const char *href, 
                         const char *base_uri, 
                         GError **error)
{
    GInputStream *stream;
    guint8 *data;
    gsize len;

    if (!(href && *href)) {
        g_set_error_literal (error, G_IO_ERROR, G_IO_ERROR_FAILED,
                            "Invalid URI");
        return NULL;
    }

    if (strncmp (href, "data:", 5) == 0) {
        if (!(data = rsvg_acquire_base64_data (href, NULL, &len, error)))
            return NULL;

        return g_memory_input_stream_new_from_data (data, len, (GDestroyNotify) g_free);
    }

    if ((data = rsvg_acquire_file_data (href, base_uri, &len, NULL)))
      return g_memory_input_stream_new_from_data (data, len, (GDestroyNotify) g_free);

    if ((stream = rsvg_acquire_gvfs_stream (href, base_uri, error)))
      return stream;

    return NULL;
}
