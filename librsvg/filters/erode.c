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

typedef struct _RsvgFilterPrimitiveErode
 RsvgFilterPrimitiveErode;

struct _RsvgFilterPrimitiveErode {
    RsvgFilterPrimitive super;
    double rx, ry;
    int mode;
};

static void
rsvg_filter_primitive_erode_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveErode *erode = (RsvgFilterPrimitiveErode *) primitive;

    guchar ch, extreme;
    gint x, y;
    gint i, j;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    gint kx, ky;
    guchar val;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    rowstride = cairo_image_surface_get_stride (in);

    /* scale the radius values */
    kx = erode->rx * ctx->paffine.xx;
    ky = erode->ry * ctx->paffine.yy;

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++)
            for (ch = 0; ch < 4; ch++) {
                if (erode->mode == 0)
                    extreme = 255;
                else
                    extreme = 0;
                for (i = -ky; i < ky + 1; i++)
                    for (j = -kx; j < kx + 1; j++) {
                        if (y + i >= height || y + i < 0 || x + j >= width || x + j < 0)
                            continue;

                        val = in_pixels[(y + i) * rowstride + (x + j) * 4 + ch];


                        if (erode->mode == 0) {
                            if (extreme > val)
                                extreme = val;
                        } else {
                            if (extreme < val)
                                extreme = val;
                        }

                    }
                output_pixels[y * rowstride + x * 4 + ch] = extreme;
            }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_erode_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveErode *filter = impl;
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

        case RSVG_ATTRIBUTE_RADIUS:
            if (!rsvg_css_parse_number_optional_number (value, &filter->rx, &filter->ry)) {
                rsvg_node_set_attribute_parse_error (node, "radius", "expected number-optional-number");
                goto out;
            }
            break;

        case RSVG_ATTRIBUTE_OPERATOR:
            if (!strcmp (value, "erode"))
                filter->mode = 0;
            else if (!strcmp (value, "dilate"))
                filter->mode = 1;
            break;

        default:
            break;
        }
    }

out:
    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_erode (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveErode *filter;

    filter = g_new0 (RsvgFilterPrimitiveErode, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->rx = 0;
    filter->ry = 0;
    filter->mode = 0;
    filter->super.render = rsvg_filter_primitive_erode_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_ERODE,
                                parent,
                                filter,
                                rsvg_filter_primitive_erode_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}
