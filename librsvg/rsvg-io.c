/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
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

/* Defined in rsvg_internals/src/io.rs */
extern char *
rsvg_decode_data_uri (const char *uri,
                      char **out_mime_type,
                      gsize *out_len,
                      GError **error);

static char *
rsvg_acquire_gvfs_data (const char *uri,
                        char **out_mime_type,
                        gsize *out_len,
                        GCancellable *cancellable,
                        GError **error)
{
    GFile *file;
    char *data;
    gsize len;
    char *content_type;
    gboolean res;

    file = g_file_new_for_uri (uri);
    data = NULL;

    res = g_file_load_contents (file, cancellable, &data, &len, NULL, error);
    g_object_unref (file);

    if (!res) {
        return NULL;
    }

    if (out_mime_type &&
        (content_type = g_content_type_guess (uri, (guchar *) data, len, NULL))) {
        *out_mime_type = g_content_type_get_mime_type (content_type);
        g_free (content_type);
    }

    *out_len = len;
    return data;
}

char *
_rsvg_io_acquire_data (const char *uri,
                       char **mime_type,
                       gsize *len,
                       GCancellable *cancellable,
                       GError **error)
{
    char *data;
    gsize llen;

    if (!len)
        len = &llen;

    if (strncmp (uri, "data:", 5) == 0)
      return rsvg_decode_data_uri (uri, mime_type, len, error);

    if ((data = rsvg_acquire_gvfs_data (uri, mime_type, len, cancellable, error)))
      return data;

    return NULL;
}
