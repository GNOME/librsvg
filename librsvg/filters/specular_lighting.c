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
#include "light_source.h"

typedef struct _RsvgFilterPrimitiveSpecularLighting RsvgFilterPrimitiveSpecularLighting;

struct _RsvgFilterPrimitiveSpecularLighting {
    RsvgFilterPrimitive super;
    double specularConstant;
    double specularExponent;
    double surfaceScale;
};

static void
rsvg_filter_primitive_specular_lighting_render (RsvgNode *node, RsvgComputedValues *values, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveSpecularLighting *specular_lighting = (RsvgFilterPrimitiveSpecularLighting *) primitive;

    gint x, y;
    gdouble z, surfaceScale;
    gint rowstride, height, width;
    gdouble factor, max, base;
    vector3 lightcolor, color;
    vector3 L;
    cairo_matrix_t iaffine;
    RsvgIRect boundarys;
    RsvgNodeLightSource *source = NULL;
    guint32 lightingcolor;
    guint32 *p_lightingcolor;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    source = find_light_source_in_children (node);
    if (source == NULL)
        return;

    cairo_matrix_t ctx_paffine = rsvg_filter_context_get_paffine(ctx);
    iaffine = ctx_paffine;
    if (cairo_matrix_invert (&iaffine) != CAIRO_STATUS_SUCCESS)
      return;

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

    lightingcolor = rsvg_computed_values_get_lighting_color_argb (values);
    p_lightingcolor = &lightingcolor;

    output_pixels = cairo_image_surface_get_data (output);

    color.x = ((guchar *) p_lightingcolor)[2] / 255.0;
    color.y = ((guchar *) p_lightingcolor)[1] / 255.0;
    color.z = ((guchar *) p_lightingcolor)[0] / 255.0;

    surfaceScale = specular_lighting->surfaceScale / 255.0;

    const int *ctx_channelmap = rsvg_filter_context_get_channelmap(ctx);
    RsvgDrawingCtx *drawing_ctx = rsvg_filter_context_get_drawing_ctx(ctx);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = in_pixels[y * rowstride + x * 4 + 3] * surfaceScale;
            L = get_light_direction (values, source, x, y, z, &iaffine, drawing_ctx);
            L.z += 1;
            L = normalise (L);

            lightcolor = get_light_color (values, source, color, x, y, z, &iaffine, drawing_ctx);
            base = dotproduct (get_surface_normal (in_pixels, boundarys, x, y,
                                                   1, 1, 1.0 / ctx_paffine.xx,
                                                   1.0 / ctx_paffine.yy, specular_lighting->surfaceScale,
                                                   rowstride, ctx_channelmap[3]), L);

            factor = specular_lighting->specularConstant * pow (base, specular_lighting->specularExponent) * 255;

            max = 0;
            if (max < lightcolor.x)
                max = lightcolor.x;
            if (max < lightcolor.y)
                max = lightcolor.y;
            if (max < lightcolor.z)
                max = lightcolor.z;

            max *= factor;
            if (max > 255)
                max = 255;
            if (max < 0)
                max = 0;

            output_pixels[y * rowstride + x * 4 + ctx_channelmap[0]] = lightcolor.x * max;
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[1]] = lightcolor.y * max;
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[2]] = lightcolor.z * max;
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[3]] = max;

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
rsvg_filter_primitive_specular_lighting_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveSpecularLighting *filter = impl;
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

        case RSVG_ATTRIBUTE_SPECULAR_CONSTANT:
            filter->specularConstant = g_ascii_strtod (value, NULL);
            break;

        case RSVG_ATTRIBUTE_SPECULAR_EXPONENT:
            filter->specularExponent = g_ascii_strtod (value, NULL);
            break;

        case RSVG_ATTRIBUTE_SURFACE_SCALE:
            filter->surfaceScale = g_ascii_strtod (value, NULL);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_specular_lighting (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveSpecularLighting *filter;

    filter = g_new0 (RsvgFilterPrimitiveSpecularLighting, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->surfaceScale = 1;
    filter->specularConstant = 1;
    filter->specularExponent = 1;
    filter->super.render = rsvg_filter_primitive_specular_lighting_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_SPECULAR_LIGHTING,
                                parent,
                                filter,
                                rsvg_filter_primitive_specular_lighting_set_atts,
                                rsvg_filter_primitive_free);
}
