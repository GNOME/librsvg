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

cairo_surface_t *
rsvg_cairo_surface_new_from_href (RsvgHandle *handle,
                                  const char *href,
                                  GError **error)
{
    guint8 *data;
    gsize data_len;
    char *mime_type = NULL;
    GdkPixbufLoader *loader;
    GdkPixbuf *pixbuf = NULL;
    int res;
    cairo_surface_t *surface;

    data = _rsvg_handle_acquire_data (handle, href, &mime_type, &data_len, error);
    if (data == NULL)
        return NULL;

    if (mime_type) {
        loader = gdk_pixbuf_loader_new_with_mime_type (mime_type, error);
        g_free (mime_type);
    } else {
        loader = gdk_pixbuf_loader_new ();
    }

    if (loader == NULL) {
        g_free (data);
        return NULL;
    }

    res = gdk_pixbuf_loader_write (loader, data, data_len, error);
    g_free (data);

    if (!res) {
        gdk_pixbuf_loader_close (loader, NULL);
        g_object_unref (loader);
        return NULL;
    }

    if (!gdk_pixbuf_loader_close (loader, error)) {
        g_object_unref (loader);
        return NULL;
    }

    pixbuf = gdk_pixbuf_loader_get_pixbuf (loader);

    if (!pixbuf) {
        g_object_unref (loader);
        g_set_error (error,
                     GDK_PIXBUF_ERROR,
                     GDK_PIXBUF_ERROR_FAILED,
                      _("Failed to load image '%s': reason not known, probably a corrupt image file"),
                      href);
        return NULL;
    }

    surface = rsvg_cairo_surface_from_pixbuf (pixbuf);

    g_object_unref (loader);

    return surface;
}

void
rsvg_preserve_aspect_ratio (unsigned int aspect_ratio, double width,
                            double height, double *w, double *h, double *x, double *y)
{
    double neww, newh;
    if (aspect_ratio & ~RSVG_ASPECT_RATIO_SLICE) {
        neww = *w;
        newh = *h;
        if ((height * *w > width * *h) == ((aspect_ratio & RSVG_ASPECT_RATIO_SLICE) == 0)) {
            neww = width * *h / height;
        } else {
            newh = height * *w / width;
        }

        if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMAX) {
        } else if (aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMID ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMAX)
            *x -= (neww - *w) / 2;
        else
            *x -= neww - *w;

        if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMAX_YMIN) {
        } else if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMID ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMAX_YMID)
            *y -= (newh - *h) / 2;
        else
            *y -= newh - *h;

        *w = neww;
        *h = newh;
    }
}

static void
rsvg_node_image_free (RsvgNode * self)
{
    RsvgNodeImage *z = (RsvgNodeImage *) self;
    rsvg_state_finalize (z->super.state);
    g_free (z->super.state);
    z->super.state = NULL;
    if (z->surface)
        cairo_surface_destroy (z->surface);
    _rsvg_node_free(self);
}

static void
rsvg_node_image_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeImage *z = (RsvgNodeImage *) self;
    unsigned int aspect_ratio = z->preserve_aspect_ratio;
    gdouble x, y, w, h;
    cairo_surface_t *surface = z->surface;

    if (surface == NULL)
        return;

    x = _rsvg_css_normalize_length (&z->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&z->y, ctx, 'v');
    w = _rsvg_css_normalize_length (&z->w, ctx, 'h');
    h = _rsvg_css_normalize_length (&z->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, z->super.state, dominate);

    rsvg_push_discrete_layer (ctx);

    if (!rsvg_current_state (ctx)->overflow && (aspect_ratio & RSVG_ASPECT_RATIO_SLICE)) {
        rsvg_add_clipping_rect (ctx, x, y, w, h);
    }

    rsvg_preserve_aspect_ratio (aspect_ratio, 
                                (double) cairo_image_surface_get_width (surface),
                                (double) cairo_image_surface_get_height (surface), 
                                &w, &h, &x, &y);

    rsvg_render_surface (ctx, surface, x, y, w, h);

    rsvg_pop_discrete_layer (ctx);
}

static void
rsvg_node_image_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeImage *image = (RsvgNodeImage *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            image->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            image->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            image->w = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            image->h = _rsvg_css_parse_length (value);
        /* path is used by some older adobe illustrator versions */
        if ((value = rsvg_property_bag_lookup (atts, "path"))
            || (value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
            image->surface = rsvg_cairo_surface_new_from_href (ctx,
                                                               value, 
                                                               NULL);

            if (!image->surface) {
#ifdef G_ENABLE_DEBUG
                g_warning ("Couldn't load image: %s\n", value);
#endif
            }
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &image->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            image->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);

        rsvg_parse_style_attrs (ctx, image->super.state, "image", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_image (void)
{
    RsvgNodeImage *image;
    image = g_new (RsvgNodeImage, 1);
    _rsvg_node_init (&image->super, RSVG_NODE_TYPE_IMAGE);
    g_assert (image->super.state);
    image->surface = NULL;
    image->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    image->x = image->y = image->w = image->h = _rsvg_css_parse_length ("0");
    image->super.free = rsvg_node_image_free;
    image->super.draw = rsvg_node_image_draw;
    image->super.set_atts = rsvg_node_image_set_atts;
    return &image->super;
}
