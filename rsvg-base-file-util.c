/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-file-util.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

   Author: Raph Levien <raph@artofcode.com>
*/

#include "config.h"
#include "rsvg.h"
#include "rsvg-private.h"

static gboolean
rsvg_handle_fill_with_data (RsvgHandle * handle,
                            const guint8 * data, gsize data_len, GError ** error)
{
    rsvg_return_val_if_fail (data != NULL, FALSE, error);
    rsvg_return_val_if_fail (data_len != 0, FALSE, error);

    if (!rsvg_handle_write (handle, data, data_len, error))
        return FALSE;
    if (!rsvg_handle_close (handle, error))
        return FALSE;

    return TRUE;
}

/**
 * rsvg_handle_new_from_data:
 * @data: The SVG data
 * @data_len: The length of #data, in bytes
 * @error: return location for errors
 *
 * Loads the SVG specified by #data.
 *
 * Returns: A RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_data (const guint8 * data, gsize data_len, GError ** error)
{
    RsvgHandle *handle;

    handle = rsvg_handle_new ();

    if (handle) {
        if (!rsvg_handle_fill_with_data (handle, data, data_len, error)) {
            g_object_unref (G_OBJECT (handle));
            handle = NULL;
        }
    }

    return handle;
}

/**
 * rsvg_handle_new_from_file:
 * @file_name: The file name to load. If built with gnome-vfs, can be a URI.
 * @error: return location for errors
 *
 * Loads the SVG specified by #file_name.
 *
 * Returns: A RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_file (const gchar * file_name, GError ** error)
{
    gchar *base_uri;
    GByteArray *f;
    RsvgHandle *handle = NULL;

    rsvg_return_val_if_fail (file_name != NULL, NULL, error);

    base_uri = rsvg_get_base_uri_from_filename (file_name);
    f = _rsvg_acquire_xlink_href_resource (file_name, base_uri, error);

    if (f) {
        handle = rsvg_handle_new ();
        if (handle) {
            rsvg_handle_set_base_uri (handle, base_uri);
            if (!rsvg_handle_fill_with_data (handle, f->data, f->len, error)) {
                g_object_unref (G_OBJECT (handle));
                handle = NULL;
            }
        }
        g_byte_array_free (f, TRUE);
    }

    g_free (base_uri);

    return handle;
}
