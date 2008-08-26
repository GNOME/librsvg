/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002-2005 Dom Lachowicz <cinamod@hotmail.com>

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
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-image.h"
#include "rsvg-text.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"

#include <math.h>
#include <string.h>
#include <stdarg.h>

#include "rsvg-cairo.h"
#include "rsvg-cairo-draw.h"

static void
rsvg_pixmap_destroy (gchar * pixels, gpointer data)
{
    g_free (pixels);
}

/**
 * rsvg_handle_get_pixbuf_sub:
 * @handle: An #RsvgHandle
 * @id: The id of an element inside the SVG, or %NULL to render the whole SVG. For
 * example, if you have a layer called "layer1" that you wish to render, pass 
 * "##layer1" as the id.
 *
 * Returns the pixbuf loaded by #handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Returns: the pixbuf loaded by #handle, or %NULL.
 *
 * Since: 2.14
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf_sub (RsvgHandle * handle, const char *id)
{
    RsvgDimensionData dimensions;
    GdkPixbuf *output = NULL;
    guint8 *pixels;
    cairo_surface_t *surface;
    cairo_t *cr;
    int rowstride;

    g_return_val_if_fail (handle != NULL, NULL);

    if (!handle->priv->finished)
        return NULL;

    rsvg_handle_get_dimensions (handle, &dimensions);
    if (!(dimensions.width && dimensions.height))
        return NULL;

    rowstride = dimensions.width * 4;

    pixels = g_try_malloc0 (dimensions.width * dimensions.height * 4);
    if (!pixels)
        return NULL;

    surface = cairo_image_surface_create_for_data (pixels,
                                                   CAIRO_FORMAT_ARGB32,
                                                   dimensions.width, dimensions.height, rowstride);
    cr = cairo_create (surface);
    cairo_surface_destroy (surface);

    if (rsvg_handle_render_cairo_sub (handle, cr, id)) {
		rsvg_cairo_to_pixbuf (pixels, rowstride, dimensions.height);

		output = gdk_pixbuf_new_from_data (pixels,
										   GDK_COLORSPACE_RGB,
										   TRUE,
										   8,
										   dimensions.width,
										   dimensions.height,
										   rowstride,
										   (GdkPixbufDestroyNotify) rsvg_pixmap_destroy, NULL);
	} else {
		g_free (pixels);
		output = NULL;
	}

    cairo_destroy (cr);

    return output;
}

/**
 * rsvg_handle_get_pixbuf:
 * @handle: An #RsvgHandle
 *
 * Returns the pixbuf loaded by #handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Returns: the pixbuf loaded by #handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle * handle)
{
    return rsvg_handle_get_pixbuf_sub (handle, NULL);
}
