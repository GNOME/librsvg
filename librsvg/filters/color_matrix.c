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

typedef struct _RsvgFilterPrimitiveColorMatrix RsvgFilterPrimitiveColorMatrix;

struct _RsvgFilterPrimitiveColorMatrix {
    RsvgFilterPrimitive super;
    gint *KernelMatrix;
};

static void
rsvg_filter_primitive_color_matrix_render (RsvgNode *node, RsvgComputedValues *values, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveColorMatrix *color_matrix = (RsvgFilterPrimitiveColorMatrix *) primitive;

    guchar ch;
    gint x, y;
    gint i;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    int sum;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    rowstride = cairo_image_surface_get_stride (in);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    const int *ctx_channelmap = rsvg_filter_context_get_channelmap(ctx);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            int umch;
            int alpha = in_pixels[4 * x + y * rowstride + ctx_channelmap[3]];
            if (!alpha)
                for (umch = 0; umch < 4; umch++) {
                    sum = color_matrix->KernelMatrix[umch * 5 + 4];
                    if (sum > 255)
                        sum = 255;
                    if (sum < 0)
                        sum = 0;
                    output_pixels[4 * x + y * rowstride + ctx_channelmap[umch]] = sum;
            } else
                for (umch = 0; umch < 4; umch++) {
                    int umi;
                    ch = ctx_channelmap[umch];
                    sum = 0;
                    for (umi = 0; umi < 4; umi++) {
                        i = ctx_channelmap[umi];
                        if (umi != 3)
                            sum += color_matrix->KernelMatrix[umch * 5 + umi] *
                                in_pixels[4 * x + y * rowstride + i] / alpha;
                        else
                            sum += color_matrix->KernelMatrix[umch * 5 + umi] *
                                in_pixels[4 * x + y * rowstride + i] / 255;
                    }
                    sum += color_matrix->KernelMatrix[umch * 5 + 4];



                    if (sum > 255)
                        sum = 255;
                    if (sum < 0)
                        sum = 0;

                    output_pixels[4 * x + y * rowstride + ch] = sum;
                }
            for (umch = 0; umch < 3; umch++) {
                ch = ctx_channelmap[umch];
                output_pixels[4 * x + y * rowstride + ch] =
                    output_pixels[4 * x + y * rowstride + ch] *
                    output_pixels[4 * x + y * rowstride + ctx_channelmap[3]] / 255;
            }
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
rsvg_filter_primitive_color_matrix_free (gpointer impl)
{
    RsvgFilterPrimitiveColorMatrix *matrix = impl;

    g_free (matrix->KernelMatrix);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_color_matrix_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveColorMatrix *filter = impl;
    gint type;
    gsize listlen = 0;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    type = 0;

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

        case RSVG_ATTRIBUTE_VALUES: {
            unsigned int i;
            double *temp;
            if (!rsvg_css_parse_number_list (value,
                                             NUMBER_LIST_LENGTH_MAXIMUM,
                                             20,
                                             &temp,
                                             &listlen)) {
                rsvg_node_set_attribute_parse_error (node, "values", "invalid number list");
                goto out;
            }

            filter->KernelMatrix = g_new0 (int, listlen);
            for (i = 0; i < listlen; i++)
                filter->KernelMatrix[i] = temp[i] * 255.;
            g_free (temp);
            break;
        }

        case RSVG_ATTRIBUTE_TYPE:
            if (!strcmp (value, "matrix"))
                type = 0;
            else if (!strcmp (value, "saturate"))
                type = 1;
            else if (!strcmp (value, "hueRotate"))
                type = 2;
            else if (!strcmp (value, "luminanceToAlpha"))
                type = 3;
            else
                type = 0;
            break;

        default:
            break;
        }
    }

    if (type == 0) {
        if (listlen != 20) {
            if (filter->KernelMatrix != NULL)
                g_free (filter->KernelMatrix);
            filter->KernelMatrix = g_new0 (int, 20);
        }
    } else if (type == 1) {
        float s;
        if (listlen != 0) {
            s = filter->KernelMatrix[0];
            g_free (filter->KernelMatrix);
        } else
            s = 255;
        filter->KernelMatrix = g_new0 (int, 20);

        filter->KernelMatrix[0] = 0.213 * 255. + 0.787 * s;
        filter->KernelMatrix[1] = 0.715 * 255. - 0.715 * s;
        filter->KernelMatrix[2] = 0.072 * 255. - 0.072 * s;
        filter->KernelMatrix[5] = 0.213 * 255. - 0.213 * s;
        filter->KernelMatrix[6] = 0.715 * 255. + 0.285 * s;
        filter->KernelMatrix[7] = 0.072 * 255. - 0.072 * s;
        filter->KernelMatrix[10] = 0.213 * 255. - 0.213 * s;
        filter->KernelMatrix[11] = 0.715 * 255. - 0.715 * s;
        filter->KernelMatrix[12] = 0.072 * 255. + 0.928 * s;
        filter->KernelMatrix[18] = 255;
    } else if (type == 2) {
        double cosval, sinval, arg;

        if (listlen != 0) {
            arg = (double) filter->KernelMatrix[0] / 255.;
            g_free (filter->KernelMatrix);
        } else
            arg = 0;

        cosval = cos (arg);
        sinval = sin (arg);

        filter->KernelMatrix = g_new0 (int, 20);

        filter->KernelMatrix[0] = (0.213 + cosval * 0.787 + sinval * -0.213) * 255.;
        filter->KernelMatrix[1] = (0.715 + cosval * -0.715 + sinval * -0.715) * 255.;
        filter->KernelMatrix[2] = (0.072 + cosval * -0.072 + sinval * 0.928) * 255.;
        filter->KernelMatrix[5] = (0.213 + cosval * -0.213 + sinval * 0.143) * 255.;
        filter->KernelMatrix[6] = (0.715 + cosval * 0.285 + sinval * 0.140) * 255.;
        filter->KernelMatrix[7] = (0.072 + cosval * -0.072 + sinval * -0.283) * 255.;
        filter->KernelMatrix[10] = (0.213 + cosval * -0.213 + sinval * -0.787) * 255.;
        filter->KernelMatrix[11] = (0.715 + cosval * -0.715 + sinval * 0.715) * 255.;
        filter->KernelMatrix[12] = (0.072 + cosval * 0.928 + sinval * 0.072) * 255.;
        filter->KernelMatrix[18] = 255;
    } else if (type == 3) {
        if (filter->KernelMatrix != NULL)
            g_free (filter->KernelMatrix);

        filter->KernelMatrix = g_new0 (int, 20);

        filter->KernelMatrix[15] = 0.2125 * 255.;
        filter->KernelMatrix[16] = 0.7154 * 255.;
        filter->KernelMatrix[17] = 0.0721 * 255.;
    } else {
        g_assert_not_reached ();
    }

out:
    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_color_matrix (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveColorMatrix *filter;

    filter = g_new0 (RsvgFilterPrimitiveColorMatrix, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->KernelMatrix = NULL;
    filter->super.render = rsvg_filter_primitive_color_matrix_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_COLOR_MATRIX,
                                parent,
                                filter,
                                rsvg_filter_primitive_color_matrix_set_atts,
                                rsvg_filter_primitive_color_matrix_free);
}
