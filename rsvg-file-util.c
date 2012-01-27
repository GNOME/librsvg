/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

/**
 * SECTION: rsvg-pixbuf
 * @short_description: How to render SVGs into GdkPixbufs, for easy use in GTK+
 *  applications
 *
 * GdkPixbuf is a library for image loading and manipulation. It is part of the
 * cross-platform GTK+ widget toolkit.
 */

#include "config.h"
#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-io.h"
#include "rsvg-size-callback.h"

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>

#define SVG_BUFFER_SIZE (1024 * 8)

/* private */
GdkPixbuf *
rsvg_pixbuf_from_data_with_size_data (const guchar * buff,
                                      size_t len,
                                      /* RsvgSizeCallbackData */ gpointer data,
                                      const char *base_uri, GError ** error)
{
    RsvgHandle *handle;
    GdkPixbuf *retval;

    handle = rsvg_handle_new ();

    if (!handle) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error creating SVG reader"));
        return NULL;
    }

    rsvg_handle_set_size_callback (handle, _rsvg_size_callback, data, NULL);
    rsvg_handle_set_base_uri (handle, base_uri);

    if (!rsvg_handle_write (handle, buff, len, error)) {
        g_object_unref (handle);
        return NULL;
    }

    if (!rsvg_handle_close (handle, error)) {
        g_object_unref (handle);
        return NULL;
    }

    retval = rsvg_handle_get_pixbuf (handle);
    g_object_unref (handle);

    return retval;
}

static GdkPixbuf *
rsvg_pixbuf_from_stdio_file_with_size_data (guint8 *data,
                                            gsize data_len,
                                            struct RsvgSizeCallbackData *cb_data,
                                            gchar * base_uri, 
                                            GError ** error)
{
    RsvgHandle *handle;
    GdkPixbuf *retval;

    handle = rsvg_handle_new ();

    if (!handle) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error creating SVG reader"));
        return NULL;
    }

    rsvg_handle_set_size_callback (handle, _rsvg_size_callback, cb_data, NULL);
    rsvg_handle_set_base_uri (handle, base_uri);

    if (!rsvg_handle_write (handle, data, data_len, error)) {
        g_object_unref (handle);
        return NULL;
    }

    if (!rsvg_handle_close (handle, error)) {
        g_object_unref (handle);
        return NULL;
    }

    retval = rsvg_handle_get_pixbuf (handle);
    g_object_unref (handle);

    return retval;
}

static GdkPixbuf *
rsvg_pixbuf_from_file_with_size_data (const gchar * file_name,
                                      struct RsvgSizeCallbackData *cb_data, 
                                      GError ** error)
{
    GdkPixbuf *pixbuf;
    guint8 *data;
    gsize data_len;
    GString *base_uri = g_string_new (file_name);

    data = _rsvg_io_acquire_data (file_name, base_uri->str, NULL, &data_len, NULL, error);

    if (data) {
        pixbuf = rsvg_pixbuf_from_stdio_file_with_size_data (data, data_len,
                                                             cb_data, base_uri->str, error);
        g_free (data);
    } else {
        pixbuf = NULL;
    }

    g_string_free (base_uri, TRUE);

    return pixbuf;
}

/**
 * rsvg_pixbuf_from_file:
 * @file_name: A file name
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  The caller must
 * assume the reference to the reurned pixbuf. If an error occurred, @error is
 * set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file (const gchar * file_name, GError ** error)
{
    return rsvg_pixbuf_from_file_at_size (file_name, -1, -1, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom:
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom.  The
 * caller must assume the reference to the returned pixbuf. If an error
 * occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom (const gchar * file_name,
                               double x_zoom, double y_zoom, GError ** error)
{
    struct RsvgSizeCallbackData data;

    g_return_val_if_fail (file_name != NULL, NULL);
    g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);

    data.type = RSVG_SIZE_ZOOM;
    data.x_zoom = x_zoom;
    data.y_zoom = y_zoom;
    data.keep_aspect_ratio = FALSE;

    return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom_with_max:
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom. If the
 * resulting pixbuf would be larger than max_width/max_heigh it is uniformly scaled
 * down to fit in that rectangle. The caller must assume the reference to the
 * returned pixbuf. If an error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom_with_max (const gchar * file_name,
                                        double x_zoom,
                                        double y_zoom,
                                        gint max_width, gint max_height, GError ** error)
{
    struct RsvgSizeCallbackData data;

    g_return_val_if_fail (file_name != NULL, NULL);
    g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);

    data.type = RSVG_SIZE_ZOOM_MAX;
    data.x_zoom = x_zoom;
    data.y_zoom = y_zoom;
    data.width = max_width;
    data.height = max_height;
    data.keep_aspect_ratio = FALSE;

    return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_size:
 * @file_name: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * either of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the returned pixbuf. If an
 * error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_size (const gchar * file_name, gint width, gint height, GError ** error)
{
    struct RsvgSizeCallbackData data;

    data.type = RSVG_SIZE_WH;
    data.width = width;
    data.height = height;
    data.keep_aspect_ratio = FALSE;

    return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_max_size:
 * @file_name: A file name
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is uniformly
 * scaled so that the it fits into a rectangle of size max_width * max_height. The
 * caller must assume the reference to the returned pixbuf. If an error occurred,
 * @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_max_size (const gchar * file_name,
                                   gint max_width, gint max_height, GError ** error)
{
    struct RsvgSizeCallbackData data;

    data.type = RSVG_SIZE_WH_MAX;
    data.width = max_width;
    data.height = max_height;
    data.keep_aspect_ratio = FALSE;

    return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}
