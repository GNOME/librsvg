/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.c: Provides filters

   Copyright (C) 2004 Caleb Moore

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

   Author: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "../rsvg-private.h"
#include "../rsvg-styles.h"
#include "../rsvg-css.h"
#include "../rsvg-cairo-draw.h"
#include "common.h"

typedef struct _RsvgFilterPrimitiveImage RsvgFilterPrimitiveImage;

struct _RsvgFilterPrimitiveImage {
    RsvgFilterPrimitive super;
    RsvgHandle *handle;
    GString *href;
};

static cairo_surface_t *
rsvg_filter_primitive_image_render_in (RsvgFilterPrimitiveImage *image, RsvgFilterContext * context)
{
    RsvgDrawingCtx *ctx;
    RsvgNode *drawable;
    cairo_surface_t *result;

    ctx = context->ctx;

    if (!image->href)
        return NULL;

    drawable = rsvg_drawing_ctx_acquire_node (ctx, image->href->str);
    if (!drawable)
        return NULL;

    rsvg_state_set_affine (rsvg_drawing_ctx_get_current_state (ctx), context->paffine);

    result = rsvg_cairo_get_surface_of_node (ctx, drawable, context->width, context->height);

    rsvg_drawing_ctx_release_node (ctx, drawable);

    return result;
}

static cairo_surface_t *
rsvg_filter_primitive_image_render_ext (RsvgFilterPrimitive *self, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveImage *image = (RsvgFilterPrimitiveImage *) self;
    RsvgIRect boundarys;
    cairo_surface_t *img, *intermediate;
    int i;
    unsigned char *pixels;
    int channelmap[4];
    int length;
    int width, height;

    if (!image->href)
        return NULL;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    width = boundarys.x1 - boundarys.x0;
    height = boundarys.y1 - boundarys.y0;
    if (width == 0 || height == 0)
        return NULL;

    img = rsvg_cairo_surface_new_from_href (image->handle,
                                            image->href->str,
                                            NULL);
    if (!img)
        return NULL;

    intermediate = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (intermediate) != CAIRO_STATUS_SUCCESS ||
        !rsvg_art_affine_image (img, intermediate,
                                &ctx->paffine,
                                (gdouble) width / ctx->paffine.xx,
                                (gdouble) height / ctx->paffine.yy)) {
        cairo_surface_destroy (intermediate);
        cairo_surface_destroy (img);
        return NULL;
    }

    cairo_surface_destroy (img);

    length = cairo_image_surface_get_height (intermediate) *
             cairo_image_surface_get_stride (intermediate);
    for (i = 0; i < 4; i++)
        channelmap[i] = ctx->channelmap[i];
    pixels = cairo_image_surface_get_data (intermediate);
    for (i = 0; i < length; i += 4) {
        unsigned char alpha;
        unsigned char pixel[4];
        int ch;
        alpha = pixels[i + 3];

        pixel[channelmap[3]] = alpha;
        if (alpha)
            for (ch = 0; ch < 3; ch++)
                pixel[channelmap[ch]] = pixels[i + ch] * alpha / 255;
        else
            for (ch = 0; ch < 3; ch++)
                pixel[channelmap[ch]] = 0;
        for (ch = 0; ch < 4; ch++)
            pixels[i + ch] = pixel[ch];
    }

    cairo_surface_mark_dirty (intermediate);
    return intermediate;
}

static void
rsvg_filter_primitive_image_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveImage *image = (RsvgFilterPrimitiveImage *) primitive;

    RsvgIRect boundarys;
    RsvgFilterPrimitiveOutput op;
    cairo_surface_t *output, *img;

    if (!image->href)
        return;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (output == NULL)
        return;

    img = rsvg_filter_primitive_image_render_in (image, ctx);
    if (img == NULL) {
        img = rsvg_filter_primitive_image_render_ext (primitive, ctx);
    }

    if (img) {
        cairo_t *cr;

        cr = cairo_create (output);
        cairo_set_source_surface (cr, img, 0, 0);
        cairo_rectangle (cr,
                         boundarys.x0,
                         boundarys.y0,
                         boundarys.x1 - boundarys.x0,
                         boundarys.y1 - boundarys.y0);
        cairo_clip (cr);
        cairo_paint (cr);
        cairo_destroy (cr);

        cairo_surface_destroy (img);
    }

    op.surface = output;
    op.bounds = boundarys;

    rsvg_filter_store_output (primitive->result, op, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_image_free (gpointer impl)
{
    RsvgFilterPrimitiveImage *image = impl;

    if (image->href)
        g_string_free (image->href, TRUE);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_image_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveImage *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter->handle = handle;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_IN:
            g_string_assign (filter->super.in, value);
            break;

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        case RSVG_ATTRIBUTE_XLINK_HREF:
            filter->href = g_string_new (NULL);
            g_string_assign (filter->href, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_image (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveImage *filter;

    filter = g_new0 (RsvgFilterPrimitiveImage, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_image_render;
    filter->href = NULL;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_IMAGE,
                                parent,
                                filter,
                                rsvg_filter_primitive_image_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_image_free);
}
