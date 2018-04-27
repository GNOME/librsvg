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

typedef struct _RsvgFilterPrimitiveConvolveMatrix RsvgFilterPrimitiveConvolveMatrix;

typedef enum {
    EDGE_MODE_DUPLICATE,
    EDGE_MODE_WRAP,
    EDGE_MODE_NONE
} EdgeMode;

struct _RsvgFilterPrimitiveConvolveMatrix {
    RsvgFilterPrimitive super;
    double *KernelMatrix;
    double divisor;
    gint orderx, ordery;
    double dx, dy;
    double bias;
    gint targetx, targety;
    gboolean preservealpha;
    EdgeMode edgemode;
};

static void
rsvg_filter_primitive_convolve_matrix_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveConvolveMatrix *convolve = (RsvgFilterPrimitiveConvolveMatrix *) primitive;

    guchar ch;
    gint x, y;
    gint i, j;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    gint sx, sy, kx, ky;
    guchar sval;
    double kval, sum, dx, dy, targetx, targety;
    int umch;

    gint tempresult;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    targetx = convolve->targetx * ctx->paffine.xx;
    targety = convolve->targety * ctx->paffine.yy;

    if (convolve->dx != 0 || convolve->dy != 0) {
        dx = convolve->dx * ctx->paffine.xx;
        dy = convolve->dy * ctx->paffine.yy;
    } else
        dx = dy = 1;

    rowstride = cairo_image_surface_get_stride (in);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = boundarys.y0; y < boundarys.y1; y++) {
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            for (umch = 0; umch < 3 + !convolve->preservealpha; umch++) {
                ch = ctx->channelmap[umch];
                sum = 0;
                for (i = 0; i < convolve->ordery; i++) {
                    for (j = 0; j < convolve->orderx; j++) {
                        int alpha;
                        sx = x - targetx + j * dx;
                        sy = y - targety + i * dy;
                        if (convolve->edgemode == EDGE_MODE_DUPLICATE) {
                            if (sx < boundarys.x0)
                                sx = boundarys.x0;
                            if (sx >= boundarys.x1)
                                sx = boundarys.x1 - 1;
                            if (sy < boundarys.y0)
                                sy = boundarys.y0;
                            if (sy >= boundarys.y1)
                                sy = boundarys.y1 - 1;
                        } else if (convolve->edgemode == EDGE_MODE_WRAP) {
                            if (sx < boundarys.x0 || (sx >= boundarys.x1))
                                sx = boundarys.x0 + (sx - boundarys.x0) %
                                    (boundarys.x1 - boundarys.x0);
                            if (sy < boundarys.y0 || (sy >= boundarys.y1))
                                sy = boundarys.y0 + (sy - boundarys.y0) %
                                    (boundarys.y1 - boundarys.y0);
                        } else if (convolve->edgemode == EDGE_MODE_NONE) {
                            if (sx < boundarys.x0 || (sx >= boundarys.x1) ||
                                sy < boundarys.y0 || (sy >= boundarys.y1))
                                continue;
                        } else {
                            g_assert_not_reached ();
                        }

                        kx = convolve->orderx - j - 1;
                        ky = convolve->ordery - i - 1;
                        alpha = in_pixels[4 * sx + sy * rowstride + 3];
                        if (ch == 3)
                            sval = alpha;
                        else if (alpha)
                            sval = in_pixels[4 * sx + sy * rowstride + ch] * 255 / alpha;
                        else
                            sval = 0;
                        kval = convolve->KernelMatrix[kx + ky * convolve->orderx];
                        sum += (double) sval *kval;
                    }
                }

                tempresult = sum / convolve->divisor + convolve->bias;

                if (tempresult > 255)
                    tempresult = 255;
                if (tempresult < 0)
                    tempresult = 0;

                output_pixels[4 * x + y * rowstride + ch] = tempresult;
            }
            if (convolve->preservealpha)
                output_pixels[4 * x + y * rowstride + ctx->channelmap[3]] =
                    in_pixels[4 * x + y * rowstride + ctx->channelmap[3]];
            for (umch = 0; umch < 3; umch++) {
                ch = ctx->channelmap[umch];
                output_pixels[4 * x + y * rowstride + ch] =
                    output_pixels[4 * x + y * rowstride + ch] *
                    output_pixels[4 * x + y * rowstride + ctx->channelmap[3]] / 255;
            }
        }
    }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_convolve_matrix_free (gpointer impl)
{
    RsvgFilterPrimitiveConvolveMatrix *convolve = impl;

    g_free (convolve->KernelMatrix);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_convolve_matrix_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveConvolveMatrix *filter = impl;
    gint i, j;
    gboolean has_target_x, has_target_y;

    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    has_target_x = 0;
    has_target_y = 0;

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

        case RSVG_ATTRIBUTE_TARGET_X:
            has_target_x = 1;
            filter->targetx = atoi (value);
            break;

        case RSVG_ATTRIBUTE_TARGET_Y:
            has_target_y = 1;
            filter->targety = atoi (value);
            break;

        case RSVG_ATTRIBUTE_BIAS:
            filter->bias = atof (value);
            break;

        case RSVG_ATTRIBUTE_PRESERVE_ALPHA:
            if (!strcmp (value, "true"))
                filter->preservealpha = TRUE;
            else
                filter->preservealpha = FALSE;
            break;

        case RSVG_ATTRIBUTE_DIVISOR:
            filter->divisor = atof (value);
            break;

        case RSVG_ATTRIBUTE_ORDER: {
            double tempx, tempy;
            if (rsvg_css_parse_number_optional_number (value, &tempx, &tempy)
                && tempx >= 1.0 && tempy <= 100.0
                && tempy >= 1.0 && tempy <= 100.0) {
                filter->orderx = (int) tempx;
                filter->ordery = (int) tempy;
                g_assert (filter->orderx >= 1);
                g_assert (filter->ordery >= 1);

#define SIZE_OVERFLOWS(a,b) (G_UNLIKELY ((b) > 0 && (a) > G_MAXSIZE / (b)))

                if (SIZE_OVERFLOWS (filter->orderx, filter->ordery)) {
                    rsvg_node_set_attribute_parse_error (node, "order", "number of kernelMatrix elements would be too big");
                    goto out;
                }
            } else {
                rsvg_node_set_attribute_parse_error (node, "order", "invalid size for convolve matrix");
                goto out;
            }
            break;
        }

        case RSVG_ATTRIBUTE_KERNEL_UNIT_LENGTH:
            if (!rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy)) {
                rsvg_node_set_attribute_parse_error (node, "kernelUnitLength", "expected number-optional-number");
                goto out;
            }
            break;

        case RSVG_ATTRIBUTE_KERNEL_MATRIX: {
            gsize num_elems;
            gsize got_num_elems;

            num_elems = filter->orderx * filter->ordery;

            if (!rsvg_css_parse_number_list (value,
                                             NUMBER_LIST_LENGTH_EXACT,
                                             num_elems,
                                             &filter->KernelMatrix,
                                             &got_num_elems)) {
                rsvg_node_set_attribute_parse_error (node, "kernelMatrix", "expected a matrix of numbers");
                goto out;
            }

            g_assert (num_elems == got_num_elems);
            break;
        }

        case RSVG_ATTRIBUTE_EDGE_MODE:
            if (!strcmp (value, "duplicate")) {
                filter->edgemode = EDGE_MODE_DUPLICATE;
            } else if (!strcmp (value, "wrap")) {
                filter->edgemode = EDGE_MODE_WRAP;
            } else if (!strcmp (value, "none")) {
                filter->edgemode = EDGE_MODE_NONE;
            } else {
                rsvg_node_set_attribute_parse_error (node, "edgeMode", "expected 'duplicate' | 'wrap' | 'none'");
                goto out;
            }
            break;

        default:
            break;
        }
    }

    if (filter->divisor == 0) {
        for (j = 0; j < filter->orderx; j++)
            for (i = 0; i < filter->ordery; i++)
                filter->divisor += filter->KernelMatrix[j + i * filter->orderx];
    }

    if (filter->divisor == 0)
        filter->divisor = 1;

    if (!has_target_x) {
        filter->targetx = floor (filter->orderx / 2);
    }
    if (!has_target_y) {
        filter->targety = floor (filter->ordery / 2);
    }

out:

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_convolve_matrix (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveConvolveMatrix *filter;

    filter = g_new0 (RsvgFilterPrimitiveConvolveMatrix, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->KernelMatrix = NULL;
    filter->divisor = 0;
    filter->orderx = 3; /* https://www.w3.org/TR/SVG/filters.html#feConvolveMatrixElementOrderAttribute */
    filter->ordery = 3;
    filter->bias = 0;
    filter->dx = 0;
    filter->dy = 0;
    filter->preservealpha = FALSE;
    filter->edgemode = EDGE_MODE_DUPLICATE;
    filter->super.render = rsvg_filter_primitive_convolve_matrix_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_CONVOLVE_MATRIX,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_convolve_matrix_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_convolve_matrix_free);
}
