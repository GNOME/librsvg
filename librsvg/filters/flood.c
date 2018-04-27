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
#include "../rsvg-cairo-render.h"
#include "common.h"

static void
rsvg_filter_primitive_flood_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgState *state;
    guchar i;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;
    guchar *output_pixels;
    cairo_surface_t *output;
    char pixcolor[4];
    RsvgFilterPrimitiveOutput out;

    state = rsvg_node_get_state (node);

    guint32 color = rsvg_state_get_flood_color (state);
    guint8 opacity = rsvg_state_get_flood_opacity (state);

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    height = ctx->height;
    width = ctx->width;
    output = _rsvg_image_surface_new (width, height);
    if (output == NULL)
        return;

    rowstride = cairo_image_surface_get_stride (output);

    output_pixels = cairo_image_surface_get_data (output);

    for (i = 0; i < 3; i++)
        pixcolor[i] = (int) (((unsigned char *)
                              (&color))[2 - i]) * opacity / 255;
    pixcolor[3] = opacity;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++)
            for (i = 0; i < 4; i++)
                output_pixels[4 * x + y * rowstride + ctx->channelmap[i]] = pixcolor[i];

    cairo_surface_mark_dirty (output);

    out.surface = output;
    out.bounds = boundarys;

    rsvg_filter_store_output (primitive->result, out, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_flood_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitive *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->result, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_flood (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitive *filter;

    filter = g_new0 (RsvgFilterPrimitive, 1);
    filter->in = g_string_new ("none");
    filter->result = g_string_new ("none");
    filter->render = rsvg_filter_primitive_flood_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_FLOOD,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_flood_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}
