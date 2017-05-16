/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-image.c: Image loading and displaying

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "rsvg-image.h"
#include <string.h>
#include <math.h>
#include <errno.h>
#include "rsvg-css.h"
#include "rsvg-io.h"
#include "rsvg-styles.h"

cairo_surface_t *
rsvg_cairo_surface_new_from_href (RsvgHandle *handle,
                                  const char *href,
                                  GError **error)
{
    char *data;
    gsize data_len;
    char *mime_type = NULL;
    GdkPixbufLoader *loader = NULL;
    GdkPixbuf *pixbuf = NULL;
    cairo_surface_t *surface = NULL;

    data = _rsvg_handle_acquire_data (handle, href, &mime_type, &data_len, error);
    if (data == NULL)
        return NULL;

    if (mime_type) {
        loader = gdk_pixbuf_loader_new_with_mime_type (mime_type, error);
    } else {
        loader = gdk_pixbuf_loader_new ();
    }

    if (loader == NULL)
        goto out;

    if (!gdk_pixbuf_loader_write (loader, (guchar *) data, data_len, error)) {
        gdk_pixbuf_loader_close (loader, NULL);
        goto out;
    }

    if (!gdk_pixbuf_loader_close (loader, error))
        goto out;

    pixbuf = gdk_pixbuf_loader_get_pixbuf (loader);

    if (!pixbuf) {
        g_set_error (error,
                     GDK_PIXBUF_ERROR,
                     GDK_PIXBUF_ERROR_FAILED,
                      _("Failed to load image '%s': reason not known, probably a corrupt image file"),
                      href);
        goto out;
    }

    surface = rsvg_cairo_surface_from_pixbuf (pixbuf);

    if (mime_type == NULL) {
        /* Try to get the information from the loader */
        GdkPixbufFormat *format;
        char **mime_types;

        if ((format = gdk_pixbuf_loader_get_format (loader)) != NULL) {
            mime_types = gdk_pixbuf_format_get_mime_types (format);

            if (mime_types != NULL)
                mime_type = g_strdup (mime_types[0]);
            g_strfreev (mime_types);
        }
    }

    if ((handle->priv->flags & RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA) != 0 &&
        mime_type != NULL &&
        cairo_surface_set_mime_data (surface, mime_type, (guchar *) data,
                                     data_len, g_free, data) == CAIRO_STATUS_SUCCESS) {
        data = NULL; /* transferred to the surface */
    }

  out:
    if (loader)
        g_object_unref (loader);
    g_free (mime_type);
    g_free (data);

    return surface;
}

static void
rsvg_node_image_free (gpointer impl)
{
    RsvgNodeImage *image = impl;

    if (image->surface)
        cairo_surface_destroy (image->surface);

    g_free (image);
}

static void
rsvg_node_image_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeImage *z = impl;
    RsvgState *state;
    unsigned int aspect_ratio = z->preserve_aspect_ratio;
    gdouble x, y, w, h;
    cairo_surface_t *surface = z->surface;

    if (surface == NULL)
        return;

    x = rsvg_length_normalize (&z->x, ctx);
    y = rsvg_length_normalize (&z->y, ctx);
    w = rsvg_length_normalize (&z->w, ctx);
    h = rsvg_length_normalize (&z->h, ctx);

    state = rsvg_node_get_state (node);

    rsvg_state_reinherit_top (ctx, state, dominate);

    rsvg_push_discrete_layer (ctx);

    if (!rsvg_current_state (ctx)->overflow && (aspect_ratio & RSVG_ASPECT_RATIO_SLICE)) {
        rsvg_drawing_ctx_add_clipping_rect (ctx, x, y, w, h);
    }

    rsvg_aspect_ratio_compute (aspect_ratio, 
                               (double) cairo_image_surface_get_width (surface),
                               (double) cairo_image_surface_get_height (surface), 
                               &x, &y, &w, &h);

    rsvg_render_surface (ctx, surface, x, y, w, h);

    rsvg_pop_discrete_layer (ctx);
}

static void
rsvg_node_image_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeImage *image = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "x")))
        image->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        image->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "width")))
        image->w = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "height")))
        image->h = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    /* path is used by some older adobe illustrator versions */
    if ((value = rsvg_property_bag_lookup (atts, "path"))
        || (value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
        image->surface = rsvg_cairo_surface_new_from_href (handle,
                                                           value, 
                                                           NULL);

        if (!image->surface) {
#ifdef G_ENABLE_DEBUG
            g_warning ("Couldn't load image: %s\n", value);
#endif
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
        image->preserve_aspect_ratio = rsvg_aspect_ratio_parse (value);
}

RsvgNode *
rsvg_new_image (const char *element_name, RsvgNode *parent)
{
    RsvgNodeImage *image;

    image = g_new0 (RsvgNodeImage, 1);
    image->surface = NULL;
    image->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    image->x = image->y = image->w = image->h = rsvg_length_parse ("0", LENGTH_DIR_BOTH);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_IMAGE,
                                parent,
                                rsvg_state_new (),
                                image,
                                rsvg_node_image_set_atts,
                                rsvg_node_image_draw,
                                rsvg_node_image_free);
}
