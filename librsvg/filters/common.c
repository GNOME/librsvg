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

void
rsvg_filter_primitive_free (gpointer impl)
{
    RsvgFilterPrimitive *primitive = impl;

    if (primitive->in) {
        g_string_free (primitive->in, TRUE);
    }

    if (primitive->result) {
        g_string_free (primitive->result, TRUE);
    }

    g_free (primitive);
}

void
filter_primitive_set_x_y_width_height_atts (RsvgFilterPrimitive *prim, RsvgPropertyBag *atts)
{
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_X:
            prim->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            prim->x_specified = TRUE;
            break;

        case RSVG_ATTRIBUTE_Y:
            prim->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            prim->y_specified = TRUE;
            break;

        case RSVG_ATTRIBUTE_WIDTH:
            prim->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            prim->width_specified = TRUE;
            break;

        case RSVG_ATTRIBUTE_HEIGHT:
            prim->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            prim->height_specified = TRUE;
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

static void
rsvg_filter_primitive_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    primitive->render (node, primitive, ctx);
}

RsvgIRect
rsvg_filter_primitive_get_bounds (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgBbox *box, *otherbox;
    cairo_matrix_t affine;
    cairo_rectangle_t rect;

    cairo_matrix_init_identity (&affine);
    box = rsvg_bbox_new (&affine, NULL);

    if (ctx->filter->filterunits == objectBoundingBox)
        rsvg_drawing_ctx_push_view_box (ctx->ctx, 1., 1.);
    rect.x = rsvg_length_normalize (&ctx->filter->x, ctx->ctx);
    rect.y = rsvg_length_normalize (&ctx->filter->y, ctx->ctx);
    rect.width = rsvg_length_normalize (&ctx->filter->width, ctx->ctx);
    rect.height = rsvg_length_normalize (&ctx->filter->height, ctx->ctx);
    if (ctx->filter->filterunits == objectBoundingBox)
        rsvg_drawing_ctx_pop_view_box (ctx->ctx);

    otherbox = rsvg_bbox_new (&ctx->affine, &rect);
    rsvg_bbox_insert (box, otherbox);
    rsvg_bbox_free (otherbox);

    if (self != NULL) {
        if (self->x_specified || self->y_specified || self->width_specified || self->height_specified) {
            if (ctx->filter->primitiveunits == objectBoundingBox)
                rsvg_drawing_ctx_push_view_box (ctx->ctx, 1., 1.);

            rect.x = self->x_specified ? rsvg_length_normalize (&self->x, ctx->ctx) : 0;
            rect.y = self->y_specified ? rsvg_length_normalize (&self->y, ctx->ctx) : 0;

            if (self->width_specified || self->height_specified) {
                double curr_vbox_w, curr_vbox_h;

                rsvg_drawing_ctx_get_view_box_size (ctx->ctx, &curr_vbox_w, &curr_vbox_h);

                if (self->width_specified)
                    rect.width = rsvg_length_normalize (&self->width, ctx->ctx);
                else
                    rect.width = curr_vbox_w;

                if (self->height_specified)
                    rect.height = rsvg_length_normalize (&self->height, ctx->ctx);
                else
                    rect.height = curr_vbox_h;
            }

            if (ctx->filter->primitiveunits == objectBoundingBox)
                rsvg_drawing_ctx_pop_view_box (ctx->ctx);

            otherbox = rsvg_bbox_new (&ctx->paffine, &rect);
            rsvg_bbox_clip (box, otherbox);
            rsvg_bbox_free (otherbox);
        }
    }

    rect.x = 0;
    rect.y = 0;
    rect.width = ctx->width;
    rect.height = ctx->height;

    otherbox = rsvg_bbox_new (&affine, &rect);
    rsvg_bbox_clip (box, otherbox);
    rsvg_bbox_free (otherbox);

    {
        cairo_rectangle_t box_rect;

        rsvg_bbox_get_rect (box, &box_rect);
        RsvgIRect output = {
            box_rect.x,
            box_rect.y,
            box_rect.x + box_rect.width,
            box_rect.y + box_rect.height
        };

        return output;
    }
}

cairo_surface_t *
_rsvg_image_surface_new (int width, int height)
{
    cairo_surface_t *surface;

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return NULL;
    }

    return surface;
}

guchar
get_interp_pixel (guchar * src, gdouble ox, gdouble oy, guchar ch, RsvgIRect boundarys,
                  guint rowstride)
{
    double xmod, ymod;
    double dist1, dist2, dist3, dist4;
    double c, c1, c2, c3, c4;
    double fox, foy, cox, coy;

    xmod = fmod (ox, 1.0);
    ymod = fmod (oy, 1.0);

    dist1 = (1 - xmod) * (1 - ymod);
    dist2 = (xmod) * (1 - ymod);
    dist3 = (xmod) * (ymod);
    dist4 = (1 - xmod) * (ymod);

    fox = floor (ox);
    foy = floor (oy);
    cox = ceil (ox);
    coy = ceil (oy);

    if (fox <= boundarys.x0 || fox >= boundarys.x1 ||
        foy <= boundarys.y0 || foy >= boundarys.y1)
        c1 = 0;
    else
        c1 = src[(guint) foy * rowstride + (guint) fox * 4 + ch];

    if (cox <= boundarys.x0 || cox >= boundarys.x1 ||
        foy <= boundarys.y0 || foy >= boundarys.y1)
        c2 = 0;
    else
        c2 = src[(guint) foy * rowstride + (guint) cox * 4 + ch];

    if (cox <= boundarys.x0 || cox >= boundarys.x1 ||
        coy <= boundarys.y0 || coy >= boundarys.y1)
        c3 = 0;
    else
        c3 = src[(guint) coy * rowstride + (guint) cox * 4 + ch];

    if (fox <= boundarys.x0 || fox >= boundarys.x1 ||
        coy <= boundarys.y0 || coy >= boundarys.y1)
        c4 = 0;
    else
        c4 = src[(guint) coy * rowstride + (guint) fox * 4 + ch];

    c = (c1 * dist1 + c2 * dist2 + c3 * dist3 + c4 * dist4) / (dist1 + dist2 + dist3 + dist4);

    return (guchar) c;
}

void
rsvg_filter_fix_coordinate_system (RsvgFilterContext * ctx, RsvgState * state, RsvgBbox *bbox)
{
    cairo_rectangle_t rect;
    int x, y, width, height;

    rsvg_bbox_get_rect (bbox, &rect);
    x = rect.x;
    y = rect.y;
    width = rect.width;
    height = rect.height;

    ctx->width = cairo_image_surface_get_width (ctx->source_surface);
    ctx->height = cairo_image_surface_get_height (ctx->source_surface);

    ctx->affine = rsvg_state_get_affine (state);
    if (ctx->filter->filterunits == objectBoundingBox) {
        cairo_matrix_t affine;
        cairo_matrix_init (&affine, width, 0, 0, height, x, y);
        cairo_matrix_multiply (&ctx->affine, &affine, &ctx->affine);
    }
    ctx->paffine = rsvg_state_get_affine (state);
    if (ctx->filter->primitiveunits == objectBoundingBox) {
        cairo_matrix_t affine;
        cairo_matrix_init (&affine, width, 0, 0, height, x, y);
        cairo_matrix_multiply (&ctx->paffine, &affine, &ctx->paffine);
    }
}

static gboolean
rectangle_intersect (gint ax, gint ay, gint awidth, gint aheight,
                     gint bx, gint by, gint bwidth, gint bheight,
                     gint *rx, gint *ry, gint *rwidth, gint *rheight)
{
    gint rx1, ry1, rx2, ry2;

    rx1 = MAX (ax, bx);
    ry1 = MAX (ay, by);
    rx2 = MIN (ax + awidth, bx + bwidth);
    ry2 = MIN (ay + aheight, by + bheight);

    if (rx2 > rx1 && ry2 > ry1) {
        *rx = rx1;
        *ry = ry1;
        *rwidth = rx2 - rx1;
        *rheight = ry2 - ry1;

        return TRUE;
    } else {
        *rx = *ry = *rwidth = *rheight = 0;

        return FALSE;
    }
}

void
rsvg_alpha_blt (cairo_surface_t *src,
                gint srcx,
                gint srcy,
                gint srcwidth,
                gint srcheight,
                cairo_surface_t *dst,
                gint dstx,
                gint dsty)
{
    gint src_surf_width, src_surf_height;
    gint dst_surf_width, dst_surf_height;
    gint src_clipped_x, src_clipped_y, src_clipped_width, src_clipped_height;
    gint dst_clipped_x, dst_clipped_y, dst_clipped_width, dst_clipped_height;
    gint x, y, srcrowstride, dstrowstride, sx, sy, dx, dy;
    guchar *src_pixels, *dst_pixels;

    g_assert (cairo_image_surface_get_format (src) == CAIRO_FORMAT_ARGB32);
    g_assert (cairo_image_surface_get_format (dst) == CAIRO_FORMAT_ARGB32);

    cairo_surface_flush (src);

    src_surf_width  = cairo_image_surface_get_width (src);
    src_surf_height = cairo_image_surface_get_height (src);

    dst_surf_width  = cairo_image_surface_get_width (dst);
    dst_surf_height = cairo_image_surface_get_height (dst);

    if (!rectangle_intersect (0, 0, src_surf_width, src_surf_height,
                              srcx, srcy, srcwidth, srcheight,
                              &src_clipped_x, &src_clipped_y, &src_clipped_width, &src_clipped_height))
        return; /* source rectangle is not in source surface */

    if (!rectangle_intersect (0, 0, dst_surf_width, dst_surf_height,
                              dstx, dsty, src_clipped_width, src_clipped_height,
                              &dst_clipped_x, &dst_clipped_y, &dst_clipped_width, &dst_clipped_height))
        return; /* dest rectangle is not in dest surface */

    srcrowstride = cairo_image_surface_get_stride (src);
    dstrowstride = cairo_image_surface_get_stride (dst);

    src_pixels = cairo_image_surface_get_data (src);
    dst_pixels = cairo_image_surface_get_data (dst);

    for (y = 0; y < dst_clipped_height; y++)
        for (x = 0; x < dst_clipped_width; x++) {
            guint a, c, ad, cd, ar, cr, i;

            sx = x + src_clipped_x;
            sy = y + src_clipped_y;
            dx = x + dst_clipped_x;
            dy = y + dst_clipped_y;
            a = src_pixels[4 * sx + sy * srcrowstride + 3];

            if (a) {
                ad = dst_pixels[4 * dx + dy * dstrowstride + 3];
                ar = a + ad * (255 - a) / 255;
                dst_pixels[4 * dx + dy * dstrowstride + 3] = ar;
                for (i = 0; i < 3; i++) {
                    c = src_pixels[4 * sx + sy * srcrowstride + i];
                    cd = dst_pixels[4 * dx + dy * dstrowstride + i];
                    cr = c + cd * (255 - a) / 255;
                    dst_pixels[4 * dx + dy * dstrowstride + i] = cr;
                }
            }
        }

    cairo_surface_mark_dirty (dst);
}

gboolean
rsvg_art_affine_image (cairo_surface_t *img,
                       cairo_surface_t *intermediate,
                       cairo_matrix_t *affine,
                       double w,
                       double h)
{
    cairo_matrix_t inv_affine, raw_inv_affine;
    gint intstride;
    gint basestride;
    gint basex, basey;
    gdouble fbasex, fbasey;
    gdouble rawx, rawy;
    guchar *intpix;
    guchar *basepix;
    gint i, j, k, basebpp, ii, jj;
    gboolean has_alpha;
    gdouble pixsum[4];
    gboolean xrunnoff, yrunnoff;
    gint iwidth, iheight;
    gint width, height;

    g_assert (cairo_image_surface_get_format (intermediate) == CAIRO_FORMAT_ARGB32);

    cairo_surface_flush (img);

    width = cairo_image_surface_get_width (img);
    height = cairo_image_surface_get_height (img);
    iwidth = cairo_image_surface_get_width (intermediate);
    iheight = cairo_image_surface_get_height (intermediate);

    has_alpha = cairo_image_surface_get_format (img) == CAIRO_FORMAT_ARGB32;

    basestride = cairo_image_surface_get_stride (img);
    intstride = cairo_image_surface_get_stride (intermediate);
    basepix = cairo_image_surface_get_data (img);
    intpix = cairo_image_surface_get_data (intermediate);
    basebpp = has_alpha ? 4 : 3;

    raw_inv_affine = *affine;
    if (cairo_matrix_invert (&raw_inv_affine) != CAIRO_STATUS_SUCCESS)
      return FALSE;

    cairo_matrix_init_scale (&inv_affine, w, h);
    cairo_matrix_multiply (&inv_affine, &inv_affine, affine);
    if (cairo_matrix_invert (&inv_affine) != CAIRO_STATUS_SUCCESS)
      return FALSE;

    /*apply the transformation */
    for (i = 0; i < iwidth; i++)
        for (j = 0; j < iheight; j++) {
            fbasex = (inv_affine.xx * (double) i + inv_affine.xy * (double) j +
                      inv_affine.x0) * (double) width;
            fbasey = (inv_affine.yx * (double) i + inv_affine.yy * (double) j +
                      inv_affine.y0) * (double) height;
            basex = floor (fbasex);
            basey = floor (fbasey);
            rawx = raw_inv_affine.xx * i + raw_inv_affine.xy * j + raw_inv_affine.x0;
            rawy = raw_inv_affine.yx * i + raw_inv_affine.yy * j + raw_inv_affine.y0;
            if (rawx < 0 || rawy < 0 || rawx >= w ||
                rawy >= h || basex < 0 || basey < 0 || basex >= width || basey >= height) {
                for (k = 0; k < 4; k++)
                    intpix[i * 4 + j * intstride + k] = 0;
            } else {
                if (basex < 0 || basex + 1 >= width)
                    xrunnoff = TRUE;
                else
                    xrunnoff = FALSE;
                if (basey < 0 || basey + 1 >= height)
                    yrunnoff = TRUE;
                else
                    yrunnoff = FALSE;
                for (k = 0; k < basebpp; k++)
                    pixsum[k] = 0;
                for (ii = 0; ii < 2; ii++)
                    for (jj = 0; jj < 2; jj++) {
                        if (basex + ii < 0 || basey + jj < 0
                            || basex + ii >= width || basey + jj >= height);
                        else {
                            for (k = 0; k < basebpp; k++) {
                                pixsum[k] +=
                                    (double) basepix[basebpp * (basex + ii) +
                                                     (basey + jj) * basestride + k]
                                    * (xrunnoff ? 1 : fabs (fbasex - (double) (basex + (1 - ii))))
                                    * (yrunnoff ? 1 : fabs (fbasey - (double) (basey + (1 - jj))));
                            }
                        }
                    }
                for (k = 0; k < basebpp; k++)
                    intpix[i * 4 + j * intstride + k] = pixsum[k];
                if (!has_alpha)
                    intpix[i * 4 + j * intstride + 3] = 255;
            }

        }

    /* Don't need cairo_surface_mark_dirty(intermediate) here since
     * the only caller does further work and then calls that himself.
     */

    return TRUE;
}

void
rsvg_filter_free_pair (gpointer value)
{
    RsvgFilterPrimitiveOutput *output;

    output = (RsvgFilterPrimitiveOutput *) value;
    cairo_surface_destroy (output->surface);
    g_free (output);
}

void
rsvg_filter_context_free (RsvgFilterContext * ctx)
{
    if (!ctx)
        return;

    if (ctx->bg_surface)
        cairo_surface_destroy (ctx->bg_surface);

    g_free (ctx);
}

static gboolean
node_is_filter_primitive (RsvgNode *node)
{
    RsvgNodeType type = rsvg_node_get_type (node);

    return type > RSVG_NODE_TYPE_FILTER_PRIMITIVE_FIRST && type < RSVG_NODE_TYPE_FILTER_PRIMITIVE_LAST;
}

void
render_child_if_filter_primitive (RsvgNode *node, RsvgFilterContext *filter_ctx)
{
    if (node_is_filter_primitive (node)) {
        RsvgFilterPrimitive *primitive;

        primitive = rsvg_rust_cnode_get_impl (node);
        rsvg_filter_primitive_render (node, primitive, filter_ctx);
    }
}

/**
 * rsvg_filter_store_result:
 * @name: The name of the result
 * @result: The pointer to the result
 * @ctx: the context that this was called in
 *
 * Puts the new result into the hash for easy finding later, also
 * Stores it as the last result
 **/
void
rsvg_filter_store_output (GString * name, RsvgFilterPrimitiveOutput result, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveOutput *store;

    cairo_surface_destroy (ctx->lastresult.surface);

    store = g_new0 (RsvgFilterPrimitiveOutput, 1);
    *store = result;

    if (name->str[0] != '\0') {
        cairo_surface_reference (result.surface);        /* increments the references for the table */
        g_hash_table_insert (ctx->results, g_strdup (name->str), store);
    }

    cairo_surface_reference (result.surface);    /* increments the references for the last result */
    ctx->lastresult = result;
}

void
rsvg_filter_store_result (GString * name,
                          cairo_surface_t *surface,
                          RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveOutput output;
    output.bounds.x0 = 0;
    output.bounds.y0 = 0;
    output.bounds.x1 = ctx->width;
    output.bounds.y1 = ctx->height;
    output.surface = surface;

    rsvg_filter_store_output (name, output, ctx);
}

static cairo_surface_t *
surface_get_alpha (cairo_surface_t *source,
                   RsvgFilterContext * ctx)
{
    guchar *data;
    guchar *pbdata;
    gsize i, pbsize;
    cairo_surface_t *surface;

    if (source == NULL)
        return NULL;

    cairo_surface_flush (source);

    pbsize = cairo_image_surface_get_width (source) *
             cairo_image_surface_get_height (source);

    surface = _rsvg_image_surface_new (cairo_image_surface_get_width (source),
                                       cairo_image_surface_get_height (source));
    if (surface == NULL)
        return NULL;

    data = cairo_image_surface_get_data (surface);
    pbdata = cairo_image_surface_get_data (source);

    /* FIXMEchpe: rewrite this into nested width, height loops */
    for (i = 0; i < pbsize; i++)
        data[i * 4 + ctx->channelmap[3]] = pbdata[i * 4 + ctx->channelmap[3]];

    cairo_surface_mark_dirty (surface);
    return surface;
}

static cairo_surface_t *
rsvg_compile_bg (RsvgDrawingCtx * ctx)
{
    cairo_surface_t *surface;
    cairo_t *cr;
    double x, y;
    GList *i;

    surface = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (surface == NULL)
        return NULL;

    cr = cairo_create (surface);

    rsvg_drawing_ctx_get_offset (ctx, &x, &y);

    for (i = g_list_last (ctx->cr_stack); i != NULL; i = g_list_previous (i)) {
        cairo_t *draw = i->data;
        gboolean nest = draw != ctx->initial_cr;
        cairo_set_source_surface (cr, cairo_get_target (draw),
                                  nest ? 0 : -x,
                                  nest ? 0 : -y);
        cairo_paint (cr);
    }

    cairo_destroy (cr);

    return surface;
}

/**
 * rsvg_filter_get_bg:
 *
 * Returns: (transfer none) (nullable): a #cairo_surface_t, or %NULL
 */
static cairo_surface_t *
rsvg_filter_get_bg (RsvgFilterContext * ctx)
{
    if (!ctx->bg_surface)
        ctx->bg_surface = rsvg_compile_bg (ctx->ctx);

    return ctx->bg_surface;
}

/**
 * rsvg_filter_get_result:
 * @name: The name of the surface
 * @ctx: the context that this was called in
 *
 * Gets a surface for a primitive
 *
 * Returns: (nullable): a pointer to the result that the name refers to, a special
 * surface if the name is a special keyword or %NULL if nothing was found
 **/
RsvgFilterPrimitiveOutput
rsvg_filter_get_result (GString * name, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveOutput output;
    RsvgFilterPrimitiveOutput *outputpointer;
    output.bounds.x0 = output.bounds.x1 = output.bounds.y0 = output.bounds.y1 = 0;

    if (!strcmp (name->str, "SourceGraphic")) {
        output.surface = cairo_surface_reference (ctx->source_surface);
        return output;
    } else if (!strcmp (name->str, "BackgroundImage")) {
        output.surface = rsvg_filter_get_bg (ctx);
        if (output.surface)
            cairo_surface_reference (output.surface);
        return output;
    } else if (!strcmp (name->str, "") || !strcmp (name->str, "none")) {
        output = ctx->lastresult;
        cairo_surface_reference (output.surface);
        return output;
    } else if (!strcmp (name->str, "SourceAlpha")) {
        output.surface = surface_get_alpha (ctx->source_surface, ctx);
        return output;
    } else if (!strcmp (name->str, "BackgroundAlpha")) {
        output.surface = surface_get_alpha (rsvg_filter_get_bg (ctx), ctx);
        return output;
    }

    outputpointer = (RsvgFilterPrimitiveOutput *) (g_hash_table_lookup (ctx->results, name->str));

    if (outputpointer != NULL) {
        output = *outputpointer;
        cairo_surface_reference (output.surface);
        return output;
    }

    output.surface = NULL;
    return output;
}

cairo_surface_t *
rsvg_filter_get_in (GString * name, RsvgFilterContext * ctx)
{
    cairo_surface_t *surface;

    surface = rsvg_filter_get_result (name, ctx).surface;
    if (surface == NULL || cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        return NULL;
    }

    return surface;
}

void
rsvg_filter_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilter *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_FILTER_UNITS:
            if (!strcmp (value, "userSpaceOnUse"))
                filter->filterunits = userSpaceOnUse;
            else
                filter->filterunits = objectBoundingBox;
            break;

        case RSVG_ATTRIBUTE_PRIMITIVE_UNITS:
            if (!strcmp (value, "objectBoundingBox"))
                filter->primitiveunits = objectBoundingBox;
            else
                filter->primitiveunits = userSpaceOnUse;
            break;

        case RSVG_ATTRIBUTE_X:
            filter->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            break;

        case RSVG_ATTRIBUTE_Y:
            filter->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            break;

        case RSVG_ATTRIBUTE_WIDTH:
            filter->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            break;

        case RSVG_ATTRIBUTE_HEIGHT:
            filter->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

void
rsvg_filter_draw (RsvgNode *node,
                  gpointer impl,
                  RsvgDrawingCtx *ctx,
                  RsvgState *state,
                  int dominate,
                  gboolean clipping)
{
    /* nothing; filters are drawn in rsvg-drawing-ctx.c */
}

void
rsvg_filter_free (gpointer impl)
{
    RsvgFilter *filter = impl;

    g_free (filter);
}
