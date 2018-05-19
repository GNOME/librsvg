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
#include "../rsvg-drawing-ctx.h"
#include "common.h"

typedef struct _RsvgFilterPrimitiveDisplacementMap RsvgFilterPrimitiveDisplacementMap;

struct _RsvgFilterPrimitiveDisplacementMap {
    RsvgFilterPrimitive super;
    gint dx, dy;
    char xChannelSelector, yChannelSelector;
    GString *in2;
    double scale;
};

static void
rsvg_filter_primitive_displacement_map_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveDisplacementMap *displacement_map = (RsvgFilterPrimitiveDisplacementMap *) primitive;
    guchar ch, xch, ych;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *in2_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in, *in2;

    double ox, oy;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in2 = rsvg_filter_get_in (displacement_map->in2, ctx);
    if (in2 == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    cairo_surface_flush (in2);

    in_pixels = cairo_image_surface_get_data (in);
    in2_pixels = cairo_image_surface_get_data (in2);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    rowstride = cairo_image_surface_get_stride (in);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        cairo_surface_destroy (in2);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    switch (displacement_map->xChannelSelector) {
    case 'R':
        xch = 0;
        break;
    case 'G':
        xch = 1;
        break;
    case 'B':
        xch = 2;
        break;
    case 'A':
        xch = 3;
        break;
    default:
        xch = 0;
        break;
    }

    switch (displacement_map->yChannelSelector) {
    case 'R':
        ych = 0;
        break;
    case 'G':
        ych = 1;
        break;
    case 'B':
        ych = 2;
        break;
    case 'A':
        ych = 3;
        break;
    default:
        ych = 1;
        break;
    }

    const int *ctx_channelmap = rsvg_filter_context_get_channelmap(ctx);
    cairo_matrix_t ctx_paffine = rsvg_filter_context_get_paffine(ctx);

    xch = ctx_channelmap[xch];
    ych = ctx_channelmap[ych];
    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            if (xch != 4)
                ox = x + displacement_map->scale * ctx_paffine.xx *
                    ((double) in2_pixels[y * rowstride + x * 4 + xch] / 255.0 - 0.5);
            else
                ox = x;

            if (ych != 4)
                oy = y + displacement_map->scale * ctx_paffine.yy *
                    ((double) in2_pixels[y * rowstride + x * 4 + ych] / 255.0 - 0.5);
            else
                oy = y;

            for (ch = 0; ch < 4; ch++) {
                output_pixels[y * rowstride + x * 4 + ch] =
                    get_interp_pixel (in_pixels, ox, oy, ch, boundarys, rowstride);
            }
        }

    cairo_surface_mark_dirty (output);

    RsvgFilterPrimitiveOutput op;
    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output(primitive->result, op, ctx);
    /* rsvg_filter_store_result (primitive->result, output, ctx); */

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_displacement_map_free (gpointer impl)
{
    RsvgFilterPrimitiveDisplacementMap *dmap = impl;

    g_string_free (dmap->in2, TRUE);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_displacement_map_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveDisplacementMap *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_IN:
            g_string_assign (filter->super.in, value);
            break;

        case RSVG_ATTRIBUTE_IN2:
            g_string_assign (filter->in2, value);
            break;

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        case RSVG_ATTRIBUTE_X_CHANNEL_SELECTOR:
            filter->xChannelSelector = (value)[0];
            break;

        case RSVG_ATTRIBUTE_Y_CHANNEL_SELECTOR:
            filter->yChannelSelector = (value)[0];
            break;

        case RSVG_ATTRIBUTE_SCALE:
            filter->scale = g_ascii_strtod (value, NULL);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_displacement_map (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveDisplacementMap *filter;

    filter = g_new0 (RsvgFilterPrimitiveDisplacementMap, 1);
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->xChannelSelector = ' ';
    filter->yChannelSelector = ' ';
    filter->scale = 0;
    filter->super.render = rsvg_filter_primitive_displacement_map_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_DISPLACEMENT_MAP,
                                parent,
                                filter,
                                rsvg_filter_primitive_displacement_map_set_atts,
                                rsvg_filter_primitive_displacement_map_free);
}
