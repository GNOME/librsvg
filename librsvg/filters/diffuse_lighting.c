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
#include "common.h"
#include "light_source.h"

typedef struct _RsvgFilterPrimitiveDiffuseLighting RsvgFilterPrimitiveDiffuseLighting;

struct _RsvgFilterPrimitiveDiffuseLighting {
    RsvgFilterPrimitive super;
    gdouble dx, dy;
    double diffuseConstant;
    double surfaceScale;
};

static void
rsvg_filter_primitive_diffuse_lighting_render (RsvgNode *node,
                                               RsvgComputedValues *values,
                                               RsvgFilterPrimitive *primitive,
                                               RsvgFilterContext *ctx,
                                               RsvgDrawingCtx *draw_ctx)
{
    RsvgFilterPrimitiveDiffuseLighting *diffuse_lighting = (RsvgFilterPrimitiveDiffuseLighting *) primitive;

    gint x, y;
    float dy, dx, rawdy, rawdx;
    gdouble z;
    gint rowstride, height, width;
    gdouble factor, surfaceScale;
    vector3 lightcolor, L, N;
    vector3 color;
    cairo_matrix_t iaffine;
    RsvgNodeLightSource *source = NULL;
    RsvgIRect boundarys;
    guint32 lightingcolor;
    guint32 *p_lightingcolor;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    source = find_light_source_in_children (node);
    if (source == NULL)
        return;

    iaffine = rsvg_filter_context_get_paffine(ctx);
    if (cairo_matrix_invert (&iaffine) != CAIRO_STATUS_SUCCESS)
      return;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx, draw_ctx);

    in = rsvg_filter_get_in (primitive->in, ctx, draw_ctx);
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

    surfaceScale = diffuse_lighting->surfaceScale / 255.0;

    cairo_matrix_t ctx_paffine = rsvg_filter_context_get_paffine(ctx);

    if (diffuse_lighting->dy < 0 || diffuse_lighting->dx < 0) {
        dx = 1;
        dy = 1;
        rawdx = 1;
        rawdy = 1;
    } else {
        dx = diffuse_lighting->dx * ctx_paffine.xx;
        dy = diffuse_lighting->dy * ctx_paffine.yy;
        rawdx = diffuse_lighting->dx;
        rawdy = diffuse_lighting->dy;
    }

    const int *ctx_channelmap = rsvg_filter_context_get_channelmap(ctx);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = surfaceScale * (double) in_pixels[y * rowstride + x * 4 + ctx_channelmap[3]];
            L = get_light_direction (values, source, x, y, z, &iaffine, draw_ctx);
            N = get_surface_normal (in_pixels, boundarys, x, y,
                                    dx, dy, rawdx, rawdy, diffuse_lighting->surfaceScale,
                                    rowstride, ctx_channelmap[3]);
            lightcolor = get_light_color (values, source, color, x, y, z, &iaffine, draw_ctx);
            factor = dotproduct (N, L);

            output_pixels[y * rowstride + x * 4 + ctx_channelmap[0]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.x * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[1]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.y * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[2]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.z * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx_channelmap[3]] = 255;
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
rsvg_filter_primitive_diffuse_lighting_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveDiffuseLighting *filter = impl;
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

        case RSVG_ATTRIBUTE_KERNEL_UNIT_LENGTH:
            rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy);
            break;

        case RSVG_ATTRIBUTE_DIFFUSE_CONSTANT:
            filter->diffuseConstant = g_ascii_strtod (value, NULL);
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
rsvg_new_filter_primitive_diffuse_lighting (const char *element_name, RsvgNode *parent, const char *id, const char *klass)
{
    RsvgFilterPrimitiveDiffuseLighting *filter;

    filter = g_new0 (RsvgFilterPrimitiveDiffuseLighting, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->surfaceScale = 1;
    filter->diffuseConstant = 1;
    filter->dx = 1;
    filter->dy = 1;
    filter->super.render = rsvg_filter_primitive_diffuse_lighting_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_DIFFUSE_LIGHTING,
                                parent,
                                id,
                                klass,
                                filter,
                                rsvg_filter_primitive_diffuse_lighting_set_atts,
                                rsvg_filter_primitive_free);
}
