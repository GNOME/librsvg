/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-file-util.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

   This library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public
   License as published by the Free Software Foundation; either
   version 2.1 of the License, or (at your option) any later version.

   This library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with this library; if not, write to the Free Software
   Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA

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

/**
 * rsvg_pixbuf_from_file_at_zoom_with_max:
 * @filename: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @max_width: The requested max width
 * @max_height: The requested max height
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

/**
 * rsvg_pixbuf_from_file_at_size:
 * @filename: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @filename and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * both of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the returned pixbuf. If an
 * error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 * Deprecated: Set up a cairo matrix and use rsvg_handle_new_from_file() + rsvg_handle_render_cairo() instead.
 **/

/**
 * rsvg_pixbuf_from_file_at_max_size:
 * @filename: A file name
 * @max_width: The requested max width
 * @max_height: The requested max height
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
