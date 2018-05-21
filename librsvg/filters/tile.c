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

typedef struct _RsvgFilterPrimitiveTile RsvgFilterPrimitiveTile;

struct _RsvgFilterPrimitiveTile {
    RsvgFilterPrimitive super;
};

static int
mod (int a, int b)
{
    while (a < 0)
        a += b;
    return a % b;
}

static void
rsvg_filter_primitive_tile_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    guchar i;
    gint x, y, rowstride;
    RsvgIRect boundarys, oboundarys;

    RsvgFilterPrimitiveOutput input;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    oboundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    input = rsvg_filter_get_result (primitive->in, ctx);
    in = input.surface;
    if (in == NULL) {
        return;
    }

    boundarys = input.bounds;

    if ((boundarys.x0 >= boundarys.x1) || (boundarys.y0 >= boundarys.y1)) {
        cairo_surface_destroy (in);
        return;
    }

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    output = _rsvg_image_surface_new (rsvg_filter_context_get_width (ctx), rsvg_filter_context_get_height (ctx));
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    rowstride = cairo_image_surface_get_stride (output);

    output_pixels = cairo_image_surface_get_data (output);

    for (y = oboundarys.y0; y < oboundarys.y1; y++)
        for (x = oboundarys.x0; x < oboundarys.x1; x++)
            for (i = 0; i < 4; i++) {
                output_pixels[4 * x + y * rowstride + i] =
                    in_pixels[(mod ((x - boundarys.x0), (boundarys.x1 - boundarys.x0)) +
                               boundarys.x0) * 4 +
                              (mod ((y - boundarys.y0), (boundarys.y1 - boundarys.y0)) +
                               boundarys.y0) * rowstride + i];
            }

    cairo_surface_mark_dirty (output);

    RsvgFilterPrimitiveOutput op;
    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output(primitive->result, op, ctx);
    /* rsvg_filter_store_result (primitive->result, output, ctx); */

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_tile_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveTile *filter = impl;
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

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_tile (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveTile *filter;

    filter = g_new0 (RsvgFilterPrimitiveTile, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_tile_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_TILE,
                                parent,
                                filter,
                                rsvg_filter_primitive_tile_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}
