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

#include "rsvg-private.h"
#include "rsvg-filter.h"
#include "rsvg-styles.h"
#include "rsvg-image.h"
#include "rsvg-css.h"
#include "rsvg-cairo-render.h"

#include <string.h>

#include <math.h>


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveOutput RsvgFilterPrimitiveOutput;

struct _RsvgFilterPrimitiveOutput {
    cairo_surface_t *surface;
    RsvgIRect bounds;
};

typedef struct _RsvgFilterContext RsvgFilterContext;

struct _RsvgFilterContext {
    gint width, height;
    RsvgFilter *filter;
    GHashTable *results;
    cairo_surface_t *source_surface;
    cairo_surface_t *bg_surface;
    RsvgFilterPrimitiveOutput lastresult;
    cairo_matrix_t affine;
    cairo_matrix_t paffine;
    int channelmap[4];
    RsvgDrawingCtx *ctx;
};

typedef struct _RsvgFilterPrimitive RsvgFilterPrimitive;

/* We don't have real subclassing here.  If you derive something from
 * RsvgFilterPrimitive, and don't need any special code to free your
 * RsvgFilterPrimitiveFoo structure, you can just pass rsvg_filter_primitive_free
 * to rsvg_rust_cnode_new() for the destructor.  Otherwise, create a custom destructor like this:
 *
 *    static void
 *    rsvg_filter_primitive_foo_free (gpointer impl)
 *    {
 *        RsvgFilterPrimitiveFoo *foo = impl;
 *
 *        g_free (foo->my_custom_stuff);
 *        g_free (foo->more_custom_stuff);
 *        ... etc ...
 *
 *        rsvg_filter_primitive_free (impl);
 *    }
 *
 * That last call to rsvg_filter_primitive_free() will free the base RsvgFilterPrimitive's own fields,
 * and your whole structure itself, via g_free().
 */
struct _RsvgFilterPrimitive {
    RsvgLength x, y, width, height;
    gboolean x_specified;
    gboolean y_specified;
    gboolean width_specified;
    gboolean height_specified;
    GString *in;
    GString *result;

    void (*render) (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx);
};

/*************************************************************/
/*************************************************************/

static void
rsvg_filter_primitive_free (gpointer impl)
{
    RsvgFilterPrimitive *primitive = impl;

    g_string_free (primitive->in, TRUE);
    g_string_free (primitive->result, TRUE);

    g_free (primitive);
}

static void
filter_primitive_set_x_y_width_height_atts (RsvgFilterPrimitive *prim, RsvgPropertyBag *atts)
{
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "x"))) {
        prim->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        prim->x_specified = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "y"))) {
        prim->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        prim->y_specified = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "width"))) {
        prim->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        prim->width_specified = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "height"))) {
        prim->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        prim->height_specified = TRUE;
    }
}

static void
rsvg_filter_primitive_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    primitive->render (node, primitive, ctx);
}

static RsvgIRect
rsvg_filter_primitive_get_bounds (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgBbox box, otherbox;
    cairo_matrix_t affine;

    cairo_matrix_init_identity (&affine);
    rsvg_bbox_init (&box, &affine);
    rsvg_bbox_init (&otherbox, &ctx->affine);
    otherbox.virgin = 0;
    if (ctx->filter->filterunits == objectBoundingBox)
        rsvg_drawing_ctx_push_view_box (ctx->ctx, 1., 1.);
    otherbox.rect.x = rsvg_length_normalize (&ctx->filter->x, ctx->ctx);
    otherbox.rect.y = rsvg_length_normalize (&ctx->filter->y, ctx->ctx);
    otherbox.rect.width = rsvg_length_normalize (&ctx->filter->width, ctx->ctx);
    otherbox.rect.height = rsvg_length_normalize (&ctx->filter->height, ctx->ctx);
    if (ctx->filter->filterunits == objectBoundingBox)
        rsvg_drawing_ctx_pop_view_box (ctx->ctx);

    rsvg_bbox_insert (&box, &otherbox);

    if (self != NULL) {
        if (self->x_specified || self->y_specified || self->width_specified || self->height_specified) {
            rsvg_bbox_init (&otherbox, &ctx->paffine);
            otherbox.virgin = 0;
            if (ctx->filter->primitiveunits == objectBoundingBox)
                rsvg_drawing_ctx_push_view_box (ctx->ctx, 1., 1.);
            if (self->x_specified)
                otherbox.rect.x = rsvg_length_normalize (&self->x, ctx->ctx);
            else
                otherbox.rect.x = 0;
            if (self->y_specified)
                otherbox.rect.y = rsvg_length_normalize (&self->y, ctx->ctx);
            else
                otherbox.rect.y = 0;

            if (self->width_specified || self->height_specified) {
                double curr_vbox_w, curr_vbox_h;

                rsvg_drawing_ctx_get_view_box_size (ctx->ctx, &curr_vbox_w, &curr_vbox_h);

                if (self->width_specified)
                    otherbox.rect.width = rsvg_length_normalize (&self->width, ctx->ctx);
                else
                    otherbox.rect.width = curr_vbox_w;

                if (self->height_specified)
                    otherbox.rect.height = rsvg_length_normalize (&self->height, ctx->ctx);
                else
                    otherbox.rect.height = curr_vbox_h;
            }

            if (ctx->filter->primitiveunits == objectBoundingBox)
                rsvg_drawing_ctx_pop_view_box (ctx->ctx);
            rsvg_bbox_clip (&box, &otherbox);
        }
    }

    rsvg_bbox_init (&otherbox, &affine);
    otherbox.virgin = 0;
    otherbox.rect.x = 0;
    otherbox.rect.y = 0;
    otherbox.rect.width = ctx->width;
    otherbox.rect.height = ctx->height;
    rsvg_bbox_clip (&box, &otherbox);
    {
        RsvgIRect output = { box.rect.x, box.rect.y,
            box.rect.x + box.rect.width,
            box.rect.y + box.rect.height
        };
        return output;
    }
}

static cairo_surface_t *
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

static guchar
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

static void
rsvg_filter_fix_coordinate_system (RsvgFilterContext * ctx, RsvgState * state, RsvgBbox *bbox)
{
    int x, y, height, width;

    x = bbox->rect.x;
    y = bbox->rect.y;
    width = bbox->rect.width;
    height = bbox->rect.height;

    ctx->width = cairo_image_surface_get_width (ctx->source_surface);
    ctx->height = cairo_image_surface_get_height (ctx->source_surface);

    ctx->affine = state->affine;
    if (ctx->filter->filterunits == objectBoundingBox) {
        cairo_matrix_t affine;
        cairo_matrix_init (&affine, width, 0, 0, height, x, y);
        cairo_matrix_multiply (&ctx->affine, &affine, &ctx->affine);
    }
    ctx->paffine = state->affine;
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

static void
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

static gboolean
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

static void
rsvg_filter_free_pair (gpointer value)
{
    RsvgFilterPrimitiveOutput *output;

    output = (RsvgFilterPrimitiveOutput *) value;
    cairo_surface_destroy (output->surface);
    g_free (output);
}

static void
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

static gboolean
render_child_if_filter_primitive (RsvgNode *node, gpointer data)
{
    RsvgFilterContext *filter_ctx = data;

    if (node_is_filter_primitive (node)) {
        RsvgFilterPrimitive *primitive;

        primitive = rsvg_rust_cnode_get_impl (node);
        rsvg_filter_primitive_render (node, primitive, filter_ctx);
    }

    return TRUE;
}

/**
 * rsvg_filter_render:
 * @node: a pointer to the filter node to use
 * @source: the a #cairo_surface_t of type %CAIRO_SURFACE_TYPE_IMAGE
 * @context: the context
 *
 * Create a new surface applied the filter. This function will create
 * a context for itself, set up the coordinate systems execute all its
 * little primatives and then clean up its own mess.
 *
 * Returns: (transfer full): a new #cairo_surface_t
 **/
cairo_surface_t *
rsvg_filter_render (RsvgNode *filter_node,
                    cairo_surface_t *source,
                    RsvgDrawingCtx *context,
                    RsvgBbox *bounds,
                    char *channelmap)
{
    RsvgFilter *filter;
    RsvgFilterContext *ctx;
    guint i;
    cairo_surface_t *output;

    g_return_val_if_fail (source != NULL, NULL);
    g_return_val_if_fail (cairo_surface_get_type (source) == CAIRO_SURFACE_TYPE_IMAGE, NULL);

    g_assert (rsvg_node_get_type (filter_node) == RSVG_NODE_TYPE_FILTER);
    filter = rsvg_rust_cnode_get_impl (filter_node);

    ctx = g_new0 (RsvgFilterContext, 1);
    ctx->filter = filter;
    ctx->source_surface = source;
    ctx->bg_surface = NULL;
    ctx->results = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, rsvg_filter_free_pair);
    ctx->ctx = context;

    rsvg_filter_fix_coordinate_system (ctx, rsvg_current_state (context), bounds);

    ctx->lastresult.surface = cairo_surface_reference (source);
    ctx->lastresult.bounds = rsvg_filter_primitive_get_bounds (NULL, ctx);

    for (i = 0; i < 4; i++)
        ctx->channelmap[i] = channelmap[i] - '0';

    rsvg_node_foreach_child (filter_node, render_child_if_filter_primitive, ctx);

    output = ctx->lastresult.surface;

    g_hash_table_destroy (ctx->results);

    rsvg_filter_context_free (ctx);

    return output;
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
static void
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

static void
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
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);
    cairo_surface_t *surface;
    cairo_t *cr;
    GList *i;

    surface = _rsvg_image_surface_new (render->width, render->height);
    if (surface == NULL)
        return NULL;

    cr = cairo_create (surface);

    for (i = g_list_last (render->cr_stack); i != NULL; i = g_list_previous (i)) {
        cairo_t *draw = i->data;
        gboolean nest = draw != render->initial_cr;
        cairo_set_source_surface (cr, cairo_get_target (draw),
                                  nest ? 0 : -render->offset_x,
                                  nest ? 0 : -render->offset_y);
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

/* FIXMEchpe: proper return value and out param! */
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
static RsvgFilterPrimitiveOutput
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

    /* g_warning (_("%s not found\n"), name->str); */

    output = ctx->lastresult;
    cairo_surface_reference (output.surface);
    return output;
}

/**
 * rsvg_filter_get_in:
 * @name:
 * @ctx:
 *
 * Returns: (transfer full) (nullable): a new #cairo_surface_t, or %NULL
 */
static cairo_surface_t *
rsvg_filter_get_in (GString * name, RsvgFilterContext * ctx)
{
    return rsvg_filter_get_result (name, ctx).surface;
}

static void
rsvg_filter_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilter *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "filterUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            filter->filterunits = userSpaceOnUse;
        else
            filter->filterunits = objectBoundingBox;
    }
    if ((value = rsvg_property_bag_lookup (atts, "primitiveUnits"))) {
        if (!strcmp (value, "objectBoundingBox"))
            filter->primitiveunits = objectBoundingBox;
        else
            filter->primitiveunits = userSpaceOnUse;
    }
    if ((value = rsvg_property_bag_lookup (atts, "x")))
        filter->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        filter->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "width")))
        filter->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "height")))
        filter->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
}

static void
rsvg_filter_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing; filters are drawn in rsvg-cairo-draw.c */
}

static void
rsvg_filter_free (gpointer impl)
{
    RsvgFilter *filter = impl;

    g_free (filter);
}

/**
 * rsvg_new_filter:
 *
 * Creates a blank filter and assigns default values to everything
 **/
RsvgNode *
rsvg_new_filter (const char *element_name, RsvgNode *parent)
{
    RsvgFilter *filter;

    filter = g_new0 (RsvgFilter, 1);
    filter->filterunits = objectBoundingBox;
    filter->primitiveunits = userSpaceOnUse;
    filter->x = rsvg_length_parse ("-10%", LENGTH_DIR_HORIZONTAL);
    filter->y = rsvg_length_parse ("-10%", LENGTH_DIR_VERTICAL);
    filter->width = rsvg_length_parse ("120%", LENGTH_DIR_HORIZONTAL);
    filter->height = rsvg_length_parse ("120%", LENGTH_DIR_VERTICAL);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_free);
}

/*************************************************************/
/*************************************************************/

typedef enum {
    normal, multiply, screen, darken, lighten, softlight,
    hardlight, colordodge, colorburn, overlay, exclusion,
    difference
} RsvgFilterPrimitiveBlendMode;

typedef struct _RsvgFilterPrimitiveBlend RsvgFilterPrimitiveBlend;
struct _RsvgFilterPrimitiveBlend {
    RsvgFilterPrimitive super;
    RsvgFilterPrimitiveBlendMode mode;
    GString *in2;
};

static void
rsvg_filter_blend (RsvgFilterPrimitiveBlendMode mode,
                   cairo_surface_t *in,
                   cairo_surface_t *in2,
                   cairo_surface_t* output,
                   RsvgIRect boundarys,
                   int *channelmap)
{
    guchar i;
    gint x, y;
    gint rowstride, rowstride2, rowstrideo, height, width;
    guchar *in_pixels;
    guchar *in2_pixels;
    guchar *output_pixels;

    cairo_surface_flush (in);
    cairo_surface_flush (in2);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);
    rowstride = cairo_image_surface_get_stride (in);
    rowstride2 = cairo_image_surface_get_stride (in2);
    rowstrideo = cairo_image_surface_get_stride (output);

    output_pixels = cairo_image_surface_get_data (output);
    in_pixels = cairo_image_surface_get_data (in);
    in2_pixels = cairo_image_surface_get_data (in2);

    if (boundarys.x0 < 0)
        boundarys.x0 = 0;
    if (boundarys.y0 < 0)
        boundarys.y0 = 0;
    if (boundarys.x1 >= width)
        boundarys.x1 = width;
    if (boundarys.y1 >= height)
        boundarys.y1 = height;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            double qr, cr, qa, qb, ca, cb, bca, bcb;
            int ch;

            qa = (double) in_pixels[4 * x + y * rowstride + channelmap[3]] / 255.0;
            qb = (double) in2_pixels[4 * x + y * rowstride2 + channelmap[3]] / 255.0;
            qr = 1 - (1 - qa) * (1 - qb);
            cr = 0;
            for (ch = 0; ch < 3; ch++) {
                i = channelmap[ch];
                ca = (double) in_pixels[4 * x + y * rowstride + i] / 255.0;
                cb = (double) in2_pixels[4 * x + y * rowstride2 + i] / 255.0;
                /*these are the ca and cb that are used in the non-standard blend functions */
                bcb = (1 - qa) * cb + ca;
                bca = (1 - qb) * ca + cb;
                switch (mode) {
                case normal:
                    cr = (1 - qa) * cb + ca;
                    break;
                case multiply:
                    cr = (1 - qa) * cb + (1 - qb) * ca + ca * cb;
                    break;
                case screen:
                    cr = cb + ca - ca * cb;
                    break;
                case darken:
                    cr = MIN ((1 - qa) * cb + ca, (1 - qb) * ca + cb);
                    break;
                case lighten:
                    cr = MAX ((1 - qa) * cb + ca, (1 - qb) * ca + cb);
                    break;
                case softlight:
                    if (bcb < 0.5)
                        cr = 2 * bca * bcb + bca * bca * (1 - 2 * bcb);
                    else
                        cr = sqrt (bca) * (2 * bcb - 1) + (2 * bca) * (1 - bcb);
                    break;
                case hardlight:
                    if (cb < 0.5)
                        cr = 2 * bca * bcb;
                    else
                        cr = 1 - 2 * (1 - bca) * (1 - bcb);
                    break;
                case colordodge:
                    if (bcb == 1)
                        cr = 1;
                    else
                        cr = MIN (bca / (1 - bcb), 1);
                    break;
                case colorburn:
                    if (bcb == 0)
                        cr = 0;
                    else
                        cr = MAX (1 - (1 - bca) / bcb, 0);
                    break;
                case overlay:
                    if (bca < 0.5)
                        cr = 2 * bca * bcb;
                    else
                        cr = 1 - 2 * (1 - bca) * (1 - bcb);
                    break;
                case exclusion:
                    cr = bca + bcb - 2 * bca * bcb;
                    break;
                case difference:
                    cr = abs (bca - bcb);
                    break;
                }
                cr *= 255.0;
                if (cr > 255)
                    cr = 255;
                if (cr < 0)
                    cr = 0;
                output_pixels[4 * x + y * rowstrideo + i] = (guchar) cr;

            }
            output_pixels[4 * x + y * rowstrideo + channelmap[3]] = qr * 255.0;
        }

    cairo_surface_mark_dirty (output);
}

static void
rsvg_filter_primitive_blend_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveBlend *blend = (RsvgFilterPrimitiveBlend *) primitive;
    RsvgIRect boundarys;
    cairo_surface_t *output, *in, *in2;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
      return;

    in2 = rsvg_filter_get_in (blend->in2, ctx);
    if (in2 == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output = _rsvg_image_surface_new (cairo_image_surface_get_width (in),
                                      cairo_image_surface_get_height (in));
    if (output == NULL) {
        cairo_surface_destroy (in);
        cairo_surface_destroy (in2);
        return;
    }

    rsvg_filter_blend (blend->mode, in, in2, output, boundarys, ctx->channelmap);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_blend_free (gpointer impl)
{
    RsvgFilterPrimitiveBlend *blend = impl;

    g_string_free (blend->in2, TRUE);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_blend_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveBlend *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "mode"))) {
        if (!strcmp (value, "multiply"))
            filter->mode = multiply;
        else if (!strcmp (value, "screen"))
            filter->mode = screen;
        else if (!strcmp (value, "darken"))
            filter->mode = darken;
        else if (!strcmp (value, "lighten"))
            filter->mode = lighten;
        else
            filter->mode = normal;
    }
    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "in2")))
        g_string_assign (filter->in2, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
}

RsvgNode *
rsvg_new_filter_primitive_blend (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveBlend *filter;

    filter = g_new0 (RsvgFilterPrimitiveBlend, 1);
    filter->mode = normal;
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_blend_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_BLEND,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_blend_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_blend_free);
}

/*************************************************************/
/*************************************************************/

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
rsvg_filter_primitive_convolve_matrix_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveConvolveMatrix *filter = impl;
    gint i, j;
    const char *value;
    gboolean has_target_x, has_target_y;

    has_target_x = 0;
    has_target_y = 0;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "targetX"))) {
        has_target_x = 1;
        filter->targetx = atoi (value);
    }
    if ((value = rsvg_property_bag_lookup (atts, "targetY"))) {
        has_target_y = 1;
        filter->targety = atoi (value);
    }
    if ((value = rsvg_property_bag_lookup (atts, "bias")))
        filter->bias = atof (value);
    if ((value = rsvg_property_bag_lookup (atts, "preserveAlpha"))) {
        if (!strcmp (value, "true"))
            filter->preservealpha = TRUE;
        else
            filter->preservealpha = FALSE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "divisor")))
        filter->divisor = atof (value);

    if ((value = rsvg_property_bag_lookup (atts, "order"))) {
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
                return;
            }
        } else {
            rsvg_node_set_attribute_parse_error (node, "order", "invalid size for convolve matrix");
            return;
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "kernelUnitLength"))) {
        if (!rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy)) {
            rsvg_node_set_attribute_parse_error (node, "kernelUnitLength", "expected number-optional-number");
            return;
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "kernelMatrix"))) {
        gsize num_elems;
        gsize got_num_elems;

        num_elems = filter->orderx * filter->ordery;

        if (!rsvg_css_parse_number_list (value,
                                         NUMBER_LIST_LENGTH_EXACT,
                                         num_elems,
                                         &filter->KernelMatrix,
                                         &got_num_elems)) {
            rsvg_node_set_attribute_parse_error (node, "kernelMatrix", "expected a matrix of numbers");
            return;
        }

        g_assert (num_elems == got_num_elems);
    }

    if ((value = rsvg_property_bag_lookup (atts, "edgeMode"))) {
        if (!strcmp (value, "duplicate")) {
            filter->edgemode = EDGE_MODE_DUPLICATE;
        } else if (!strcmp (value, "wrap")) {
            filter->edgemode = EDGE_MODE_WRAP;
        } else if (!strcmp (value, "none")) {
            filter->edgemode = EDGE_MODE_NONE;
        } else {
            rsvg_node_set_attribute_parse_error (node, "edgeMode", "expected 'duplicate' | 'wrap' | 'none'");
            return;
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

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveGaussianBlur
 RsvgFilterPrimitiveGaussianBlur;

struct _RsvgFilterPrimitiveGaussianBlur {
    RsvgFilterPrimitive super;
    double sdx, sdy;
};

static void
box_blur_line (gint box_width, gint even_offset,
               guchar *src, guchar *dest,
               gint len, gint bpp)
{
    gint  i;
    gint  lead;    /* This marks the leading edge of the kernel              */
    gint  output;  /* This marks the center of the kernel                    */
    gint  trail;   /* This marks the pixel BEHIND the last 1 in the
                      kernel; it's the pixel to remove from the accumulator. */
    gint  *ac;     /* Accumulator for each channel                           */

    ac = g_new0 (gint, bpp);

    /* The algorithm differs for even and odd-sized kernels.
     * With the output at the center,
     * If odd, the kernel might look like this: 0011100
     * If even, the kernel will either be centered on the boundary between
     * the output and its left neighbor, or on the boundary between the
     * output and its right neighbor, depending on even_lr.
     * So it might be 0111100 or 0011110, where output is on the center
     * of these arrays.
     */
    lead = 0;

    if (box_width % 2 != 0) {
        /* Odd-width kernel */
        output = lead - (box_width - 1) / 2;
        trail  = lead - box_width;
    } else {
        /* Even-width kernel. */
        if (even_offset == 1) {
            /* Right offset */
            output = lead + 1 - box_width / 2;
            trail  = lead - box_width;
        } else if (even_offset == -1) {
            /* Left offset */
            output = lead - box_width / 2;
            trail  = lead - box_width;
        } else {
            /* If even_offset isn't 1 or -1, there's some error. */
            g_assert_not_reached ();
        }
    }

    /* Initialize accumulator */
    for (i = 0; i < bpp; i++)
        ac[i] = 0;

    /* As the kernel moves across the image, it has a leading edge and a
     * trailing edge, and the output is in the middle. */
    while (output < len) {
        /* The number of pixels that are both in the image and
         * currently covered by the kernel. This is necessary to
         * handle edge cases. */
        guint coverage = (lead < len ? lead : len - 1) - (trail >= 0 ? trail : -1);

#ifdef READABLE_BOXBLUR_CODE
/* The code here does the same as the code below, but the code below
 * has been optimized by moving the if statements out of the tight for
 * loop, and is harder to understand.
 * Don't use both this code and the code below. */
        for (i = 0; i < bpp; i++) {
            /* If the leading edge of the kernel is still on the image,
             * add the value there to the accumulator. */
            if (lead < len)
                ac[i] += src[bpp * lead + i];

            /* If the trailing edge of the kernel is on the image,
             * subtract the value there from the accumulator. */
            if (trail >= 0)
                ac[i] -= src[bpp * trail + i];

            /* Take the averaged value in the accumulator and store
             * that value in the output. The number of pixels currently
             * stored in the accumulator can be less than the nominal
             * width of the kernel because the kernel can go "over the edge"
             * of the image. */
            if (output >= 0)
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
        }
#endif

        /* If the leading edge of the kernel is still on the image... */
        if (lead < len) {
            if (trail >= 0) {
                /* If the trailing edge of the kernel is on the image. (Since
                 * the output is in between the lead and trail, it must be on
                 * the image. */
                for (i = 0; i < bpp; i++) {
                    ac[i] += src[bpp * lead + i];
                    ac[i] -= src[bpp * trail + i];
                    dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
                }
            } else if (output >= 0) {
                /* If the output is on the image, but the trailing edge isn't yet
                 * on the image. */

                for (i = 0; i < bpp; i++) {
                    ac[i] += src[bpp * lead + i];
                    dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
                }
            } else {
                /* If leading edge is on the image, but the output and trailing
                 * edge aren't yet on the image. */
                for (i = 0; i < bpp; i++)
                    ac[i] += src[bpp * lead + i];
            }
        } else if (trail >= 0) {
            /* If the leading edge has gone off the image, but the output and
             * trailing edge are on the image. (The big loop exits when the
             * output goes off the image. */
            for (i = 0; i < bpp; i++) {
                ac[i] -= src[bpp * trail + i];
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
            }
        } else if (output >= 0) {
            /* Leading has gone off the image and trailing isn't yet in it
             * (small image) */
            for (i = 0; i < bpp; i++)
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
        }

        lead++;
        output++;
        trail++;
    }

    g_free (ac);
}

static gint
compute_box_blur_width (double radius)
{
    double width;

    width = radius * 3 * sqrt (2 * G_PI) / 4;
    return (gint) (width + 0.5);
}

#define SQR(x) ((x) * (x))

static void
make_gaussian_convolution_matrix (gdouble radius, gdouble **out_matrix, gint *out_matrix_len)
{
    gdouble *matrix;
    gdouble std_dev;
    gdouble sum;
    gint matrix_len;
    gint i, j;

    std_dev = radius + 1.0;
    radius = std_dev * 2;

    matrix_len = 2 * ceil (radius - 0.5) + 1;
    if (matrix_len <= 0)
        matrix_len = 1;

    matrix = g_new0 (gdouble, matrix_len);

    /* Fill the matrix by doing numerical integration approximation
     * from -2*std_dev to 2*std_dev, sampling 50 points per pixel.
     * We do the bottom half, mirror it to the top half, then compute the
     * center point.  Otherwise asymmetric quantization errors will occur.
     * The formula to integrate is e^-(x^2/2s^2).
     */

    for (i = matrix_len / 2 + 1; i < matrix_len; i++)
    {
        gdouble base_x = i - (matrix_len / 2) - 0.5;

        sum = 0;
        for (j = 1; j <= 50; j++)
        {
            gdouble r = base_x + 0.02 * j;

            if (r <= radius)
                sum += exp (- SQR (r) / (2 * SQR (std_dev)));
        }

        matrix[i] = sum / 50;
    }

    /* mirror to the bottom half */
    for (i = 0; i <= matrix_len / 2; i++)
        matrix[i] = matrix[matrix_len - 1 - i];

    /* find center val -- calculate an odd number of quanta to make it
     * symmetric, even if the center point is weighted slightly higher
     * than others.
     */
    sum = 0;
    for (j = 0; j <= 50; j++)
        sum += exp (- SQR (- 0.5 + 0.02 * j) / (2 * SQR (std_dev)));

    matrix[matrix_len / 2] = sum / 51;

    /* normalize the distribution by scaling the total sum to one */
    sum = 0;
    for (i = 0; i < matrix_len; i++)
        sum += matrix[i];

    for (i = 0; i < matrix_len; i++)
        matrix[i] = matrix[i] / sum;

    *out_matrix = matrix;
    *out_matrix_len = matrix_len;
}

static void
gaussian_blur_line (gdouble *matrix,
                    gint matrix_len,
                    guchar *src,
                    guchar *dest,
                    gint len,
                    gint bpp)
{
    guchar *src_p;
    guchar *src_p1;
    gint matrix_middle;
    gint row;
    gint i, j;

    matrix_middle = matrix_len / 2;

    /* picture smaller than the matrix? */
    if (matrix_len > len) {
        for (row = 0; row < len; row++) {
            /* find the scale factor */
            gdouble scale = 0;

            for (j = 0; j < len; j++) {
                /* if the index is in bounds, add it to the scale counter */
                if (j + matrix_middle - row >= 0 &&
                    j + matrix_middle - row < matrix_len)
                    scale += matrix[j];
            }

            src_p = src;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < len; j++) {
                    if (j + matrix_middle - row >= 0 &&
                        j + matrix_middle - row < matrix_len)
                        sum += *src_p1 * matrix[j];

                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }
    } else {
        /* left edge */

        for (row = 0; row < matrix_middle; row++) {
            /* find scale factor */
            gdouble scale = 0;

            for (j = matrix_middle - row; j < matrix_len; j++)
                scale += matrix[j];

            src_p = src;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = matrix_middle - row; j < matrix_len; j++) {
                    sum += *src_p1 * matrix[j];
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }

        /* go through each pixel in each col */
        for (; row < len - matrix_middle; row++) {
            src_p = src + (row - matrix_middle) * bpp;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < matrix_len; j++) {
                    sum += matrix[j] * *src_p1;
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum + 0.5);
            }
        }

        /* for the edge condition, we only use available info and scale to one */
        for (; row < len; row++) {
            /* find scale factor */
            gdouble scale = 0;

            for (j = 0; j < len - row + matrix_middle; j++)
                scale += matrix[j];

            src_p = src + (row - matrix_middle) * bpp;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < len - row + matrix_middle; j++) {
                    sum += *src_p1 * matrix[j];
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }
    }
}

static void
get_column (guchar *column_data,
            guchar *src_data,
            gint src_stride,
            gint bpp,
            gint height,
            gint x)
{
    gint y;
    gint c;

    for (y = 0; y < height; y++) {
        guchar *src = src_data + y * src_stride + x * bpp;

        for (c = 0; c < bpp; c++)
            column_data[c] = src[c];

        column_data += bpp;
    }
}

static void
put_column (guchar *column_data, guchar *dest_data, gint dest_stride, gint bpp, gint height, gint x)
{
    gint y;
    gint c;

    for (y = 0; y < height; y++) {
        guchar *dst = dest_data + y * dest_stride + x * bpp;

        for (c = 0; c < bpp; c++)
            dst[c] = column_data[c];

        column_data += bpp;
    }
}

static void
gaussian_blur_surface (cairo_surface_t *in,
                       cairo_surface_t *out,
                       gdouble sx,
                       gdouble sy)
{
    gboolean use_box_blur;
    gint width, height;
    cairo_format_t in_format, out_format;
    gint in_stride;
    gint out_stride;
    guchar *in_data, *out_data;
    gint bpp;
    gboolean out_has_data;

    cairo_surface_flush (in);

    width = cairo_image_surface_get_width (in);
    height = cairo_image_surface_get_height (in);

    g_assert (width == cairo_image_surface_get_width (out)
              && height == cairo_image_surface_get_height (out));

    in_format = cairo_image_surface_get_format (in);
    out_format = cairo_image_surface_get_format (out);
    g_assert (in_format == out_format);
    g_assert (in_format == CAIRO_FORMAT_ARGB32
              || in_format == CAIRO_FORMAT_A8);

    if (in_format == CAIRO_FORMAT_ARGB32)
        bpp = 4;
    else if (in_format == CAIRO_FORMAT_A8)
        bpp = 1;
    else {
        g_assert_not_reached ();
        return;
    }

    in_stride = cairo_image_surface_get_stride (in);
    out_stride = cairo_image_surface_get_stride (out);

    in_data = cairo_image_surface_get_data (in);
    out_data = cairo_image_surface_get_data (out);

    if (sx < 0.0)
        sx = 0.0;

    if (sy < 0.0)
        sy = 0.0;

    /* For small radiuses, use a true gaussian kernel; otherwise use three box blurs with
     * clever offsets.
     */
    if (sx < 10.0 && sy < 10.0)
        use_box_blur = FALSE;
    else
        use_box_blur = TRUE;

    /* Bail out by just copying? */
    if ((sx == 0.0 && sy == 0.0)
        || sx > 1000 || sy > 1000) {
        cairo_t *cr;

        cr = cairo_create (out);
        cairo_set_source_surface (cr, in, 0, 0);
        cairo_paint (cr);
        cairo_destroy (cr);
        return;
    }

    if (sx != 0.0) {
        gint box_width;
        gdouble *gaussian_matrix;
        gint gaussian_matrix_len;
        int y;
        guchar *row_buffer = NULL;
        guchar *row1, *row2;

        if (use_box_blur) {
            box_width = compute_box_blur_width (sx);

            /* twice the size so we can have "two" scratch rows */
            row_buffer = g_new0 (guchar, width * bpp * 2);
            row1 = row_buffer;
            row2 = row_buffer + width * bpp;
        } else
            make_gaussian_convolution_matrix (sx, &gaussian_matrix, &gaussian_matrix_len);

        for (y = 0; y < height; y++) {
            guchar *in_row, *out_row;

            in_row = in_data + in_stride * y;
            out_row = out_data + out_stride * y;

            if (use_box_blur) {
                if (box_width % 2 != 0) {
                    /* Odd-width box blur: repeat 3 times, centered on output pixel */

                    box_blur_line (box_width, 0, in_row, row1,    width, bpp);
                    box_blur_line (box_width, 0, row1,   row2,    width, bpp);
                    box_blur_line (box_width, 0, row2,   out_row, width, bpp);
                } else {
                    /* Even-width box blur:
                     * This method is suggested by the specification for SVG.
                     * One pass with width n, centered between output and right pixel
                     * One pass with width n, centered between output and left pixel
                     * One pass with width n+1, centered on output pixel
                     */
                    box_blur_line (box_width,     -1, in_row, row1,    width, bpp);
                    box_blur_line (box_width,      1, row1,   row2,    width, bpp);
                    box_blur_line (box_width + 1,  0, row2,   out_row, width, bpp);
                }
            } else
                gaussian_blur_line (gaussian_matrix, gaussian_matrix_len, in_row, out_row, width, bpp);
        }

        if (!use_box_blur)
            g_free (gaussian_matrix);

        g_free (row_buffer);

        out_has_data = TRUE;
    } else
        out_has_data = FALSE;

    if (sy != 0.0) {
        gint box_height;
        gdouble *gaussian_matrix = NULL;
        gint gaussian_matrix_len;
        guchar *col_buffer;
        guchar *col1, *col2;
        int x;

        /* twice the size so we can have the source pixels and the blurred pixels */
        col_buffer = g_new0 (guchar, height * bpp * 2);
        col1 = col_buffer;
        col2 = col_buffer + height * bpp;

        if (use_box_blur) {
            box_height = compute_box_blur_width (sy);
        } else
            make_gaussian_convolution_matrix (sy, &gaussian_matrix, &gaussian_matrix_len);

        for (x = 0; x < width; x++) {
            if (out_has_data)
                get_column (col1, out_data, out_stride, bpp, height, x);
            else
                get_column (col1, in_data, in_stride, bpp, height, x);

            if (use_box_blur) {
                if (box_height % 2 != 0) {
                    /* Odd-width box blur */
                    box_blur_line (box_height, 0, col1, col2, height, bpp);
                    box_blur_line (box_height, 0, col2, col1, height, bpp);
                    box_blur_line (box_height, 0, col1, col2, height, bpp);
                } else {
                    /* Even-width box blur */
                    box_blur_line (box_height,     -1, col1, col2, height, bpp);
                    box_blur_line (box_height,      1, col2, col1, height, bpp);
                    box_blur_line (box_height + 1,  0, col1, col2, height, bpp);
                }
            } else
                gaussian_blur_line (gaussian_matrix, gaussian_matrix_len, col1, col2, height, bpp);

            put_column (col2, out_data, out_stride, bpp, height, x);
        }

        g_free (gaussian_matrix);
        g_free (col_buffer);
    }

    cairo_surface_mark_dirty (out);
}

static void
rsvg_filter_primitive_gaussian_blur_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveGaussianBlur *gaussian = (RsvgFilterPrimitiveGaussianBlur *) primitive;

    int width, height;
    cairo_surface_t *output, *in;
    RsvgIRect boundarys;
    gdouble sdx, sdy;
    RsvgFilterPrimitiveOutput op;
    cairo_t *cr;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    op = rsvg_filter_get_result (primitive->in, ctx);
    in = op.surface;

    width = cairo_image_surface_get_width (in);
    height = cairo_image_surface_get_height (in);

    output = _rsvg_image_surface_new (width, height);

    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    /* scale the SD values */
    sdx = fabs (gaussian->sdx * ctx->paffine.xx);
    sdy = fabs (gaussian->sdy * ctx->paffine.yy);

    gaussian_blur_surface (in, output, sdx, sdy);

    /* Hard-clip to the filter area */
    if (!(boundarys.x0 == 0
          && boundarys.y0 == 0
          && boundarys.x1 == width
          && boundarys.y1 == height)) {
        cr = cairo_create (output);
        cairo_set_operator (cr, CAIRO_OPERATOR_CLEAR);
        cairo_set_fill_rule (cr, CAIRO_FILL_RULE_EVEN_ODD);
        cairo_rectangle (cr, 0, 0, width, height);
        cairo_rectangle (cr,
                         boundarys.x0, boundarys.y0,
                         boundarys.x1 - boundarys.x0, boundarys.y1 - boundarys.y0);
        cairo_fill (cr);
        cairo_destroy (cr);
    }

    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output (primitive->result, op, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_gaussian_blur_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveGaussianBlur *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "stdDeviation"))) {
        if (!rsvg_css_parse_number_optional_number (value, &filter->sdx, &filter->sdy)) {
            rsvg_node_set_attribute_parse_error (node, "stdDeviation", "expected number-optional-number");
            return;
        }
    }
}

RsvgNode *
rsvg_new_filter_primitive_gaussian_blur (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveGaussianBlur *filter;

    filter = g_new0 (RsvgFilterPrimitiveGaussianBlur, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->sdx = 0;
    filter->sdy = 0;
    filter->super.render = rsvg_filter_primitive_gaussian_blur_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_GAUSSIAN_BLUR,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_gaussian_blur_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveOffset RsvgFilterPrimitiveOffset;

struct _RsvgFilterPrimitiveOffset {
    RsvgFilterPrimitive super;
    RsvgLength dx, dy;
};

static void
rsvg_filter_primitive_offset_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveOffset *offset = (RsvgFilterPrimitiveOffset *) primitive;

    guchar ch;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveOutput out;

    cairo_surface_t *output, *in;

    double dx, dy;
    int ox, oy;

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

    dx = rsvg_length_normalize (&offset->dx, ctx->ctx);
    dy = rsvg_length_normalize (&offset->dy, ctx->ctx);

    ox = ctx->paffine.xx * dx + ctx->paffine.xy * dy;
    oy = ctx->paffine.yx * dx + ctx->paffine.yy * dy;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            if (x - ox < boundarys.x0 || x - ox >= boundarys.x1)
                continue;
            if (y - oy < boundarys.y0 || y - oy >= boundarys.y1)
                continue;

            for (ch = 0; ch < 4; ch++) {
                output_pixels[y * rowstride + x * 4 + ch] =
                    in_pixels[(y - oy) * rowstride + (x - ox) * 4 + ch];
            }
        }

    cairo_surface_mark_dirty (output);

    out.surface = output;
    out.bounds = boundarys;

    rsvg_filter_store_output (primitive->result, out, ctx);

    cairo_surface_destroy  (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_offset_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag * atts)
{
    RsvgFilterPrimitiveOffset *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "dx")))
        filter->dx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "dy")))
        filter->dy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
}

RsvgNode *
rsvg_new_filter_primitive_offset (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveOffset *filter;

    filter = g_new0 (RsvgFilterPrimitiveOffset, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->dx = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    filter->dy = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    filter->super.render = rsvg_filter_primitive_offset_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_OFFSET,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_offset_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveMerge RsvgFilterPrimitiveMerge;

struct _RsvgFilterPrimitiveMerge {
    RsvgFilterPrimitive super;
};

struct merge_render_closure {
    cairo_surface_t *output;
    RsvgIRect boundarys;
    RsvgFilterContext *ctx;
};

static gboolean
merge_render_child (RsvgNode *node, gpointer data)
{
    struct merge_render_closure *closure = data;
    RsvgFilterPrimitive *fp;
    cairo_surface_t *in;

    if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE)
        return TRUE;

    fp = rsvg_rust_cnode_get_impl (node);

    in = rsvg_filter_get_in (fp->in, closure->ctx);
    if (in == NULL)
        return TRUE;

    rsvg_alpha_blt (in,
                    closure->boundarys.x0,
                    closure->boundarys.y0,
                    closure->boundarys.x1 - closure->boundarys.x0,
                    closure->boundarys.y1 - closure->boundarys.y0,
                    closure->output,
                    closure->boundarys.x0,
                    closure->boundarys.y0);

    cairo_surface_destroy (in);

    return TRUE;
}

static void
rsvg_filter_primitive_merge_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    struct merge_render_closure closure;

    closure.boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    closure.output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (closure.output == NULL) {
        return;
    }

    closure.ctx = ctx;

    rsvg_node_foreach_child (node, merge_render_child, &closure);

    rsvg_filter_store_result (primitive->result, closure.output, ctx);

    cairo_surface_destroy (closure.output);
}

static void
rsvg_filter_primitive_merge_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveMerge *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
}

RsvgNode *
rsvg_new_filter_primitive_merge (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveMerge *filter;

    filter = g_new0 (RsvgFilterPrimitiveMerge, 1);
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_merge_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_merge_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

static void
rsvg_filter_primitive_merge_node_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitive *primitive = impl;
    const char *value;

    /* see bug 145149 - sodipodi generates bad SVG... */
    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (primitive->in, value);
}

static void
rsvg_filter_primitive_merge_node_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    /* todo */
}

RsvgNode *
rsvg_new_filter_primitive_merge_node (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitive *filter;

    filter = g_new0 (RsvgFilterPrimitive, 1);
    filter->in = g_string_new ("none");
    filter->render = rsvg_filter_primitive_merge_node_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_merge_node_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveColorMatrix RsvgFilterPrimitiveColorMatrix;

struct _RsvgFilterPrimitiveColorMatrix {
    RsvgFilterPrimitive super;
    gint *KernelMatrix;
};

static void
rsvg_filter_primitive_color_matrix_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
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

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            int umch;
            int alpha = in_pixels[4 * x + y * rowstride + ctx->channelmap[3]];
            if (!alpha)
                for (umch = 0; umch < 4; umch++) {
                    sum = color_matrix->KernelMatrix[umch * 5 + 4];
                    if (sum > 255)
                        sum = 255;
                    if (sum < 0)
                        sum = 0;
                    output_pixels[4 * x + y * rowstride + ctx->channelmap[umch]] = sum;
            } else
                for (umch = 0; umch < 4; umch++) {
                    int umi;
                    ch = ctx->channelmap[umch];
                    sum = 0;
                    for (umi = 0; umi < 4; umi++) {
                        i = ctx->channelmap[umi];
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
                ch = ctx->channelmap[umch];
                output_pixels[4 * x + y * rowstride + ch] =
                    output_pixels[4 * x + y * rowstride + ch] *
                    output_pixels[4 * x + y * rowstride + ctx->channelmap[3]] / 255;
            }
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

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
rsvg_filter_primitive_color_matrix_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveColorMatrix *filter = impl;
    gint type;
    gsize listlen = 0;
    const char *value;

    type = 0;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "values"))) {
        unsigned int i;
        double *temp;
        if (!rsvg_css_parse_number_list (value,
                                         NUMBER_LIST_LENGTH_MAXIMUM,
                                         20,
                                         &temp,
                                         &listlen)) {
            rsvg_node_set_attribute_parse_error (node, "values", "invalid number list");
            return;
        }

        filter->KernelMatrix = g_new0 (int, listlen);
        for (i = 0; i < listlen; i++)
            filter->KernelMatrix[i] = temp[i] * 255.;
        g_free (temp);
    }
    if ((value = rsvg_property_bag_lookup (atts, "type"))) {
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
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_color_matrix_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_color_matrix_free);                                
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgNodeComponentTransferFunc RsvgNodeComponentTransferFunc;

typedef gint (*ComponentTransferFunc) (gint C, RsvgNodeComponentTransferFunc * user_data);

typedef struct _RsvgFilterPrimitiveComponentTransfer
 RsvgFilterPrimitiveComponentTransfer;

struct _RsvgNodeComponentTransferFunc {
    ComponentTransferFunc function;
    gint *tableValues;
    gsize nbTableValues;
    gint slope;
    gint intercept;
    gint amplitude;
    gint offset;
    gdouble exponent;
    char channel;
};

struct _RsvgFilterPrimitiveComponentTransfer {
    RsvgFilterPrimitive super;
};

static gint
identity_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    return C;
}

static gint
table_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    guint k;
    gint vk, vk1, distancefromlast;
    guint num_values;

    if (!user_data->nbTableValues)
        return C;

    num_values = user_data->nbTableValues;

    k = (C * (num_values - 1)) / 255;

    vk = user_data->tableValues[MIN (k, num_values - 1)];
    vk1 = user_data->tableValues[MIN (k + 1, num_values - 1)];

    distancefromlast = (C * (user_data->nbTableValues - 1)) - k * 255;

    return vk + distancefromlast * (vk1 - vk) / 255;
}

static gint
discrete_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    gint k;

    if (!user_data->nbTableValues)
        return C;

    k = (C * user_data->nbTableValues) / 255;

    return user_data->tableValues[CLAMP (k, 0, user_data->nbTableValues - 1)];
}

static gint
linear_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    return (user_data->slope * C) / 255 + user_data->intercept;
}

static gint
fixpow (gint base, gint exp)
{
    int out = 255;
    for (; exp > 0; exp--)
        out = out * base / 255;
    return out;
}

static gint
gamma_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    if (floor (user_data->exponent) == user_data->exponent)
        return user_data->amplitude * fixpow (C, user_data->exponent) / 255 + user_data->offset;
    else
        return (double) user_data->amplitude * pow ((double) C / 255.,
                                                    user_data->exponent) + user_data->offset;
}

struct component_transfer_closure {
    int channel_num;
    char channel;
    gboolean set_func;
    RsvgNodeComponentTransferFunc *channels[4];
    ComponentTransferFunc functions[4];
    RsvgFilterContext *ctx;
};

static gboolean
component_transfer_render_child (RsvgNode *node, gpointer data)
{
    struct component_transfer_closure *closure = data;
    RsvgNodeComponentTransferFunc *f;

    if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION)
        return TRUE;

    f = rsvg_rust_cnode_get_impl (node);

    if (f->channel == closure->channel) {
        closure->functions[closure->ctx->channelmap[closure->channel_num]] = f->function;
        closure->channels[closure->ctx->channelmap[closure->channel_num]] = f;
        closure->set_func = TRUE;
    }

    return TRUE;
}

static void
rsvg_filter_primitive_component_transfer_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    gint x, y, c;
    gint rowstride, height, width;
    RsvgIRect boundarys;
    guchar *inpix, outpix[4];
    gint achan = ctx->channelmap[3];
    guchar *in_pixels;
    guchar *output_pixels;
    cairo_surface_t *output, *in;
    struct component_transfer_closure closure;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    closure.ctx = ctx;

    for (c = 0; c < 4; c++) {
        closure.channel_num = c;
        closure.channel = "rgba"[c]; /* see rsvg_new_node_component_transfer_function() for where these chars come from */
        closure.set_func = FALSE;

        rsvg_node_foreach_child (node, component_transfer_render_child, &closure);

        if (!closure.set_func)
            closure.functions[ctx->channelmap[c]] = identity_component_transfer_func;
    }

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

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            inpix = in_pixels + (y * rowstride + x * 4);
            for (c = 0; c < 4; c++) {
                gint temp;
                int inval;
                if (c != achan) {
                    if (inpix[achan] == 0)
                        inval = 0;
                    else
                        inval = inpix[c] * 255 / inpix[achan];
                } else
                    inval = inpix[c];

                temp = closure.functions[c] (inval, closure.channels[c]);
                if (temp > 255)
                    temp = 255;
                else if (temp < 0)
                    temp = 0;
                outpix[c] = temp;
            }
            for (c = 0; c < 3; c++)
                output_pixels[y * rowstride + x * 4 + ctx->channelmap[c]] =
                    outpix[ctx->channelmap[c]] * outpix[achan] / 255;
            output_pixels[y * rowstride + x * 4 + achan] = outpix[achan];
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_component_transfer_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveComponentTransfer *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);
    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
}

RsvgNode *
rsvg_new_filter_primitive_component_transfer (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveComponentTransfer *filter;

    filter = g_new0 (RsvgFilterPrimitiveComponentTransfer, 1);
    filter->super.result = g_string_new ("none");
    filter->super.in = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_component_transfer_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPONENT_TRANSFER,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_component_transfer_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);                                
}

static void
rsvg_node_component_transfer_function_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeComponentTransferFunc *data = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "type"))) {
        if (!strcmp (value, "identity"))
            data->function = identity_component_transfer_func;
        else if (!strcmp (value, "table"))
            data->function = table_component_transfer_func;
        else if (!strcmp (value, "discrete"))
            data->function = discrete_component_transfer_func;
        else if (!strcmp (value, "linear"))
            data->function = linear_component_transfer_func;
        else if (!strcmp (value, "gamma"))
            data->function = gamma_component_transfer_func;
    }

    if ((value = rsvg_property_bag_lookup (atts, "tableValues"))) {
        unsigned int i;
        double *temp;
        if (!rsvg_css_parse_number_list (value,
                                         NUMBER_LIST_LENGTH_MAXIMUM,
                                         256,
                                         &temp,
                                         &data->nbTableValues)) {
            rsvg_node_set_attribute_parse_error (node, "tableValues", "invalid number list");
            return;
        }

        data->tableValues = g_new0 (gint, data->nbTableValues);
        for (i = 0; i < data->nbTableValues; i++)
            data->tableValues[i] = temp[i] * 255.;
        g_free (temp);
    }
    if ((value = rsvg_property_bag_lookup (atts, "slope"))) {
        data->slope = g_ascii_strtod (value, NULL) * 255.;
    }
    if ((value = rsvg_property_bag_lookup (atts, "intercept"))) {
        data->intercept = g_ascii_strtod (value, NULL) * 255.;
    }
    if ((value = rsvg_property_bag_lookup (atts, "amplitude"))) {
        data->amplitude = g_ascii_strtod (value, NULL) * 255.;
    }
    if ((value = rsvg_property_bag_lookup (atts, "exponent"))) {
        data->exponent = g_ascii_strtod (value, NULL);
    }
    if ((value = rsvg_property_bag_lookup (atts, "offset"))) {
        data->offset = g_ascii_strtod (value, NULL) * 255.;
    }
}

static void
rsvg_node_component_transfer_function_free (gpointer impl)
{
    RsvgNodeComponentTransferFunc *filter = impl;

    if (filter->nbTableValues)
        g_free (filter->tableValues);

    g_free (filter);
}

RsvgNode *
rsvg_new_node_component_transfer_function (const char *element_name, RsvgNode *parent)
{
    RsvgNodeComponentTransferFunc *filter;

    char channel;

    if (strcmp (element_name, "feFuncR") == 0)
        channel = 'r';
    else if (strcmp (element_name, "feFuncG") == 0)
        channel = 'g';
    else if (strcmp (element_name, "feFuncB") == 0)
        channel = 'b';
    else if (strcmp (element_name, "feFuncA") == 0)
        channel = 'a';
    else {
        g_assert_not_reached ();
        channel = '\0';
    }

    filter = g_new0 (RsvgNodeComponentTransferFunc, 1);
    filter->function = identity_component_transfer_func;
    filter->nbTableValues = 0;
    filter->channel = channel;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_node_component_transfer_function_set_atts,
                                rsvg_filter_draw,
                                rsvg_node_component_transfer_function_free);
}

/*************************************************************/
/*************************************************************/

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
rsvg_filter_primitive_erode_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveErode *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "radius"))) {
        if (!rsvg_css_parse_number_optional_number (value, &filter->rx, &filter->ry)) {
            rsvg_node_set_attribute_parse_error (node, "radius", "expected number-optional-number");
            return;
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "operator"))) {
        if (!strcmp (value, "erode"))
            filter->mode = 0;
        else if (!strcmp (value, "dilate"))
            filter->mode = 1;
    }
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
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_erode_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

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
rsvg_filter_primitive_composite_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveComposite *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "operator"))) {
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
    }
    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "in2")))
        g_string_assign (filter->in2, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "k1")))
        filter->k1 = g_ascii_strtod (value, NULL) * 255.;
    if ((value = rsvg_property_bag_lookup (atts, "k2")))
        filter->k2 = g_ascii_strtod (value, NULL) * 255.;
    if ((value = rsvg_property_bag_lookup (atts, "k3")))
        filter->k3 = g_ascii_strtod (value, NULL) * 255.;
    if ((value = rsvg_property_bag_lookup (atts, "k4")))
        filter->k4 = g_ascii_strtod (value, NULL) * 255.;
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

/*************************************************************/
/*************************************************************/

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

    guint32 color = state->flood_color;
    guint8 opacity = state->flood_opacity;

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
rsvg_filter_primitive_flood_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitive *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
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

/*************************************************************/
/*************************************************************/

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

    xch = ctx->channelmap[xch];
    ych = ctx->channelmap[ych];
    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            if (xch != 4)
                ox = x + displacement_map->scale * ctx->paffine.xx *
                    ((double) in2_pixels[y * rowstride + x * 4 + xch] / 255.0 - 0.5);
            else
                ox = x;

            if (ych != 4)
                oy = y + displacement_map->scale * ctx->paffine.yy *
                    ((double) in2_pixels[y * rowstride + x * 4 + ych] / 255.0 - 0.5);
            else
                oy = y;

            for (ch = 0; ch < 4; ch++) {
                output_pixels[y * rowstride + x * 4 + ch] =
                    get_interp_pixel (in_pixels, ox, oy, ch, boundarys, rowstride);
            }
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

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
rsvg_filter_primitive_displacement_map_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveDisplacementMap *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "in2")))
        g_string_assign (filter->in2, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "xChannelSelector")))
        filter->xChannelSelector = (value)[0];
    if ((value = rsvg_property_bag_lookup (atts, "yChannelSelector")))
        filter->yChannelSelector = (value)[0];
    if ((value = rsvg_property_bag_lookup (atts, "scale")))
        filter->scale = g_ascii_strtod (value, NULL);
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
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_displacement_map_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_displacement_map_free);
}

/*************************************************************/
/*************************************************************/

/* Produces results in the range [1, 2**31 - 2].
   Algorithm is: r = (a * r) mod m
   where a = 16807 and m = 2**31 - 1 = 2147483647
   See [Park & Miller], CACM vol. 31 no. 10 p. 1195, Oct. 1988
   To test: the algorithm should produce the result 1043618065
   as the 10,000th generated number if the original seed is 1.
*/
#define feTurbulence_RAND_m 2147483647  /* 2**31 - 1 */
#define feTurbulence_RAND_a 16807       /* 7**5; primitive root of m */
#define feTurbulence_RAND_q 127773      /* m / a */
#define feTurbulence_RAND_r 2836        /* m % a */
#define feTurbulence_BSize 0x100
#define feTurbulence_BM 0xff
#define feTurbulence_PerlinN 0x1000
#define feTurbulence_NP 12      /* 2^PerlinN */
#define feTurbulence_NM 0xfff

typedef struct _RsvgFilterPrimitiveTurbulence RsvgFilterPrimitiveTurbulence;
struct _RsvgFilterPrimitiveTurbulence {
    RsvgFilterPrimitive super;

    int uLatticeSelector[feTurbulence_BSize + feTurbulence_BSize + 2];
    double fGradient[4][feTurbulence_BSize + feTurbulence_BSize + 2][2];

    int seed;

    double fBaseFreqX;
    double fBaseFreqY;

    int nNumOctaves;
    gboolean bFractalSum;
    gboolean bDoStitching;
};

struct feTurbulence_StitchInfo {
    int nWidth;                 /* How much to subtract to wrap for stitching. */
    int nHeight;
    int nWrapX;                 /* Minimum value to wrap. */
    int nWrapY;
};

static long
feTurbulence_setup_seed (int lSeed)
{
    if (lSeed <= 0)
        lSeed = -(lSeed % (feTurbulence_RAND_m - 1)) + 1;
    if (lSeed > feTurbulence_RAND_m - 1)
        lSeed = feTurbulence_RAND_m - 1;
    return lSeed;
}

static long
feTurbulence_random (int lSeed)
{
    long result;

    result =
        feTurbulence_RAND_a * (lSeed % feTurbulence_RAND_q) -
        feTurbulence_RAND_r * (lSeed / feTurbulence_RAND_q);
    if (result <= 0)
        result += feTurbulence_RAND_m;
    return result;
}

static void
feTurbulence_init (RsvgFilterPrimitiveTurbulence * filter)
{
    double s;
    int i, j, k, lSeed;

    lSeed = feTurbulence_setup_seed (filter->seed);
    for (k = 0; k < 4; k++) {
        for (i = 0; i < feTurbulence_BSize; i++) {
            filter->uLatticeSelector[i] = i;
            for (j = 0; j < 2; j++)
                filter->fGradient[k][i][j] =
                    (double) (((lSeed =
                                feTurbulence_random (lSeed)) % (feTurbulence_BSize +
                                                                feTurbulence_BSize)) -
                              feTurbulence_BSize) / feTurbulence_BSize;
            s = (double) (sqrt
                          (filter->fGradient[k][i][0] * filter->fGradient[k][i][0] +
                           filter->fGradient[k][i][1] * filter->fGradient[k][i][1]));
            filter->fGradient[k][i][0] /= s;
            filter->fGradient[k][i][1] /= s;
        }
    }

    while (--i) {
        k = filter->uLatticeSelector[i];
        filter->uLatticeSelector[i] = filter->uLatticeSelector[j =
                                                               (lSeed =
                                                                feTurbulence_random (lSeed)) %
                                                               feTurbulence_BSize];
        filter->uLatticeSelector[j] = k;
    }

    for (i = 0; i < feTurbulence_BSize + 2; i++) {
        filter->uLatticeSelector[feTurbulence_BSize + i] = filter->uLatticeSelector[i];
        for (k = 0; k < 4; k++)
            for (j = 0; j < 2; j++)
                filter->fGradient[k][feTurbulence_BSize + i][j] = filter->fGradient[k][i][j];
    }
}

#define feTurbulence_s_curve(t) ( t * t * (3. - 2. * t) )
#define feTurbulence_lerp(t, a, b) ( a + t * (b - a) )

static double
feTurbulence_noise2 (RsvgFilterPrimitiveTurbulence * filter,
                     int nColorChannel, double vec[2], struct feTurbulence_StitchInfo *pStitchInfo)
{
    int bx0, bx1, by0, by1, b00, b10, b01, b11;
    double rx0, rx1, ry0, ry1, *q, sx, sy, a, b, t, u, v;
    register int i, j;

    t = vec[0] + feTurbulence_PerlinN;
    bx0 = (int) t;
    bx1 = bx0 + 1;
    rx0 = t - (int) t;
    rx1 = rx0 - 1.0f;
    t = vec[1] + feTurbulence_PerlinN;
    by0 = (int) t;
    by1 = by0 + 1;
    ry0 = t - (int) t;
    ry1 = ry0 - 1.0f;

    /* If stitching, adjust lattice points accordingly. */
    if (pStitchInfo != NULL) {
        if (bx0 >= pStitchInfo->nWrapX)
            bx0 -= pStitchInfo->nWidth;
        if (bx1 >= pStitchInfo->nWrapX)
            bx1 -= pStitchInfo->nWidth;
        if (by0 >= pStitchInfo->nWrapY)
            by0 -= pStitchInfo->nHeight;
        if (by1 >= pStitchInfo->nWrapY)
            by1 -= pStitchInfo->nHeight;
    }

    bx0 &= feTurbulence_BM;
    bx1 &= feTurbulence_BM;
    by0 &= feTurbulence_BM;
    by1 &= feTurbulence_BM;
    i = filter->uLatticeSelector[bx0];
    j = filter->uLatticeSelector[bx1];
    b00 = filter->uLatticeSelector[i + by0];
    b10 = filter->uLatticeSelector[j + by0];
    b01 = filter->uLatticeSelector[i + by1];
    b11 = filter->uLatticeSelector[j + by1];
    sx = (double) (feTurbulence_s_curve (rx0));
    sy = (double) (feTurbulence_s_curve (ry0));
    q = filter->fGradient[nColorChannel][b00];
    u = rx0 * q[0] + ry0 * q[1];
    q = filter->fGradient[nColorChannel][b10];
    v = rx1 * q[0] + ry0 * q[1];
    a = feTurbulence_lerp (sx, u, v);
    q = filter->fGradient[nColorChannel][b01];
    u = rx0 * q[0] + ry1 * q[1];
    q = filter->fGradient[nColorChannel][b11];
    v = rx1 * q[0] + ry1 * q[1];
    b = feTurbulence_lerp (sx, u, v);

    return feTurbulence_lerp (sy, a, b);
}

static double
feTurbulence_turbulence (RsvgFilterPrimitiveTurbulence * filter,
                         int nColorChannel, double *point,
                         double fTileX, double fTileY, double fTileWidth, double fTileHeight)
{
    struct feTurbulence_StitchInfo stitch;
    struct feTurbulence_StitchInfo *pStitchInfo = NULL; /* Not stitching when NULL. */

    double fSum = 0.0f, vec[2], ratio = 1.;
    int nOctave;

    /* Adjust the base frequencies if necessary for stitching. */
    if (filter->bDoStitching) {
        /* When stitching tiled turbulence, the frequencies must be adjusted
           so that the tile borders will be continuous. */
        if (filter->fBaseFreqX != 0.0) {
            double fLoFreq = (double) (floor (fTileWidth * filter->fBaseFreqX)) / fTileWidth;
            double fHiFreq = (double) (ceil (fTileWidth * filter->fBaseFreqX)) / fTileWidth;
            if (filter->fBaseFreqX / fLoFreq < fHiFreq / filter->fBaseFreqX)
                filter->fBaseFreqX = fLoFreq;
            else
                filter->fBaseFreqX = fHiFreq;
        }

        if (filter->fBaseFreqY != 0.0) {
            double fLoFreq = (double) (floor (fTileHeight * filter->fBaseFreqY)) / fTileHeight;
            double fHiFreq = (double) (ceil (fTileHeight * filter->fBaseFreqY)) / fTileHeight;
            if (filter->fBaseFreqY / fLoFreq < fHiFreq / filter->fBaseFreqY)
                filter->fBaseFreqY = fLoFreq;
            else
                filter->fBaseFreqY = fHiFreq;
        }

        /* Set up initial stitch values. */
        pStitchInfo = &stitch;
        stitch.nWidth = (int) (fTileWidth * filter->fBaseFreqX + 0.5f);
        stitch.nWrapX = fTileX * filter->fBaseFreqX + feTurbulence_PerlinN + stitch.nWidth;
        stitch.nHeight = (int) (fTileHeight * filter->fBaseFreqY + 0.5f);
        stitch.nWrapY = fTileY * filter->fBaseFreqY + feTurbulence_PerlinN + stitch.nHeight;
    }

    vec[0] = point[0] * filter->fBaseFreqX;
    vec[1] = point[1] * filter->fBaseFreqY;

    for (nOctave = 0; nOctave < filter->nNumOctaves; nOctave++) {
        if (filter->bFractalSum)
            fSum +=
                (double) (feTurbulence_noise2 (filter, nColorChannel, vec, pStitchInfo) / ratio);
        else
            fSum +=
                (double) (fabs (feTurbulence_noise2 (filter, nColorChannel, vec, pStitchInfo)) /
                          ratio);

        vec[0] *= 2;
        vec[1] *= 2;
        ratio *= 2;

        if (pStitchInfo != NULL) {
            /* Update stitch values. Subtracting PerlinN before the multiplication and
               adding it afterward simplifies to subtracting it once. */
            stitch.nWidth *= 2;
            stitch.nWrapX = 2 * stitch.nWrapX - feTurbulence_PerlinN;
            stitch.nHeight *= 2;
            stitch.nWrapY = 2 * stitch.nWrapY - feTurbulence_PerlinN;
        }
    }

    return fSum;
}

static void
rsvg_filter_primitive_turbulence_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveTurbulence *turbulence = (RsvgFilterPrimitiveTurbulence *) primitive;

    gint x, y, tileWidth, tileHeight, rowstride, width, height;
    RsvgIRect boundarys;
    guchar *output_pixels;
    cairo_surface_t *output, *in;
    cairo_matrix_t affine;

    affine = ctx->paffine;
    if (cairo_matrix_invert (&affine) != CAIRO_STATUS_SUCCESS)
      return;

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);
    rowstride = cairo_image_surface_get_stride (in);

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    tileWidth = (boundarys.x1 - boundarys.x0);
    tileHeight = (boundarys.y1 - boundarys.y0);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = 0; y < tileHeight; y++) {
        for (x = 0; x < tileWidth; x++) {
            gint i;
            double point[2];
            guchar *pixel;
            point[0] = affine.xx * (x + boundarys.x0) + affine.xy * (y + boundarys.y0) + affine.x0;
            point[1] = affine.yx * (x + boundarys.x0) + affine.yy * (y + boundarys.y0) + affine.y0;

            pixel = output_pixels + 4 * (x + boundarys.x0) + (y + boundarys.y0) * rowstride;

            for (i = 0; i < 4; i++) {
                double cr;

                cr = feTurbulence_turbulence (turbulence, i, point, (double) x, (double) y,
                                              (double) tileWidth, (double) tileHeight);

                if (turbulence->bFractalSum)
                    cr = ((cr * 255.) + 255.) / 2.;
                else
                    cr = (cr * 255.);

                cr = CLAMP (cr, 0., 255.);

                pixel[ctx->channelmap[i]] = (guchar) cr;
            }
            for (i = 0; i < 3; i++)
                pixel[ctx->channelmap[i]] =
                    pixel[ctx->channelmap[i]] * pixel[ctx->channelmap[3]] / 255;

        }
    }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_turbulence_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveTurbulence *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "baseFrequency"))) {
        if (!rsvg_css_parse_number_optional_number (value, &filter->fBaseFreqX, &filter->fBaseFreqY)) {
            rsvg_node_set_attribute_parse_error (node, "baseFrequency", "expected number-optional-number");
            return;
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "numOctaves")))
        filter->nNumOctaves = atoi (value);
    if ((value = rsvg_property_bag_lookup (atts, "seed")))
        filter->seed = atoi (value);
    if ((value = rsvg_property_bag_lookup (atts, "stitchTiles")))
        filter->bDoStitching = (!strcmp (value, "stitch"));
    if ((value = rsvg_property_bag_lookup (atts, "type")))
        filter->bFractalSum = (!strcmp (value, "fractalNoise"));
}

RsvgNode *
rsvg_new_filter_primitive_turbulence (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveTurbulence *filter;

    filter = g_new0 (RsvgFilterPrimitiveTurbulence, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->fBaseFreqX = 0;
    filter->fBaseFreqY = 0;
    filter->nNumOctaves = 1;
    filter->seed = 0;
    filter->bDoStitching = 0;
    filter->bFractalSum = 0;

    feTurbulence_init (filter);

    filter->super.render = rsvg_filter_primitive_turbulence_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_TURBULENCE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_turbulence_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveImage RsvgFilterPrimitiveImage;

struct _RsvgFilterPrimitiveImage {
    RsvgFilterPrimitive super;
    RsvgHandle *ctx;
    GString *href;
};

static cairo_surface_t *
rsvg_filter_primitive_image_render_in (RsvgFilterPrimitiveImage *image, RsvgFilterContext * context)
{
    RsvgDrawingCtx *ctx;
    RsvgNode *drawable;
    cairo_surface_t *result;

    ctx = context->ctx;

    if (!image->href)
        return NULL;

    drawable = rsvg_drawing_ctx_acquire_node (ctx, image->href->str);
    if (!drawable)
        return NULL;

    rsvg_current_state (ctx)->affine = context->paffine;

    result = rsvg_get_surface_of_node (ctx, drawable, context->width, context->height);

    rsvg_drawing_ctx_release_node (ctx, drawable);

    return result;
}

static cairo_surface_t *
rsvg_filter_primitive_image_render_ext (RsvgFilterPrimitive *self, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveImage *image = (RsvgFilterPrimitiveImage *) self;
    RsvgIRect boundarys;
    cairo_surface_t *img, *intermediate;
    int i;
    unsigned char *pixels;
    int channelmap[4];
    int length;
    int width, height;

    if (!image->href)
        return NULL;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    width = boundarys.x1 - boundarys.x0;
    height = boundarys.y1 - boundarys.y0;
    if (width == 0 || height == 0)
        return NULL;

    img = rsvg_cairo_surface_new_from_href (image->ctx,
                                            image->href->str,
                                            NULL);
    if (!img)
        return NULL;

    intermediate = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (intermediate) != CAIRO_STATUS_SUCCESS ||
        !rsvg_art_affine_image (img, intermediate,
                                &ctx->paffine,
                                (gdouble) width / ctx->paffine.xx,
                                (gdouble) height / ctx->paffine.yy)) {
        cairo_surface_destroy (intermediate);
        cairo_surface_destroy (img);
        return NULL;
    }

    cairo_surface_destroy (img);

    length = cairo_image_surface_get_height (intermediate) *
             cairo_image_surface_get_stride (intermediate);
    for (i = 0; i < 4; i++)
        channelmap[i] = ctx->channelmap[i];
    pixels = cairo_image_surface_get_data (intermediate);
    for (i = 0; i < length; i += 4) {
        unsigned char alpha;
        unsigned char pixel[4];
        int ch;
        alpha = pixels[i + 3];

        pixel[channelmap[3]] = alpha;
        if (alpha)
            for (ch = 0; ch < 3; ch++)
                pixel[channelmap[ch]] = pixels[i + ch] * alpha / 255;
        else
            for (ch = 0; ch < 3; ch++)
                pixel[channelmap[ch]] = 0;
        for (ch = 0; ch < 4; ch++)
            pixels[i + ch] = pixel[ch];
    }

    cairo_surface_mark_dirty (intermediate);
    return intermediate;
}

static void
rsvg_filter_primitive_image_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveImage *image = (RsvgFilterPrimitiveImage *) primitive;

    RsvgIRect boundarys;
    RsvgFilterPrimitiveOutput op;
    cairo_surface_t *output, *img;

    if (!image->href)
        return;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (output == NULL)
        return;

    img = rsvg_filter_primitive_image_render_in (image, ctx);
    if (img == NULL) {
        img = rsvg_filter_primitive_image_render_ext (primitive, ctx);
    }

    if (img) {
        cairo_t *cr;

        cr = cairo_create (output);
        cairo_set_source_surface (cr, img, 0, 0);
        cairo_rectangle (cr,
                         boundarys.x0,
                         boundarys.y0,
                         boundarys.x1 - boundarys.x0,
                         boundarys.y1 - boundarys.y0);
        cairo_clip (cr);
        cairo_paint (cr);
        cairo_destroy (cr);

        cairo_surface_destroy (img);
    }

    op.surface = output;
    op.bounds = boundarys;

    rsvg_filter_store_output (primitive->result, op, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_image_free (gpointer impl)
{
    RsvgFilterPrimitiveImage *image = impl;

    if (image->href)
        g_string_free (image->href, TRUE);

    rsvg_filter_primitive_free (impl);
}

static void
rsvg_filter_primitive_image_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveImage *filter = impl;
    const char *value;

    filter->ctx = handle;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);
    if ((value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
        filter->href = g_string_new (NULL);
        g_string_assign (filter->href, value);
    }

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
}

RsvgNode *
rsvg_new_filter_primitive_image (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveImage *filter;

    filter = g_new0 (RsvgFilterPrimitiveImage, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_image_render;
    filter->href = NULL;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_IMAGE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_image_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_image_free);
}

/*************************************************************/
/*************************************************************/


typedef struct _FactorAndMatrix FactorAndMatrix;

struct _FactorAndMatrix {
    gint matrix[9];
    gdouble factor;
};

typedef struct _vector3 vector3;

struct _vector3 {
    gdouble x;
    gdouble y;
    gdouble z;
};

static gdouble
norm (vector3 A)
{
    return sqrt (A.x * A.x + A.y * A.y + A.z * A.z);
}

static gdouble
dotproduct (vector3 A, vector3 B)
{
    return A.x * B.x + A.y * B.y + A.z * B.z;
}

static vector3
normalise (vector3 A)
{
    double divisor;
    divisor = norm (A);

    A.x /= divisor;
    A.y /= divisor;
    A.z /= divisor;

    return A;
}

static FactorAndMatrix
get_light_normal_matrix_x (gint n)
{
    static const FactorAndMatrix matrix_list[] = {
        {
         {0, 0, 0,
          0, -2, 2,
          0, -1, 1},
         2.0 / 3.0},
        {
         {0, 0, 0,
          -2, 0, 2,
          -1, 0, 1},
         1.0 / 3.0},
        {
         {0, 0, 0,
          -2, 2, 0,
          -1, 1, 0},
         2.0 / 3.0},
        {
         {0, -1, 1,
          0, -2, 2,
          0, -1, 1},
         1.0 / 2.0},
        {
         {-1, 0, 1,
          -2, 0, 2,
          -1, 0, 1},
         1.0 / 4.0},
        {
         {-1, 1, 0,
          -2, 2, 0,
          -1, 1, 0},
         1.0 / 2.0},
        {
         {0, -1, 1,
          0, -2, 2,
          0, 0, 0},
         2.0 / 3.0},
        {
         {-1, 0, 1,
          -2, 0, 2,
          0, 0, 0},
         1.0 / 3.0},
        {
         {-1, 1, 0,
          -2, 2, 0,
          0, 0, 0},
         2.0 / 3.0}
    };

    return matrix_list[n];
}

static FactorAndMatrix
get_light_normal_matrix_y (gint n)
{
    static const FactorAndMatrix matrix_list[] = {
        {
         {0, 0, 0,
          0, -2, -1,
          0, 2, 1},
         2.0 / 3.0},
        {
         {0, 0, 0,
          -1, -2, -1,
          1, 2, 1},
         1.0 / 3.0},
        {
         {0, 0, 0,
          -1, -2, 0,
          1, 2, 0},
         2.0 / 3.0},
        {

         {0, -2, -1,
          0, 0, 0,
          0, 2, 1},
         1.0 / 2.0},
        {
         {-1, -2, -1,
          0, 0, 0,
          1, 2, 1},
         1.0 / 4.0},
        {
         {-1, -2, 0,
          0, 0, 0,
          1, 2, 0},
         1.0 / 2.0},
        {

         {0, -2, -1,
          0, 2, 1,
          0, 0, 0},
         2.0 / 3.0},
        {
         {0, -2, -1,
          1, 2, 1,
          0, 0, 0},
         1.0 / 3.0},
        {
         {-1, -2, 0,
          1, 2, 0,
          0, 0, 0},
         2.0 / 3.0}
    };

    return matrix_list[n];
}

static vector3
get_surface_normal (guchar * I, RsvgIRect boundarys, gint x, gint y,
                    gdouble dx, gdouble dy, gdouble rawdx, gdouble rawdy, gdouble surfaceScale,
                    gint rowstride, int chan)
{
    gint mrow, mcol;
    FactorAndMatrix fnmx, fnmy;
    gint *Kx, *Ky;
    gdouble factorx, factory;
    gdouble Nx, Ny;
    vector3 output;

    if (x + dx >= boundarys.x1 - 1)
        mcol = 2;
    else if (x - dx < boundarys.x0 + 1)
        mcol = 0;
    else
        mcol = 1;

    if (y + dy >= boundarys.y1 - 1)
        mrow = 2;
    else if (y - dy < boundarys.y0 + 1)
        mrow = 0;
    else
        mrow = 1;

    fnmx = get_light_normal_matrix_x (mrow * 3 + mcol);
    factorx = fnmx.factor / rawdx;
    Kx = fnmx.matrix;

    fnmy = get_light_normal_matrix_y (mrow * 3 + mcol);
    factory = fnmy.factor / rawdy;
    Ky = fnmy.matrix;

    Nx = -surfaceScale * factorx * ((gdouble)
                                    (Kx[0] *
                                     get_interp_pixel (I, x - dx, y - dy, chan,
                                                                  boundarys,
                                                                  rowstride) +
                                     Kx[1] * get_interp_pixel (I, x, y - dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[2] * get_interp_pixel (I, x + dx, y - dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[3] * get_interp_pixel (I, x - dx, y, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[4] * get_interp_pixel (I, x, y, chan, boundarys,
                                                                          rowstride) +
                                     Kx[5] * get_interp_pixel (I, x + dx, y, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[6] * get_interp_pixel (I, x - dx, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[7] * get_interp_pixel (I, x, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Kx[8] * get_interp_pixel (I, x + dx, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride))) / 255.0;

    Ny = -surfaceScale * factory * ((gdouble)
                                    (Ky[0] *
                                     get_interp_pixel (I, x - dx, y - dy, chan,
                                                                  boundarys,
                                                                  rowstride) +
                                     Ky[1] * get_interp_pixel (I, x, y - dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[2] * get_interp_pixel (I, x + dx, y - dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[3] * get_interp_pixel (I, x - dx, y, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[4] * get_interp_pixel (I, x, y, chan, boundarys,
                                                                          rowstride) +
                                     Ky[5] * get_interp_pixel (I, x + dx, y, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[6] * get_interp_pixel (I, x - dx, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[7] * get_interp_pixel (I, x, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride) +
                                     Ky[8] * get_interp_pixel (I, x + dx, y + dy, chan,
                                                                          boundarys,
                                                                          rowstride))) / 255.0;

    output.x = Nx;
    output.y = Ny;

    output.z = 1;
    output = normalise (output);
    return output;
}

typedef enum {
    DISTANTLIGHT, POINTLIGHT, SPOTLIGHT
} lightType;

typedef struct _RsvgNodeLightSource RsvgNodeLightSource;

struct _RsvgNodeLightSource {
    lightType type;
    gdouble azimuth;
    gdouble elevation;
    RsvgLength x, y, z, pointsAtX, pointsAtY, pointsAtZ;
    gdouble specularExponent;
    gdouble limitingconeAngle;
};

static vector3
get_light_direction (RsvgNodeLightSource * source, gdouble x1, gdouble y1, gdouble z,
                     cairo_matrix_t *affine, RsvgDrawingCtx * ctx)
{
    vector3 output;

    switch (source->type) {
    case DISTANTLIGHT:
        output.x = cos (source->azimuth) * cos (source->elevation);
        output.y = sin (source->azimuth) * cos (source->elevation);
        output.z = sin (source->elevation);
        break;
    default:
        {
            double x, y;
            x = affine->xx * x1 + affine->xy * y1 + affine->x0;
            y = affine->yx * x1 + affine->yy * y1 + affine->y0;
            output.x = rsvg_length_normalize (&source->x, ctx) - x;
            output.y = rsvg_length_normalize (&source->y, ctx) - y;
            output.z = rsvg_length_normalize (&source->z, ctx) - z;
            output = normalise (output);
        }
        break;
    }
    return output;
}

static vector3
get_light_color (RsvgNodeLightSource * source, vector3 color,
                 gdouble x1, gdouble y1, gdouble z, cairo_matrix_t *affine, RsvgDrawingCtx * ctx)
{
    double base, angle, x, y;
    vector3 s;
    vector3 L;
    vector3 output;
    double sx, sy, sz, spx, spy, spz;

    if (source->type != SPOTLIGHT)
        return color;

    sx = rsvg_length_normalize (&source->x, ctx);
    sy = rsvg_length_normalize (&source->y, ctx);
    sz = rsvg_length_normalize (&source->z, ctx);
    spx = rsvg_length_normalize (&source->pointsAtX, ctx);
    spy = rsvg_length_normalize (&source->pointsAtY, ctx);
    spz = rsvg_length_normalize (&source->pointsAtZ, ctx);

    x = affine->xx * x1 + affine->xy * y1 + affine->x0;
    y = affine->yx * x1 + affine->yy * y1 + affine->y0;

    L.x = sx - x;
    L.y = sy - y;
    L.z = sz - z;
    L = normalise (L);

    s.x = spx - sx;
    s.y = spy - sy;
    s.z = spz - sz;
    s = normalise (s);

    base = -dotproduct (L, s);

    angle = acos (base);

    if (base < 0 || angle > source->limitingconeAngle) {
        output.x = 0;
        output.y = 0;
        output.z = 0;
        return output;
    }

    output.x = color.x * pow (base, source->specularExponent);
    output.y = color.y * pow (base, source->specularExponent);
    output.z = color.z * pow (base, source->specularExponent);

    return output;
}


static void
rsvg_node_light_source_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeLightSource *data = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "azimuth")))
        data->azimuth = g_ascii_strtod (value, NULL) / 180.0 * M_PI;
    if ((value = rsvg_property_bag_lookup (atts, "elevation")))
        data->elevation = g_ascii_strtod (value, NULL) / 180.0 * M_PI;
    if ((value = rsvg_property_bag_lookup (atts, "limitingConeAngle")))
        data->limitingconeAngle = g_ascii_strtod (value, NULL) / 180.0 * M_PI;
    if ((value = rsvg_property_bag_lookup (atts, "x")))
        data->x = data->pointsAtX = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        data->y = data->pointsAtX = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "z")))
        data->z = data->pointsAtX = rsvg_length_parse (value, LENGTH_DIR_BOTH);
    if ((value = rsvg_property_bag_lookup (atts, "pointsAtX")))
        data->pointsAtX = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "pointsAtY")))
        data->pointsAtY = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "pointsAtZ")))
        data->pointsAtZ = rsvg_length_parse (value, LENGTH_DIR_BOTH);
    if ((value = rsvg_property_bag_lookup (atts, "specularExponent")))
        data->specularExponent = g_ascii_strtod (value, NULL);
}

RsvgNode *
rsvg_new_node_light_source (const char *element_name, RsvgNode *parent)
{
    RsvgNodeLightSource *data;

    data = g_new0 (RsvgNodeLightSource, 1);

    data->specularExponent = 1;

    if (strcmp (element_name, "feDistantLight") == 0)
        data->type = SPOTLIGHT;
    else if (strcmp (element_name, "feSpotLight") == 0)
        data->type = DISTANTLIGHT;
    else if (strcmp (element_name, "fePointLight") == 0)
        data->type = POINTLIGHT;
    else
        g_assert_not_reached ();

    data->limitingconeAngle = 180;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_LIGHT_SOURCE,
                                parent,
                                rsvg_state_new (),
                                data,
                                rsvg_node_light_source_set_atts,
                                rsvg_filter_draw,
                                g_free);                                
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveDiffuseLighting RsvgFilterPrimitiveDiffuseLighting;

struct _RsvgFilterPrimitiveDiffuseLighting {
    RsvgFilterPrimitive super;
    gdouble dx, dy;
    double diffuseConstant;
    double surfaceScale;
    guint32 lightingcolor;
};

struct find_light_source_closure {
    RsvgNode *found_node;
};

static gboolean
is_light_source (RsvgNode *node, gpointer data)
{
    struct find_light_source_closure *closure = data;

    if (rsvg_node_get_type (node) == RSVG_NODE_TYPE_LIGHT_SOURCE) {
        closure->found_node = rsvg_node_ref (node);
    }

    return TRUE;
}

static RsvgNodeLightSource *
find_light_source_in_children (RsvgNode *node)
{
    struct find_light_source_closure closure;
    RsvgNodeLightSource *source;

    closure.found_node = NULL;
    rsvg_node_foreach_child (node, is_light_source, &closure);
    if (closure.found_node == NULL)
        return NULL;

    g_assert (rsvg_node_get_type (closure.found_node) == RSVG_NODE_TYPE_LIGHT_SOURCE);

    source = rsvg_rust_cnode_get_impl (closure.found_node);
    closure.found_node = rsvg_node_unref (closure.found_node);

    return source;
}

static void
rsvg_filter_primitive_diffuse_lighting_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
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

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    source = find_light_source_in_children (node);
    if (source == NULL)
        return;

    iaffine = ctx->paffine;
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

    output_pixels = cairo_image_surface_get_data (output);

    color.x = ((guchar *) (&diffuse_lighting->lightingcolor))[2] / 255.0;
    color.y = ((guchar *) (&diffuse_lighting->lightingcolor))[1] / 255.0;
    color.z = ((guchar *) (&diffuse_lighting->lightingcolor))[0] / 255.0;

    surfaceScale = diffuse_lighting->surfaceScale / 255.0;

    if (diffuse_lighting->dy < 0 || diffuse_lighting->dx < 0) {
        dx = 1;
        dy = 1;
        rawdx = 1;
        rawdy = 1;
    } else {
        dx = diffuse_lighting->dx * ctx->paffine.xx;
        dy = diffuse_lighting->dy * ctx->paffine.yy;
        rawdx = diffuse_lighting->dx;
        rawdy = diffuse_lighting->dy;
    }

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = surfaceScale * (double) in_pixels[y * rowstride + x * 4 + ctx->channelmap[3]];
            L = get_light_direction (source, x, y, z, &iaffine, ctx->ctx);
            N = get_surface_normal (in_pixels, boundarys, x, y,
                                    dx, dy, rawdx, rawdy, diffuse_lighting->surfaceScale,
                                    rowstride, ctx->channelmap[3]);
            lightcolor = get_light_color (source, color, x, y, z, &iaffine, ctx->ctx);
            factor = dotproduct (N, L);

            output_pixels[y * rowstride + x * 4 + ctx->channelmap[0]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.x * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[1]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.y * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[2]] =
                MAX (0, MIN (255, diffuse_lighting->diffuseConstant * factor * lightcolor.z * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[3]] = 255;
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_diffuse_lighting_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveDiffuseLighting *filter = impl;
    const char *value;
    RsvgState *state;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "kernelUnitLength")))
        rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy);

    if ((value = rsvg_property_bag_lookup (atts, "lighting-color"))) {
        RsvgCssColorSpec spec;

        spec = rsvg_css_parse_color (value, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_YES);

        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            /* FIXME: we should inherit; see how stop-color is handled in rsvg-styles.c */
            break;

        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            state = rsvg_state_new ();
            rsvg_state_reconstruct (state, node);
            filter->lightingcolor = state->current_color;
            break;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            filter->lightingcolor = spec.argb;
            break;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            rsvg_node_set_attribute_parse_error (node, "lighting-color", "Invalid color");
            break;

        default:
            g_assert_not_reached ();
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "diffuseConstant")))
        filter->diffuseConstant = g_ascii_strtod (value, NULL);
    if ((value = rsvg_property_bag_lookup (atts, "surfaceScale")))
        filter->surfaceScale = g_ascii_strtod (value, NULL);
}

RsvgNode *
rsvg_new_filter_primitive_diffuse_lighting (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveDiffuseLighting *filter;

    filter = g_new0 (RsvgFilterPrimitiveDiffuseLighting, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->surfaceScale = 1;
    filter->diffuseConstant = 1;
    filter->dx = 1;
    filter->dy = 1;
    filter->lightingcolor = 0xFFFFFFFF;
    filter->super.render = rsvg_filter_primitive_diffuse_lighting_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_DIFFUSE_LIGHTING,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_diffuse_lighting_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveSpecularLighting RsvgFilterPrimitiveSpecularLighting;

struct _RsvgFilterPrimitiveSpecularLighting {
    RsvgFilterPrimitive super;
    double specularConstant;
    double specularExponent;
    double surfaceScale;
    guint32 lightingcolor;
};

static void
rsvg_filter_primitive_specular_lighting_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
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

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    source = find_light_source_in_children (node);
    if (source == NULL)
        return;

    iaffine = ctx->paffine;
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

    output_pixels = cairo_image_surface_get_data (output);

    color.x = ((guchar *) (&specular_lighting->lightingcolor))[2] / 255.0;
    color.y = ((guchar *) (&specular_lighting->lightingcolor))[1] / 255.0;
    color.z = ((guchar *) (&specular_lighting->lightingcolor))[0] / 255.0;

    surfaceScale = specular_lighting->surfaceScale / 255.0;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = in_pixels[y * rowstride + x * 4 + 3] * surfaceScale;
            L = get_light_direction (source, x, y, z, &iaffine, ctx->ctx);
            L.z += 1;
            L = normalise (L);

            lightcolor = get_light_color (source, color, x, y, z, &iaffine, ctx->ctx);
            base = dotproduct (get_surface_normal (in_pixels, boundarys, x, y,
                                                   1, 1, 1.0 / ctx->paffine.xx,
                                                   1.0 / ctx->paffine.yy, specular_lighting->surfaceScale,
                                                   rowstride, ctx->channelmap[3]), L);

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

            output_pixels[y * rowstride + x * 4 + ctx->channelmap[0]] = lightcolor.x * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[1]] = lightcolor.y * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[2]] = lightcolor.z * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[3]] = max;

        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_specular_lighting_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveSpecularLighting *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    if ((value = rsvg_property_bag_lookup (atts, "lighting-color"))) {
        RsvgCssColorSpec spec;
        RsvgState *state;

        spec = rsvg_css_parse_color (value, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_YES);

        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            /* FIXME: we should inherit; see how stop-color is handled in rsvg-styles.c */
            break;

        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            state = rsvg_state_new ();
            rsvg_state_reconstruct (state, node);
            filter->lightingcolor = state->current_color;
            break;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            filter->lightingcolor = spec.argb;
            break;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            rsvg_node_set_attribute_parse_error (node, "lighting-color", "Invalid color");
            break;

        default:
            g_assert_not_reached ();
        }
    }

    if ((value = rsvg_property_bag_lookup (atts, "specularConstant")))
        filter->specularConstant = g_ascii_strtod (value, NULL);
    if ((value = rsvg_property_bag_lookup (atts, "specularExponent")))
        filter->specularExponent = g_ascii_strtod (value, NULL);
    if ((value = rsvg_property_bag_lookup (atts, "surfaceScale")))
        filter->surfaceScale = g_ascii_strtod (value, NULL);
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
    filter->lightingcolor = 0xFFFFFFFF;
    filter->super.render = rsvg_filter_primitive_specular_lighting_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_SPECULAR_LIGHTING,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_specular_lighting_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

/*************************************************************/
/*************************************************************/

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
    boundarys = input.bounds;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
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

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_tile_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgFilterPrimitiveTile *filter = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "in")))
        g_string_assign (filter->super.in, value);
    if ((value = rsvg_property_bag_lookup (atts, "result")))
        g_string_assign (filter->super.result, value);

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);
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
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_tile_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}
