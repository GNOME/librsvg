/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>

#include "rsvg.h"

/* Defined in rsvg_internals/src/pixbuf_utils.rs */
extern GdkPixbuf *rsvg_rust_pixbuf_from_file_at_size (const char *filename,
                                                      int width,
                                                      int height,
                                                      GError **error);
extern GdkPixbuf *rsvg_rust_pixbuf_from_file_at_zoom (const char *filename,
                                                      double x_zoom,
                                                      double y_zoom,
                                                      GError **error);
extern GdkPixbuf *rsvg_rust_pixbuf_from_file_at_zoom_with_max (const char *filename,
                                                               double x_zoom,
                                                               double y_zoom,
                                                               int width,
                                                               int height,
                                                               GError **error);
extern GdkPixbuf *rsvg_rust_pixbuf_from_file_at_max_size (const char *filename,
                                                          int width,
                                                          int height,
                                                          GError **error);

/**
 * rsvg_pixbuf_from_file:
 * @filename: A file name
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  The caller must
 * assume the reference to the reurned pixbuf. If an error occurred, @error is
 * set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file (const gchar *filename, GError **error)
{
    g_return_val_if_fail (filename != NULL, NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    return rsvg_rust_pixbuf_from_file_at_size (filename, -1, -1, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom:
 * @filename: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom.  The
 * caller must assume the reference to the returned pixbuf. If an error
 * occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom (const gchar *filename,
                               double x_zoom, double y_zoom, GError **error)
{
    g_return_val_if_fail (filename != NULL, NULL);
    g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    return rsvg_rust_pixbuf_from_file_at_zoom (filename, x_zoom, y_zoom, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom_with_max:
 * @filename: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom. If the
 * resulting pixbuf would be larger than max_width/max_heigh it is uniformly scaled
 * down to fit in that rectangle. The caller must assume the reference to the
 * returned pixbuf. If an error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom_with_max (const gchar *filename,
                                        double x_zoom,
                                        double y_zoom,
                                        gint max_width, gint max_height, GError **error)
{
    g_return_val_if_fail (filename != NULL, NULL);
    g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
    g_return_val_if_fail (max_width >= 1 && max_height >= 1, NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    return rsvg_rust_pixbuf_from_file_at_zoom_with_max (filename, x_zoom, y_zoom, max_width, max_height, error);
}

/**
 * rsvg_pixbuf_from_file_at_size:
 * @filename: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * either of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the returned pixbuf. If an
 * error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_size (const gchar *filename, gint width, gint height, GError **error)
{
    g_return_val_if_fail (filename != NULL, NULL);
    g_return_val_if_fail ((width >= 1 && height >= 1) || (width == -1 && height == -1), NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    return rsvg_rust_pixbuf_from_file_at_size (filename, width, height, error);
}

/**
 * rsvg_pixbuf_from_file_at_max_size:
 * @filename: A file name
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  This pixbuf is uniformly
 * scaled so that the it fits into a rectangle of size max_width * max_height. The
 * caller must assume the reference to the returned pixbuf. If an error occurred,
 * @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_max_size (const gchar *filename,
                                   gint max_width, gint max_height, GError **error)
{
    g_return_val_if_fail (filename != NULL, NULL);
    g_return_val_if_fail (max_width >= 1 && max_height >= 1, NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    return rsvg_rust_pixbuf_from_file_at_max_size(filename, max_width, max_height, error);
}
