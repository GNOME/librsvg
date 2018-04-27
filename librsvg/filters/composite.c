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

typedef enum {
    COMPOSITE_MODE_OVER, COMPOSITE_MODE_IN, COMPOSITE_MODE_OUT,
    COMPOSITE_MODE_ATOP, COMPOSITE_MODE_XOR, COMPOSITE_MODE_ARITHMETIC
} RsvgFilterPrimitiveCompositeMode;

typedef struct _RsvgFilterPrimitiveComposite RsvgFilterPrimitiveComposite;
struct _RsvgFilterPrimitiveComposite {
    RsvgFilterPrimitive super;
    RsvgFilterPrimitiveCompositeMode mode;
    GString *in2;

    int k1, k2, k3, k4;
};

static cairo_operator_t
composite_mode_to_cairo_operator (RsvgFilterPrimitiveCompositeMode mode)
{
    switch (mode) {
    case COMPOSITE_MODE_OVER:
        return CAIRO_OPERATOR_OVER;

    case COMPOSITE_MODE_IN:
        return CAIRO_OPERATOR_IN;

    case COMPOSITE_MODE_OUT:
        return CAIRO_OPERATOR_OUT;

    case COMPOSITE_MODE_ATOP:
        return CAIRO_OPERATOR_ATOP;

    case COMPOSITE_MODE_XOR:
        return CAIRO_OPERATOR_XOR;

    default:
        g_assert_not_reached ();
        return CAIRO_OPERATOR_CLEAR;
    }
}

static void
rsvg_filter_primitive_composite_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveComposite *composite = (RsvgFilterPrimitiveComposite *) primitive;
    RsvgIRect boundarys;
    cairo_surface_t *output, *in, *in2;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    in2 = rsvg_filter_get_in (composite->in2, ctx);
    if (in2 == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    if (composite->mode == COMPOSITE_MODE_ARITHMETIC) {
        guchar i;
        gint x, y;
        gint rowstride, height, width;
        guchar *in_pixels;
        guchar *in2_pixels;
        guchar *output_pixels;

        height = cairo_image_surface_get_height (in);
        width = cairo_image_surface_get_width (in);
        rowstride = cairo_image_surface_get_stride (in);

        output = _rsvg_image_surface_new (width, height);
        if (output == NULL) {
            cairo_surface_destroy (in);
            cairo_surface_destroy (in2);
            return;
        }

        cairo_surface_flush (in);
        cairo_surface_flush (in2);

        in_pixels = cairo_image_surface_get_data (in);
        in2_pixels = cairo_image_surface_get_data (in2);
        output_pixels = cairo_image_surface_get_data (output);

        for (y = boundarys.y0; y < boundarys.y1; y++) {
            for (x = boundarys.x0; x < boundarys.x1; x++) {
                int qr, qa, qb;

                qa = in_pixels[4 * x + y * rowstride + 3];
                qb = in2_pixels[4 * x + y * rowstride + 3];
                qr = (composite->k1 * qa * qb / 255 + composite->k2 * qa + composite->k3 * qb) / 255;

                if (qr > 255)
                    qr = 255;
                if (qr < 0)
                    qr = 0;
                output_pixels[4 * x + y * rowstride + 3] = qr;
                if (qr) {
                    for (i = 0; i < 3; i++) {
                        int ca, cb, cr;
                        ca = in_pixels[4 * x + y * rowstride + i];
                        cb = in2_pixels[4 * x + y * rowstride + i];

                        cr = (ca * cb * composite->k1 / 255 + ca * composite->k2 +
                              cb * composite->k3 + composite->k4 * qr) / 255;
                        if (cr > qr)
                            cr = qr;
                        if (cr < 0)
                            cr = 0;
                        output_pixels[4 * x + y * rowstride + i] = cr;
                    }
                }
            }
        }

        cairo_surface_mark_dirty (output);
    } else {
        cairo_t *cr;

        cairo_surface_reference (in2);
        output = in2;

        cr = cairo_create (output);
        cairo_set_source_surface (cr, in, 0, 0);
        cairo_rectangle (cr,
                         boundarys.x0,
                         boundarys.y0,
                         boundarys.x1 - boundarys.x0,
                         boundarys.y1 - boundarys.y0);
        cairo_clip (cr);
        cairo_set_operator (cr, composite_mode_to_cairo_operator (composite->mode));
        cairo_paint (cr);
        cairo_destroy (cr);
    }

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_composite_free (gpointer impl)
{
    RsvgFilterPrimitiveComposite *composite = impl;

    g_string_free (composite->in2, TRUE);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_composite_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveComposite *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_OPERATOR:
            if (!strcmp (value, "in"))
                filter->mode = COMPOSITE_MODE_IN;
            else if (!strcmp (value, "out"))
                filter->mode = COMPOSITE_MODE_OUT;
            else if (!strcmp (value, "atop"))
                filter->mode = COMPOSITE_MODE_ATOP;
            else if (!strcmp (value, "xor"))
                filter->mode = COMPOSITE_MODE_XOR;
            else if (!strcmp (value, "arithmetic"))
                filter->mode = COMPOSITE_MODE_ARITHMETIC;
            else
                filter->mode = COMPOSITE_MODE_OVER;
            break;

        case RSVG_ATTRIBUTE_IN:
            g_string_assign (filter->super.in, value);
            break;

        case RSVG_ATTRIBUTE_IN2:
            g_string_assign (filter->in2, value);
            break;

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        case RSVG_ATTRIBUTE_K1:
            filter->k1 = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_K2:
            filter->k2 = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_K3:
            filter->k3 = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_K4:
            filter->k4 = g_ascii_strtod (value, NULL) * 255.;
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_composite (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveComposite *filter;

    filter = g_new0 (RsvgFilterPrimitiveComposite, 1);
    filter->mode = COMPOSITE_MODE_OVER;
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->k1 = 0;
    filter->k2 = 0;
    filter->k3 = 0;
    filter->k4 = 0;
    filter->super.render = rsvg_filter_primitive_composite_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPOSITE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_composite_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_composite_free);
}
