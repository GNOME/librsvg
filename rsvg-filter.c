/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
    gboolean Rused;
    gboolean Gused;
    gboolean Bused;
    gboolean Aused;
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

struct _RsvgFilterPrimitive {
    RsvgNode super;
    RsvgLength x, y, width, height;
    GString *in;
    GString *result;

    void (*render) (RsvgFilterPrimitive * self, RsvgFilterContext * ctx);
};

/*************************************************************/
/*************************************************************/

static void
rsvg_filter_primitive_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    self->render (self, ctx);
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
        _rsvg_push_view_box (ctx->ctx, 1., 1.);
    otherbox.rect.x = _rsvg_css_normalize_length (&ctx->filter->x, ctx->ctx, 'h');
    otherbox.rect.y = _rsvg_css_normalize_length (&ctx->filter->y, ctx->ctx, 'v');
    otherbox.rect.width = _rsvg_css_normalize_length (&ctx->filter->width, ctx->ctx, 'h');
    otherbox.rect.height = _rsvg_css_normalize_length (&ctx->filter->height, ctx->ctx, 'v');
    if (ctx->filter->filterunits == objectBoundingBox)
        _rsvg_pop_view_box (ctx->ctx);

    rsvg_bbox_insert (&box, &otherbox);

    if (self != NULL)
        if (self->x.factor != 'n' || self->y.factor != 'n' ||
            self->width.factor != 'n' || self->height.factor != 'n') {

            rsvg_bbox_init (&otherbox, &ctx->paffine);
            otherbox.virgin = 0;
            if (ctx->filter->primitiveunits == objectBoundingBox)
                _rsvg_push_view_box (ctx->ctx, 1., 1.);
            if (self->x.factor != 'n')
                otherbox.rect.x = _rsvg_css_normalize_length (&self->x, ctx->ctx, 'h');
            else
                otherbox.rect.x = 0;
            if (self->y.factor != 'n')
                otherbox.rect.y = _rsvg_css_normalize_length (&self->y, ctx->ctx, 'v');
            else
                otherbox.rect.y = 0;
            if (self->width.factor != 'n')
                otherbox.rect.width = _rsvg_css_normalize_length (&self->width, ctx->ctx, 'h');
            else
                otherbox.rect.width = ctx->ctx->vb.rect.width;
            if (self->height.factor != 'n')
                otherbox.rect.height = _rsvg_css_normalize_length (&self->height, ctx->ctx, 'v');
            else
                otherbox.rect.height = ctx->ctx->vb.rect.height;
            if (ctx->filter->primitiveunits == objectBoundingBox)
                _rsvg_pop_view_box (ctx->ctx);
            rsvg_bbox_clip (&box, &otherbox);
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
    gint rightx;
    gint bottomy;
    gint dstwidth;
    gint dstheight;

    gint srcoffsetx;
    gint srcoffsety;
    gint dstoffsetx;
    gint dstoffsety;

    gint x, y, srcrowstride, dstrowstride, sx, sy, dx, dy;
    guchar *src_pixels, *dst_pixels;

    cairo_surface_flush (src);

    dstheight = srcheight;
    dstwidth = srcwidth;

    rightx = srcx + srcwidth;
    bottomy = srcy + srcheight;

    if (rightx > cairo_image_surface_get_width (src))
        rightx = cairo_image_surface_get_width (src);
    if (bottomy > cairo_image_surface_get_height (src))
        bottomy = cairo_image_surface_get_height (src);
    srcwidth = rightx - srcx;
    srcheight = bottomy - srcy;

    rightx = dstx + dstwidth;
    bottomy = dsty + dstheight;
    if (rightx > cairo_image_surface_get_width (dst))
        rightx = cairo_image_surface_get_width (dst);
    if (bottomy > cairo_image_surface_get_height (dst))
        bottomy = cairo_image_surface_get_height (dst);
    dstwidth = rightx - dstx;
    dstheight = bottomy - dsty;

    if (dstwidth < srcwidth)
        srcwidth = dstwidth;
    if (dstheight < srcheight)
        srcheight = dstheight;

    if (srcx < 0)
        srcoffsetx = 0 - srcx;
    else
        srcoffsetx = 0;

    if (srcy < 0)
        srcoffsety = 0 - srcy;
    else
        srcoffsety = 0;

    if (dstx < 0)
        dstoffsetx = 0 - dstx;
    else
        dstoffsetx = 0;

    if (dsty < 0)
        dstoffsety = 0 - dsty;
    else
        dstoffsety = 0;

    if (dstoffsetx > srcoffsetx)
        srcoffsetx = dstoffsetx;
    if (dstoffsety > srcoffsety)
        srcoffsety = dstoffsety;

    srcrowstride = cairo_image_surface_get_stride (src);
    dstrowstride = cairo_image_surface_get_stride (dst);

    src_pixels = cairo_image_surface_get_data (src);
    dst_pixels = cairo_image_surface_get_data (dst);

    for (y = srcoffsety; y < srcheight; y++)
        for (x = srcoffsetx; x < srcwidth; x++) {
            guint a, c, ad, cd, ar, cr, i;

            sx = x + srcx;
            sy = y + srcy;
            dx = x + dstx;
            dy = y + dsty;
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

/**
 * rsvg_filter_render: Create a new surface applied the filter.
 * @self: a pointer to the filter to use
 * @source: the a #cairo_surface_t of type %CAIRO_SURFACE_TYPE_IMAGE
 * @context: the context
 *
 * This function will create a context for itself, set up the coordinate systems
 * execute all its little primatives and then clean up its own mess
 * 
 * Returns: (transfer full): a new #cairo_surface_t
 **/
cairo_surface_t *
rsvg_filter_render (RsvgFilter *self,
                    cairo_surface_t *source,
                    RsvgDrawingCtx *context, 
                    RsvgBbox *bounds, 
                    char *channelmap)
{
    RsvgFilterContext *ctx;
    RsvgFilterPrimitive *current;
    guint i;
    cairo_surface_t *output;

    g_return_val_if_fail (source != NULL, NULL);
    g_return_val_if_fail (cairo_surface_get_type (source) == CAIRO_SURFACE_TYPE_IMAGE, NULL);

    ctx = g_new (RsvgFilterContext, 1);
    ctx->filter = self;
    ctx->source_surface = source;
    ctx->bg_surface = NULL;
    ctx->results = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, rsvg_filter_free_pair);
    ctx->ctx = context;

    rsvg_filter_fix_coordinate_system (ctx, rsvg_current_state (context), bounds);

    ctx->lastresult.surface = cairo_surface_reference (source);
    ctx->lastresult.Rused = 1;
    ctx->lastresult.Gused = 1;
    ctx->lastresult.Bused = 1;
    ctx->lastresult.Aused = 1;
    ctx->lastresult.bounds = rsvg_filter_primitive_get_bounds (NULL, ctx);

    for (i = 0; i < 4; i++)
        ctx->channelmap[i] = channelmap[i] - '0';

    for (i = 0; i < self->super.children->len; i++) {
        current = g_ptr_array_index (self->super.children, i);
        if (RSVG_NODE_IS_FILTER_PRIMITIVE (&current->super))
            rsvg_filter_primitive_render (current, ctx);
    }

    output = ctx->lastresult.surface;

    g_hash_table_destroy (ctx->results);

    rsvg_filter_context_free (ctx);

    return output;
}

/**
 * rsvg_filter_store_result: Files a result into a context.
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

    store = g_new (RsvgFilterPrimitiveOutput, 1);
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
    output.Rused = 1;
    output.Gused = 1;
    output.Bused = 1;
    output.Aused = 1;
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
 * Returns: (transfer none): a #cairo_surface_t, or %NULL
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
 * rsvg_filter_get_in: Gets a surface for a primitive
 * @name: The name of the surface
 * @ctx: the context that this was called in
 * @
 * Returns: a pointer to the result that the name refers to, a special
 * surface if the name is a special keyword or NULL if nothing was found
 **/
static RsvgFilterPrimitiveOutput
rsvg_filter_get_result (GString * name, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveOutput output;
    RsvgFilterPrimitiveOutput *outputpointer;
    output.bounds.x0 = output.bounds.x1 = output.bounds.y0 = output.bounds.y1 = 0;

    if (!strcmp (name->str, "SourceGraphic")) {
        output.surface = cairo_surface_reference (ctx->source_surface);
        output.Rused = output.Gused = output.Bused = output.Aused = 1;
        return output;
    } else if (!strcmp (name->str, "BackgroundImage")) {
        output.surface = rsvg_filter_get_bg (ctx);
        if (output.surface)
            cairo_surface_reference (output.surface);
        output.Rused = output.Gused = output.Bused = output.Aused = 1;
        return output;
    } else if (!name || !strcmp (name->str, "") || !strcmp (name->str, "none")) {
        output = ctx->lastresult;
        cairo_surface_reference (output.surface);
        return output;
    } else if (!strcmp (name->str, "SourceAlpha")) {
        output.Rused = output.Gused = output.Bused = 0;
        output.Aused = 1;
        output.surface = surface_get_alpha (ctx->source_surface, ctx);
        return output;
    } else if (!strcmp (name->str, "BackgroundAlpha")) {
        output.Rused = output.Gused = output.Bused = 0;
        output.Aused = 1;
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
 * Returns: (transfer full): a new #cairo_surface_t, or %NULL
 */
static cairo_surface_t *
rsvg_filter_get_in (GString * name, RsvgFilterContext * ctx)
{
    return rsvg_filter_get_result (name, ctx).surface;
}

/**
 * rsvg_filter_parse: Looks up an allready created filter.
 * @defs: a pointer to the hash of definitions
 * @str: a string with the name of the filter to be looked up
 *
 * Returns: a pointer to the filter that the name refers to, or NULL
 * if none was found
 **/
RsvgFilter *
rsvg_filter_parse (const RsvgDefs * defs, const char *str)
{
    char *name;

    name = rsvg_get_url_string (str);
    if (name) {
        RsvgNode *val;
        val = rsvg_defs_lookup (defs, name);
        g_free (name);

        if (val && RSVG_NODE_TYPE (val) == RSVG_NODE_TYPE_FILTER)
            return (RsvgFilter *) val;
    }
    return NULL;
}

static void
rsvg_filter_set_args (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilter *filter;

    filter = (RsvgFilter *) self;

    if (rsvg_property_bag_size (atts)) {
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
            filter->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super);
    }
}

/**
 * rsvg_new_filter: Creates a black filter
 *
 * Creates a blank filter and assigns default values to everything
 **/
RsvgNode *
rsvg_new_filter (void)
{
    RsvgFilter *filter;

    filter = g_new (RsvgFilter, 1);
    _rsvg_node_init (&filter->super, RSVG_NODE_TYPE_FILTER);
    filter->filterunits = objectBoundingBox;
    filter->primitiveunits = userSpaceOnUse;
    filter->x = _rsvg_css_parse_length ("-10%");
    filter->y = _rsvg_css_parse_length ("-10%");
    filter->width = _rsvg_css_parse_length ("120%");
    filter->height = _rsvg_css_parse_length ("120%");
    filter->super.set_atts = rsvg_filter_set_args;
    return (RsvgNode *) filter;
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
rsvg_filter_primitive_blend_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgIRect boundarys;

    RsvgFilterPrimitiveBlend *upself;

    cairo_surface_t *output, *in, *in2;

    upself = (RsvgFilterPrimitiveBlend *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
      return;

    in2 = rsvg_filter_get_in (upself->in2, ctx);
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

    rsvg_filter_blend (upself->mode, in, in2, output, boundarys, ctx->channelmap);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_blend_free (RsvgNode * self)
{
    RsvgFilterPrimitiveBlend *upself;
    upself = (RsvgFilterPrimitiveBlend *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    g_string_free (upself->in2, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_blend_set_atts (RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveBlend *filter;

    filter = (RsvgFilterPrimitiveBlend *) node;

    if (rsvg_property_bag_size (atts)) {
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
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_blend (void)
{
    RsvgFilterPrimitiveBlend *filter;
    filter = g_new (RsvgFilterPrimitiveBlend, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_BLEND);
    filter->mode = normal;
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->super.render = &rsvg_filter_primitive_blend_render;
    filter->super.super.free = &rsvg_filter_primitive_blend_free;
    filter->super.super.set_atts = rsvg_filter_primitive_blend_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveConvolveMatrix RsvgFilterPrimitiveConvolveMatrix;

struct _RsvgFilterPrimitiveConvolveMatrix {
    RsvgFilterPrimitive super;
    double *KernelMatrix;
    double divisor;
    gint orderx, ordery;
    double dx, dy;
    double bias;
    gint targetx, targety;
    gboolean preservealpha;
    gint edgemode;
};

static void
rsvg_filter_primitive_convolve_matrix_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar ch;
    gint x, y;
    gint i, j;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveConvolveMatrix *upself;

    cairo_surface_t *output, *in;

    gint sx, sy, kx, ky;
    guchar sval;
    double kval, sum, dx, dy, targetx, targety;
    int umch;

    gint tempresult;

    upself = (RsvgFilterPrimitiveConvolveMatrix *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    targetx = upself->targetx * ctx->paffine.xx;
    targety = upself->targety * ctx->paffine.yy;

    if (upself->dx != 0 || upself->dy != 0) {
        dx = upself->dx * ctx->paffine.xx;
        dy = upself->dy * ctx->paffine.yy;
    } else
        dx = dy = 1;

    rowstride = cairo_image_surface_get_stride (in);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            for (umch = 0; umch < 3 + !upself->preservealpha; umch++) {
                ch = ctx->channelmap[umch];
                sum = 0;
                for (i = 0; i < upself->ordery; i++)
                    for (j = 0; j < upself->orderx; j++) {
                        int alpha;
                        sx = x - targetx + j * dx;
                        sy = y - targety + i * dy;
                        if (upself->edgemode == 0) {
                            if (sx < boundarys.x0)
                                sx = boundarys.x0;
                            if (sx >= boundarys.x1)
                                sx = boundarys.x1 - 1;
                            if (sy < boundarys.y0)
                                sy = boundarys.y0;
                            if (sy >= boundarys.y1)
                                sy = boundarys.y1 - 1;
                        } else if (upself->edgemode == 1) {
                            if (sx < boundarys.x0 || (sx >= boundarys.x1))
                                sx = boundarys.x0 + (sx - boundarys.x0) %
                                    (boundarys.x1 - boundarys.x0);
                            if (sy < boundarys.y0 || (sy >= boundarys.y1))
                                sy = boundarys.y0 + (sy - boundarys.y0) %
                                    (boundarys.y1 - boundarys.y0);
                        } else if (upself->edgemode == 2)
                            if (sx < boundarys.x0 || (sx >= boundarys.x1) ||
                                sy < boundarys.y0 || (sy >= boundarys.y1))
                                continue;

                        kx = upself->orderx - j - 1;
                        ky = upself->ordery - i - 1;
                        alpha = in_pixels[4 * sx + sy * rowstride + 3];
                        if (ch == 3)
                            sval = alpha;
                        else if (alpha)
                            sval = in_pixels[4 * sx + sy * rowstride + ch] * 255 / alpha;
                        else
                            sval = 0;
                        kval = upself->KernelMatrix[kx + ky * upself->orderx];
                        sum += (double) sval *kval;
                    }
                tempresult = sum / upself->divisor + upself->bias;

                if (tempresult > 255)
                    tempresult = 255;
                if (tempresult < 0)
                    tempresult = 0;

                output_pixels[4 * x + y * rowstride + ch] = tempresult;
            }
            if (upself->preservealpha)
                output_pixels[4 * x + y * rowstride + ctx->channelmap[3]] =
                    in_pixels[4 * x + y * rowstride + ctx->channelmap[3]];
            for (umch = 0; umch < 3; umch++) {
                ch = ctx->channelmap[umch];
                output_pixels[4 * x + y * rowstride + ch] =
                    output_pixels[4 * x + y * rowstride + ch] *
                    output_pixels[4 * x + y * rowstride + ctx->channelmap[3]] / 255;
            }
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_convolve_matrix_free (RsvgNode * self)
{
    RsvgFilterPrimitiveConvolveMatrix *upself;

    upself = (RsvgFilterPrimitiveConvolveMatrix *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    g_free (upself->KernelMatrix);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_convolve_matrix_set_atts (RsvgNode * self,
                                                RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    gint i, j;
    guint listlen = 0;
    const char *value;
    gboolean has_target_x, has_target_y;
    RsvgFilterPrimitiveConvolveMatrix *filter;

    filter = (RsvgFilterPrimitiveConvolveMatrix *) self;
    has_target_x = 0;
    has_target_y = 0;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
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
            rsvg_css_parse_number_optional_number (value, &tempx, &tempy);
            filter->orderx = tempx;
            filter->ordery = tempy;

        }
        if ((value = rsvg_property_bag_lookup (atts, "kernelUnitLength")))
            rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy);

        if ((value = rsvg_property_bag_lookup (atts, "kernelMatrix")))
            filter->KernelMatrix = rsvg_css_parse_number_list (value, &listlen);

        if ((value = rsvg_property_bag_lookup (atts, "edgeMode"))) {
            if (!strcmp (value, "wrap"))
                filter->edgemode = 1;
            else if (!strcmp (value, "none"))
                filter->edgemode = 2;
            else
                filter->edgemode = 0;
        }
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }

    if ((gint) listlen != filter->orderx * filter->ordery)
        filter->orderx = filter->ordery = 0;

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
rsvg_new_filter_primitive_convolve_matrix (void)
{
    RsvgFilterPrimitiveConvolveMatrix *filter;
    filter = g_new (RsvgFilterPrimitiveConvolveMatrix, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_CONVOLVE_MATRIX);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->KernelMatrix = NULL;
    filter->divisor = 0;
    filter->bias = 0;
    filter->dx = 0;
    filter->dy = 0;
    filter->preservealpha = FALSE;
    filter->edgemode = 0;
    filter->super.render = &rsvg_filter_primitive_convolve_matrix_render;
    filter->super.super.free = &rsvg_filter_primitive_convolve_matrix_free;
    filter->super.super.set_atts = rsvg_filter_primitive_convolve_matrix_set_atts;
    return (RsvgNode *) filter;
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
box_blur (cairo_surface_t *in, 
          cairo_surface_t *output, 
          guchar *intermediate, 
          gint kw,
          gint kh, 
          RsvgIRect boundarys, 
          RsvgFilterPrimitiveOutput op)
{
    gint ch;
    gint x, y;
    gint rowstride;
    guchar *in_pixels;
    guchar *output_pixels;
    gint sum;

    in_pixels = cairo_image_surface_get_data (in);
    output_pixels = cairo_image_surface_get_data (output);

    rowstride = cairo_image_surface_get_stride (in);

    if (kw > boundarys.x1 - boundarys.x0)
        kw = boundarys.x1 - boundarys.x0;

    if (kh > boundarys.y1 - boundarys.y0)
        kh = boundarys.y1 - boundarys.y0;


    if (kw >= 1) {
        for (ch = 0; ch < 4; ch++) {
            switch (ch) {
            case 0:
                if (!op.Rused)
                    continue;
            case 1:
                if (!op.Gused)
                    continue;
            case 2:
                if (!op.Bused)
                    continue;
            case 3:
                if (!op.Aused)
                    continue;
            }
            for (y = boundarys.y0; y < boundarys.y1; y++) {
                sum = 0;
                for (x = boundarys.x0; x < boundarys.x0 + kw; x++) {
                    sum += (intermediate[x % kw] = in_pixels[4 * x + y * rowstride + ch]);

                    if (x - kw / 2 >= 0 && x - kw / 2 < boundarys.x1)
                        output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / kw;
                }
                for (x = boundarys.x0 + kw; x < boundarys.x1; x++) {
                    sum -= intermediate[x % kw];
                    sum += (intermediate[x % kw] = in_pixels[4 * x + y * rowstride + ch]);
                    output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / kw;
                }
                for (x = boundarys.x1; x < boundarys.x1 + kw; x++) {
                    sum -= intermediate[x % kw];

                    if (x - kw / 2 >= 0 && x - kw / 2 < boundarys.x1)
                        output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / kw;
                }
            }
        }
        in_pixels = output_pixels;
    }

    if (kh >= 1) {
        for (ch = 0; ch < 4; ch++) {
            switch (ch) {
            case 0:
                if (!op.Rused)
                    continue;
            case 1:
                if (!op.Gused)
                    continue;
            case 2:
                if (!op.Bused)
                    continue;
            case 3:
                if (!op.Aused)
                    continue;
            }


            for (x = boundarys.x0; x < boundarys.x1; x++) {
                sum = 0;

                for (y = boundarys.y0; y < boundarys.y0 + kh; y++) {
                    sum += (intermediate[y % kh] = in_pixels[4 * x + y * rowstride + ch]);

                    if (y - kh / 2 >= 0 && y - kh / 2 < boundarys.y1)
                        output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / kh;
                }
                for (y = boundarys.y0 + kh; y < boundarys.y1; y++) {
                    sum -= intermediate[y % kh];
                    sum += (intermediate[y % kh] = in_pixels[4 * x + y * rowstride + ch]);
                    output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / kh;
                }
                for (y = boundarys.y1; y < boundarys.y1 + kh; y++) {
                    sum -= intermediate[y % kh];

                    if (y - kh / 2 >= 0 && y - kh / 2 < boundarys.y1)
                        output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / kh;
                }
            }
        }
    }
}

static void
fast_blur (cairo_surface_t *in, 
           cairo_surface_t *output, 
           gfloat sx,
           gfloat sy, 
           RsvgIRect boundarys, 
           RsvgFilterPrimitiveOutput op)
{
    gint kx, ky;
    guchar *intermediate;

    cairo_surface_flush (in);

    kx = floor (sx * 3 * sqrt (2 * M_PI) / 4 + 0.5);
    ky = floor (sy * 3 * sqrt (2 * M_PI) / 4 + 0.5);

    if (kx < 1 && ky < 1)
        return;

    intermediate = g_new (guchar, MAX (kx, ky));

    box_blur (in, output, intermediate, kx, ky, boundarys, op);
    box_blur (output, output, intermediate, kx, ky, boundarys, op);
    box_blur (output, output, intermediate, kx, ky, boundarys, op);

    g_free (intermediate);

    cairo_surface_mark_dirty (output);
}

static void
rsvg_filter_primitive_gaussian_blur_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveGaussianBlur *upself;
    cairo_surface_t *output, *in;
    RsvgIRect boundarys;
    gfloat sdx, sdy;
    RsvgFilterPrimitiveOutput op;

    upself = (RsvgFilterPrimitiveGaussianBlur *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    op = rsvg_filter_get_result (self->in, ctx);
    in = op.surface;

    output = _rsvg_image_surface_new (cairo_image_surface_get_width (in), 
                                      cairo_image_surface_get_height (in));
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    /* scale the SD values */
    sdx = upself->sdx * ctx->paffine.xx;
    sdy = upself->sdy * ctx->paffine.yy;

    fast_blur (in, output, sdx, sdy, boundarys, op);

    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output (self->result, op, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_gaussian_blur_free (RsvgNode * self)
{
    RsvgFilterPrimitiveGaussianBlur *upself;

    upself = (RsvgFilterPrimitiveGaussianBlur *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_gaussian_blur_set_atts (RsvgNode * self,
                                              RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveGaussianBlur *filter;

    filter = (RsvgFilterPrimitiveGaussianBlur *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "stdDeviation")))
            rsvg_css_parse_number_optional_number (value, &filter->sdx, &filter->sdy);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_gaussian_blur (void)
{
    RsvgFilterPrimitiveGaussianBlur *filter;
    filter = g_new (RsvgFilterPrimitiveGaussianBlur, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_GAUSSIAN_BLUR);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->sdx = 0;
    filter->sdy = 0;
    filter->super.render = &rsvg_filter_primitive_gaussian_blur_render;
    filter->super.super.free = &rsvg_filter_primitive_gaussian_blur_free;
    filter->super.super.set_atts = rsvg_filter_primitive_gaussian_blur_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveOffset RsvgFilterPrimitiveOffset;

struct _RsvgFilterPrimitiveOffset {
    RsvgFilterPrimitive super;
    RsvgLength dx, dy;
};

static void
rsvg_filter_primitive_offset_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar ch;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveOutput out;
    RsvgFilterPrimitiveOffset *upself;

    cairo_surface_t *output, *in;

    double dx, dy;
    int ox, oy;

    upself = (RsvgFilterPrimitiveOffset *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
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

    dx = _rsvg_css_normalize_length (&upself->dx, ctx->ctx, 'w');
    dy = _rsvg_css_normalize_length (&upself->dy, ctx->ctx, 'v');

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
    out.Rused = 1;
    out.Gused = 1;
    out.Bused = 1;
    out.Aused = 1;
    out.bounds = boundarys;

    rsvg_filter_store_output (self->result, out, ctx);

    cairo_surface_destroy  (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_offset_free (RsvgNode * self)
{
    RsvgFilterPrimitiveOffset *upself;

    upself = (RsvgFilterPrimitiveOffset *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_offset_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveOffset *filter;

    filter = (RsvgFilterPrimitiveOffset *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dx")))
            filter->dx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dy")))
            filter->dy = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_offset (void)
{
    RsvgFilterPrimitiveOffset *filter;
    filter = g_new (RsvgFilterPrimitiveOffset, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_OFFSET);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->dy = _rsvg_css_parse_length ("0");
    filter->dx = _rsvg_css_parse_length ("0");
    filter->super.render = &rsvg_filter_primitive_offset_render;
    filter->super.super.free = &rsvg_filter_primitive_offset_free;
    filter->super.super.set_atts = rsvg_filter_primitive_offset_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveMerge RsvgFilterPrimitiveMerge;

struct _RsvgFilterPrimitiveMerge {
    RsvgFilterPrimitive super;
};

static void
rsvg_filter_primitive_merge_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guint i;
    RsvgIRect boundarys;

    RsvgFilterPrimitiveMerge *upself;

    cairo_surface_t *output, *in;

    upself = (RsvgFilterPrimitiveMerge *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (output == NULL) {
        return;
    }

    for (i = 0; i < upself->super.super.children->len; i++) {
        RsvgFilterPrimitive *mn;
        mn = g_ptr_array_index (upself->super.super.children, i);
        if (RSVG_NODE_TYPE (&mn->super) != RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE)
            continue;
        in = rsvg_filter_get_in (mn->in, ctx);
        if (in == NULL)
            continue;

        rsvg_alpha_blt (in, boundarys.x0, boundarys.y0, boundarys.x1 - boundarys.x0,
                        boundarys.y1 - boundarys.y0, output, boundarys.x0, boundarys.y0);
        cairo_surface_destroy (in);
    }

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_merge_free (RsvgNode * self)
{
    RsvgFilterPrimitiveMerge *upself;

    upself = (RsvgFilterPrimitiveMerge *) self;
    g_string_free (upself->super.result, TRUE);

    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_merge_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveMerge *filter;

    filter = (RsvgFilterPrimitiveMerge *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_merge (void)
{
    RsvgFilterPrimitiveMerge *filter;
    filter = g_new (RsvgFilterPrimitiveMerge, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE);
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->super.render = &rsvg_filter_primitive_merge_render;
    filter->super.super.free = &rsvg_filter_primitive_merge_free;

    filter->super.super.set_atts = rsvg_filter_primitive_merge_set_atts;
    return (RsvgNode *) filter;
}

static void
rsvg_filter_primitive_merge_node_set_atts (RsvgNode * self,
                                           RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    if (rsvg_property_bag_size (atts)) {
        /* see bug 145149 - sodipodi generates bad SVG... */
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (((RsvgFilterPrimitive *) self)->in, value);
    }
}

static void
rsvg_filter_primitive_merge_node_free (RsvgNode * self)
{
    RsvgFilterPrimitive *upself;
    upself = (RsvgFilterPrimitive *) self;
    g_string_free (upself->in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_merge_node_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    /* todo */
}

RsvgNode *
rsvg_new_filter_primitive_merge_node (void)
{
    RsvgFilterPrimitive *filter;
    filter = g_new (RsvgFilterPrimitive, 1);
    _rsvg_node_init (&filter->super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE);
    filter->in = g_string_new ("none");
    filter->super.free = rsvg_filter_primitive_merge_node_free;
    filter->render = &rsvg_filter_primitive_merge_node_render;
    filter->super.set_atts = rsvg_filter_primitive_merge_node_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveColourMatrix
 RsvgFilterPrimitiveColourMatrix;

struct _RsvgFilterPrimitiveColourMatrix {
    RsvgFilterPrimitive super;
    gint *KernelMatrix;
};

static void
rsvg_filter_primitive_colour_matrix_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar ch;
    gint x, y;
    gint i;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveColourMatrix *upself;

    cairo_surface_t *output, *in;

    int sum;

    upself = (RsvgFilterPrimitiveColourMatrix *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
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
                    sum = upself->KernelMatrix[umch * 5 + 4];
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
                            sum += upself->KernelMatrix[umch * 5 + umi] *
                                in_pixels[4 * x + y * rowstride + i] / alpha;
                        else
                            sum += upself->KernelMatrix[umch * 5 + umi] *
                                in_pixels[4 * x + y * rowstride + i] / 255;
                    }
                    sum += upself->KernelMatrix[umch * 5 + 4];



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

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy(output);
}

static void
rsvg_filter_primitive_colour_matrix_free (RsvgNode * self)
{
    RsvgFilterPrimitiveColourMatrix *upself;

    upself = (RsvgFilterPrimitiveColourMatrix *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    if (upself->KernelMatrix)
        g_free (upself->KernelMatrix);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_colour_matrix_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                              RsvgPropertyBag * atts)
{
    gint type;
    guint listlen = 0;
    const char *value;
    RsvgFilterPrimitiveColourMatrix *filter;

    filter = (RsvgFilterPrimitiveColourMatrix *) self;

    type = 0;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "values"))) {
            unsigned int i;
            double *temp = rsvg_css_parse_number_list (value, &listlen);
            filter->KernelMatrix = g_new (int, listlen);
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
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
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
rsvg_new_filter_primitive_colour_matrix (void)
{
    RsvgFilterPrimitiveColourMatrix *filter;
    filter = g_new (RsvgFilterPrimitiveColourMatrix, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_COLOUR_MATRIX);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->KernelMatrix = NULL;
    filter->super.render = &rsvg_filter_primitive_colour_matrix_render;
    filter->super.super.free = &rsvg_filter_primitive_colour_matrix_free;

    filter->super.super.set_atts = rsvg_filter_primitive_colour_matrix_set_atts;
    return (RsvgNode *) filter;
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgNodeComponentTransferFunc RsvgNodeComponentTransferFunc;

typedef gint (*ComponentTransferFunc) (gint C, RsvgNodeComponentTransferFunc * user_data);

typedef struct _RsvgFilterPrimitiveComponentTransfer
 RsvgFilterPrimitiveComponentTransfer;

struct _RsvgNodeComponentTransferFunc {
    RsvgNode super;
    ComponentTransferFunc function;
    gint *tableValues;
    guint nbTableValues;
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

    if (!user_data->nbTableValues)
        return C;

    k = (C * (user_data->nbTableValues - 1)) / 255;

    vk = user_data->tableValues[k];
    vk1 = user_data->tableValues[k + 1];

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

    return user_data->tableValues[k];
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

static void
rsvg_filter_primitive_component_transfer_render (RsvgFilterPrimitive *
                                                 self, RsvgFilterContext * ctx)
{
    gint x, y, c;
    guint i;
    gint temp;
    gint rowstride, height, width;
    RsvgIRect boundarys;
    RsvgNodeComponentTransferFunc *channels[4];
    ComponentTransferFunc functions[4];
    guchar *inpix, outpix[4];
    gint achan = ctx->channelmap[3];
    guchar *in_pixels;
    guchar *output_pixels;
    cairo_surface_t *output, *in;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    for (c = 0; c < 4; c++) {
        char channel = "RGBA"[c];
        for (i = 0; i < self->super.children->len; i++) {
            RsvgNode *child_node;

            child_node = (RsvgNode *) g_ptr_array_index (self->super.children, i);
            if (RSVG_NODE_TYPE (child_node) == RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPONENT_TRANSFER) {
                RsvgNodeComponentTransferFunc *temp = (RsvgNodeComponentTransferFunc *) child_node;

                if (temp->channel == channel) {
                    functions[ctx->channelmap[c]] = temp->function;
                    channels[ctx->channelmap[c]] = temp;
                    break;
                }
            }
        }
        if (i == self->super.children->len)
            functions[ctx->channelmap[c]] = identity_component_transfer_func;

    }

    in = rsvg_filter_get_in (self->in, ctx);
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
                int inval;
                if (c != achan) {
                    if (inpix[achan] == 0)
                        inval = 0;
                    else
                        inval = inpix[c] * 255 / inpix[achan];
                } else
                    inval = inpix[c];

                temp = functions[c] (inval, channels[c]);
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

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_component_transfer_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                                   RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveComponentTransfer *filter;

    filter = (RsvgFilterPrimitiveComponentTransfer *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_component_transfer (void)
{
    RsvgFilterPrimitiveComponentTransfer *filter;

    filter = g_new (RsvgFilterPrimitiveComponentTransfer, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPONENT_TRANSFER);
    filter->super.result = g_string_new ("none");
    filter->super.in = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->super.render = &rsvg_filter_primitive_component_transfer_render;

    filter->super.super.set_atts = rsvg_filter_primitive_component_transfer_set_atts;

    return (RsvgNode *) filter;
}

static void
rsvg_node_component_transfer_function_set_atts (RsvgNode * self,
                                                RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgNodeComponentTransferFunc *data = (RsvgNodeComponentTransferFunc *) self;

    if (rsvg_property_bag_size (atts)) {
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
            double *temp = rsvg_css_parse_number_list (value,
                                                       &data->nbTableValues);
            data->tableValues = g_new (gint, data->nbTableValues);
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
}

static void
rsvg_component_transfer_function_free (RsvgNode * self)
{
    RsvgNodeComponentTransferFunc *filter = (RsvgNodeComponentTransferFunc *) self;
    if (filter->nbTableValues)
        g_free (filter->tableValues);
    _rsvg_node_free (self);
}

RsvgNode *
rsvg_new_node_component_transfer_function (char channel)
{
    RsvgNodeComponentTransferFunc *filter;

    filter = g_new (RsvgNodeComponentTransferFunc, 1);
    _rsvg_node_init (&filter->super, RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION);
    filter->super.free = rsvg_component_transfer_function_free;
    filter->super.set_atts = rsvg_node_component_transfer_function_set_atts;
    filter->function = identity_component_transfer_func;
    filter->nbTableValues = 0;
    return (RsvgNode *) filter;
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
rsvg_filter_primitive_erode_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar ch, extreme;
    gint x, y;
    gint i, j;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveErode *upself;

    cairo_surface_t *output, *in;

    gint kx, ky;
    guchar val;

    upself = (RsvgFilterPrimitiveErode *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    rowstride = cairo_image_surface_get_stride (in);

    /* scale the radius values */
    kx = upself->rx * ctx->paffine.xx;
    ky = upself->ry * ctx->paffine.yy;

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++)
            for (ch = 0; ch < 4; ch++) {
                if (upself->mode == 0)
                    extreme = 255;
                else
                    extreme = 0;
                for (i = -ky; i < ky + 1; i++)
                    for (j = -kx; j < kx + 1; j++) {
                        if (y + i >= height || y + i < 0 || x + j >= width || x + j < 0)
                            continue;

                        val = in_pixels[(y + i) * rowstride + (x + j) * 4 + ch];


                        if (upself->mode == 0) {
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

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_erode_free (RsvgNode * self)
{
    RsvgFilterPrimitiveErode *upself;

    upself = (RsvgFilterPrimitiveErode *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_erode_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveErode *filter;

    filter = (RsvgFilterPrimitiveErode *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "radius"))) {
            rsvg_css_parse_number_optional_number (value, &filter->rx, &filter->ry);
        }
        if ((value = rsvg_property_bag_lookup (atts, "operator"))) {
            if (!strcmp (value, "erode"))
                filter->mode = 0;
            else if (!strcmp (value, "dilate"))
                filter->mode = 1;
        }
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_erode (void)
{
    RsvgFilterPrimitiveErode *filter;
    filter = g_new (RsvgFilterPrimitiveErode, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_ERODE);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->rx = 0;
    filter->ry = 0;
    filter->mode = 0;
    filter->super.render = &rsvg_filter_primitive_erode_render;
    filter->super.super.free = &rsvg_filter_primitive_erode_free;
    filter->super.super.set_atts = rsvg_filter_primitive_erode_set_atts;
    return (RsvgNode *) filter;
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

static void
rsvg_filter_primitive_composite_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar i;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *in2_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveComposite *upself;

    cairo_surface_t *output, *in, *in2;

    upself = (RsvgFilterPrimitiveComposite *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in2 = rsvg_filter_get_in (upself->in2, ctx);
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

    if (upself->mode == COMPOSITE_MODE_ARITHMETIC)
        for (y = boundarys.y0; y < boundarys.y1; y++)
            for (x = boundarys.x0; x < boundarys.x1; x++) {
                int qr, qa, qb;

                qa = in_pixels[4 * x + y * rowstride + 3];
                qb = in2_pixels[4 * x + y * rowstride + 3];
                qr = (upself->k1 * qa * qb / 255 + upself->k2 * qa + upself->k3 * qb) / 255;

                if (qr > 255)
                    qr = 255;
                if (qr < 0)
                    qr = 0;
                output_pixels[4 * x + y * rowstride + 3] = qr;
                if (qr)
                    for (i = 0; i < 3; i++) {
                        int ca, cb, cr;
                        ca = in_pixels[4 * x + y * rowstride + i];
                        cb = in2_pixels[4 * x + y * rowstride + i];

                        cr = (ca * cb * upself->k1 / 255 + ca * upself->k2 +
                              cb * upself->k3 + upself->k4 * qr) / 255;
                        if (cr > qr)
                            cr = qr;
                        if (cr < 0)
                            cr = 0;
                        output_pixels[4 * x + y * rowstride + i] = cr;

                    }
            }

    else
        for (y = boundarys.y0; y < boundarys.y1; y++)
            for (x = boundarys.x0; x < boundarys.x1; x++) {
                int qr, cr, qa, qb, ca, cb, Fa, Fb, Fab, Fo;

                qa = in_pixels[4 * x + y * rowstride + 3];
                qb = in2_pixels[4 * x + y * rowstride + 3];
                cr = 0;
                Fa = Fb = Fab = Fo = 0;
                switch (upself->mode) {
                case COMPOSITE_MODE_OVER:
                    Fa = 255;
                    Fb = 255 - qa;
                    break;
                case COMPOSITE_MODE_IN:
                    Fa = qb;
                    Fb = 0;
                    break;
                case COMPOSITE_MODE_OUT:
                    Fa = 255 - qb;
                    Fb = 0;
                    break;
                case COMPOSITE_MODE_ATOP:
                    Fa = qb;
                    Fb = 255 - qa;
                    break;
                case COMPOSITE_MODE_XOR:
                    Fa = 255 - qb;
                    Fb = 255 - qa;
                    break;
                default:
                    break;
                }

                qr = (Fa * qa + Fb * qb) / 255;
                if (qr > 255)
                    qr = 255;
                if (qr < 0)
                    qr = 0;

                for (i = 0; i < 3; i++) {
                    ca = in_pixels[4 * x + y * rowstride + i];
                    cb = in2_pixels[4 * x + y * rowstride + i];

                    cr = (ca * Fa + cb * Fb + ca * cb * Fab + Fo) / 255;
                    if (cr > qr)
                        cr = qr;
                    if (cr < 0)
                        cr = 0;
                    output_pixels[4 * x + y * rowstride + i] = cr;

                }
                output_pixels[4 * x + y * rowstride + 3] = qr;
            }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_composite_free (RsvgNode * self)
{
    RsvgFilterPrimitiveComposite *upself;

    upself = (RsvgFilterPrimitiveComposite *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    g_string_free (upself->in2, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_composite_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveComposite *filter;

    filter = (RsvgFilterPrimitiveComposite *) self;

    if (rsvg_property_bag_size (atts)) {
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
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "k1")))
            filter->k1 = g_ascii_strtod (value, NULL) * 255.;
        if ((value = rsvg_property_bag_lookup (atts, "k2")))
            filter->k2 = g_ascii_strtod (value, NULL) * 255.;
        if ((value = rsvg_property_bag_lookup (atts, "k3")))
            filter->k3 = g_ascii_strtod (value, NULL) * 255.;
        if ((value = rsvg_property_bag_lookup (atts, "k4")))
            filter->k4 = g_ascii_strtod (value, NULL) * 255.;
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_composite (void)
{
    RsvgFilterPrimitiveComposite *filter;
    filter = g_new (RsvgFilterPrimitiveComposite, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPOSITE);
    filter->mode = COMPOSITE_MODE_OVER;
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->k1 = 0;
    filter->k2 = 0;
    filter->k3 = 0;
    filter->k4 = 0;
    filter->super.render = &rsvg_filter_primitive_composite_render;
    filter->super.super.free = &rsvg_filter_primitive_composite_free;
    filter->super.super.set_atts = rsvg_filter_primitive_composite_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

static void
rsvg_filter_primitive_flood_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar i;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;
    guchar *output_pixels;
    cairo_surface_t *output;
    char pixcolour[4];
    RsvgFilterPrimitiveOutput out;

    guint32 colour = self->super.state->flood_color;
    guint8 opacity = self->super.state->flood_opacity;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    height = ctx->height;
    width = ctx->width;
    output = _rsvg_image_surface_new (width, height);
    if (output == NULL)
        return;

    rowstride = cairo_image_surface_get_stride (output);

    output_pixels = cairo_image_surface_get_data (output);

    for (i = 0; i < 3; i++)
        pixcolour[i] = (int) (((unsigned char *)
                               (&colour))[2 - i]) * opacity / 255;
    pixcolour[3] = opacity;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++)
            for (i = 0; i < 4; i++)
                output_pixels[4 * x + y * rowstride + ctx->channelmap[i]] = pixcolour[i];

    cairo_surface_mark_dirty (output);

    out.surface = output;
    out.Rused = 1;
    out.Gused = 1;
    out.Bused = 1;
    out.Aused = 1;
    out.bounds = boundarys;

    rsvg_filter_store_output (self->result, out, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_flood_free (RsvgNode * self)
{
    RsvgFilterPrimitive *upself;

    upself = (RsvgFilterPrimitive *) self;
    g_string_free (upself->result, TRUE);
    g_string_free (upself->in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_flood_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value, *id = NULL;
    RsvgFilterPrimitive *filter = (RsvgFilterPrimitive *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, id = value, &filter->super);
        rsvg_parse_style_attrs (ctx, self->state, "feFlood", NULL, id, atts);
    }
}

RsvgNode *
rsvg_new_filter_primitive_flood (void)
{
    RsvgFilterPrimitive *filter;
    filter = g_new (RsvgFilterPrimitive, 1);
    _rsvg_node_init (&filter->super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_FLOOD);
    filter->in = g_string_new ("none");
    filter->result = g_string_new ("none");
    filter->x.factor = filter->y.factor = filter->width.factor = filter->height.factor = 'n';
    filter->render = &rsvg_filter_primitive_flood_render;
    filter->super.free = &rsvg_filter_primitive_flood_free;
    filter->super.set_atts = rsvg_filter_primitive_flood_set_atts;
    return (RsvgNode *) filter;
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
rsvg_filter_primitive_displacement_map_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar ch, xch, ych;
    gint x, y;
    gint rowstride, height, width;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *in2_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveDisplacementMap *upself;

    cairo_surface_t *output, *in, *in2;

    double ox, oy;

    upself = (RsvgFilterPrimitiveDisplacementMap *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in2 = rsvg_filter_get_in (upself->in2, ctx);
    if (in == NULL) {
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

    switch (upself->xChannelSelector) {
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
        xch = 4;
    };

    switch (upself->yChannelSelector) {
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
        ych = 4;
    };

    xch = ctx->channelmap[xch];
    ych = ctx->channelmap[ych];
    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            if (xch != 4)
                ox = x + upself->scale * ctx->paffine.xx *
                    ((double) in2_pixels[y * rowstride + x * 4 + xch] / 255.0 - 0.5);
            else
                ox = x;

            if (ych != 4)
                oy = y + upself->scale * ctx->paffine.yy *
                    ((double) in2_pixels[y * rowstride + x * 4 + ych] / 255.0 - 0.5);
            else
                oy = y;

            for (ch = 0; ch < 4; ch++) {
                output_pixels[y * rowstride + x * 4 + ch] =
                    get_interp_pixel (in_pixels, ox, oy, ch, boundarys, rowstride);
            }
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (in2);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_displacement_map_free (RsvgNode * self)
{
    RsvgFilterPrimitiveDisplacementMap *upself;

    upself = (RsvgFilterPrimitiveDisplacementMap *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    g_string_free (upself->in2, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_displacement_map_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                                 RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveDisplacementMap *filter;

    filter = (RsvgFilterPrimitiveDisplacementMap *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "in2")))
            g_string_assign (filter->in2, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "xChannelSelector")))
            filter->xChannelSelector = (value)[0];
        if ((value = rsvg_property_bag_lookup (atts, "yChannelSelector")))
            filter->yChannelSelector = (value)[0];
        if ((value = rsvg_property_bag_lookup (atts, "scale")))
            filter->scale = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_displacement_map (void)
{
    RsvgFilterPrimitiveDisplacementMap *filter;
    filter = g_new (RsvgFilterPrimitiveDisplacementMap, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_DISPLACEMENT_MAP);
    filter->super.in = g_string_new ("none");
    filter->in2 = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->xChannelSelector = ' ';
    filter->yChannelSelector = ' ';
    filter->scale = 0;
    filter->super.render = &rsvg_filter_primitive_displacement_map_render;
    filter->super.super.free = &rsvg_filter_primitive_displacement_map_free;
    filter->super.super.set_atts = rsvg_filter_primitive_displacement_map_set_atts;
    return (RsvgNode *) filter;
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
rsvg_filter_primitive_turbulence_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgFilterPrimitiveTurbulence *upself;
    gint x, y, tileWidth, tileHeight, rowstride, width, height;
    RsvgIRect boundarys;
    guchar *output_pixels;
    cairo_surface_t *output, *in;
    cairo_matrix_t affine;

    affine = ctx->paffine;
    if (cairo_matrix_invert (&affine) != CAIRO_STATUS_SUCCESS)
      return;

    in = rsvg_filter_get_in (self->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);
    rowstride = cairo_image_surface_get_stride (in);

    upself = (RsvgFilterPrimitiveTurbulence *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

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

                cr = feTurbulence_turbulence (upself, i, point, (double) x, (double) y,
                                              (double) tileWidth, (double) tileHeight);

                if (upself->bFractalSum)
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

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_turbulence_free (RsvgNode * self)
{
    RsvgFilterPrimitiveTurbulence *upself;

    upself = (RsvgFilterPrimitiveTurbulence *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_turbulence_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                           RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveTurbulence *filter;

    filter = (RsvgFilterPrimitiveTurbulence *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "baseFrequency")))
            rsvg_css_parse_number_optional_number (value, &filter->fBaseFreqX, &filter->fBaseFreqY);
        if ((value = rsvg_property_bag_lookup (atts, "numOctaves")))
            filter->nNumOctaves = atoi (value);
        if ((value = rsvg_property_bag_lookup (atts, "seed")))
            filter->seed = atoi (value);
        if ((value = rsvg_property_bag_lookup (atts, "stitchTiles")))
            filter->bDoStitching = (!strcmp (value, "stitch"));
        if ((value = rsvg_property_bag_lookup (atts, "type")))
            filter->bFractalSum = (!strcmp (value, "fractalNoise"));
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_turbulence (void)
{
    RsvgFilterPrimitiveTurbulence *filter;
    filter = g_new (RsvgFilterPrimitiveTurbulence, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_TURBULENCE);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->fBaseFreqX = 0;
    filter->fBaseFreqY = 0;
    filter->nNumOctaves = 1;
    filter->seed = 0;
    filter->bDoStitching = 0;
    filter->bFractalSum = 0;
    feTurbulence_init (filter);
    filter->super.render = &rsvg_filter_primitive_turbulence_render;
    filter->super.super.free = &rsvg_filter_primitive_turbulence_free;
    filter->super.super.set_atts = rsvg_filter_primitive_turbulence_set_atts;
    return (RsvgNode *) filter;
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
rsvg_filter_primitive_image_render_in (RsvgFilterPrimitive * self, RsvgFilterContext * context)
{
    RsvgDrawingCtx *ctx;
    RsvgFilterPrimitiveImage *upself;
    RsvgNode *drawable;

    ctx = context->ctx;

    upself = (RsvgFilterPrimitiveImage *) self;

    if (!upself->href)
        return NULL;

    drawable = rsvg_defs_lookup (ctx->defs, upself->href->str);
    if (!drawable)
        return NULL;

    rsvg_current_state (ctx)->affine = context->paffine;

    return rsvg_get_surface_of_node (ctx, drawable, context->width, context->height);
}

static cairo_surface_t *
rsvg_filter_primitive_image_render_ext (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgIRect boundarys;
    RsvgFilterPrimitiveImage *upself;
    cairo_surface_t *img, *intermediate;
    int i;
    unsigned char *pixels;
    int channelmap[4];
    int length;
    int width, height;

    upself = (RsvgFilterPrimitiveImage *) self;

    if (!upself->href)
        return NULL;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    width = boundarys.x1 - boundarys.x0;
    height = boundarys.y1 - boundarys.y0;
    if (width == 0 || height == 0)
        return NULL;

    img = rsvg_cairo_surface_new_from_href (upself->ctx,
                                            upself->href->str,
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
rsvg_filter_primitive_image_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    RsvgIRect boundarys;
    RsvgFilterPrimitiveImage *upself;
    RsvgFilterPrimitiveOutput op;
    int x, y;

    cairo_surface_t *output, *img;

    upself = (RsvgFilterPrimitiveImage *) self;

    if (!upself->href)
        return;

    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (output == NULL)
        return;

    img = rsvg_filter_primitive_image_render_in (self, ctx);
    if (img == NULL) {
        img = rsvg_filter_primitive_image_render_ext (self, ctx);
        x = y = 0;
    } else {
        x = boundarys.x0;
        y = boundarys.y0;
    }
    if (img) {
        cairo_t *cr;

        cr = cairo_create (output);
        cairo_set_source_surface (cr, img, x, y);
        cairo_rectangle (cr, 0, 0,
                         boundarys.x1 - boundarys.x0,
                         boundarys.y1 - boundarys.y0);
        cairo_clip (cr);
        cairo_translate (cr, -boundarys.x0, -boundarys.y0);
        cairo_paint (cr);
        cairo_destroy (cr);

        cairo_surface_destroy (img);
    }

    op.surface = output;
    op.bounds = boundarys;
    op.Rused = 1;
    op.Gused = 1;
    op.Bused = 1;
    op.Aused = 1;

    rsvg_filter_store_output (self->result, op, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_image_free (RsvgNode * self)
{
    RsvgFilterPrimitiveImage *upself;

    upself = (RsvgFilterPrimitiveImage *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);

    if (upself->href)
        g_string_free (upself->href, TRUE);

    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_image_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveImage *filter;

    filter = (RsvgFilterPrimitiveImage *) self;
    filter->ctx = ctx;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
            filter->href = g_string_new (NULL);
            g_string_assign (filter->href, value);
        }
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_image (void)
{
    RsvgFilterPrimitiveImage *filter;
    filter = g_new (RsvgFilterPrimitiveImage, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_IMAGE);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->super.render = &rsvg_filter_primitive_image_render;
    filter->super.super.free = &rsvg_filter_primitive_image_free;
    filter->super.super.set_atts = rsvg_filter_primitive_image_set_atts;
    filter->href = NULL;
    return (RsvgNode *) filter;
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
    RsvgNode super;
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
            output.x = _rsvg_css_normalize_length (&source->x, ctx, 'h') - x;
            output.y = _rsvg_css_normalize_length (&source->y, ctx, 'v') - y;
            output.z = _rsvg_css_normalize_length (&source->z, ctx, 'o') - z;
            output = normalise (output);
        }
        break;
    }
    return output;
}

static vector3
get_light_colour (RsvgNodeLightSource * source, vector3 colour,
                  gdouble x1, gdouble y1, gdouble z, cairo_matrix_t *affine, RsvgDrawingCtx * ctx)
{
    double base, angle, x, y;
    vector3 s;
    vector3 L;
    vector3 output;
    double sx, sy, sz, spx, spy, spz;

    if (source->type != SPOTLIGHT)
        return colour;

    sx = _rsvg_css_normalize_length (&source->x, ctx, 'h');
    sy = _rsvg_css_normalize_length (&source->y, ctx, 'v');
    sz = _rsvg_css_normalize_length (&source->z, ctx, 'o');
    spx = _rsvg_css_normalize_length (&source->pointsAtX, ctx, 'h');
    spy = _rsvg_css_normalize_length (&source->pointsAtY, ctx, 'v');
    spz = _rsvg_css_normalize_length (&source->pointsAtZ, ctx, 'o');

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

    output.x = colour.x * pow (base, source->specularExponent);
    output.y = colour.y * pow (base, source->specularExponent);
    output.z = colour.z * pow (base, source->specularExponent);

    return output;
}


static void
rsvg_node_light_source_set_atts (RsvgNode * self,
                                 RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgNodeLightSource *data;
    const char *value;

    data = (RsvgNodeLightSource *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "azimuth")))
            data->azimuth = rsvg_css_parse_angle (value) / 180.0 * M_PI;
        if ((value = rsvg_property_bag_lookup (atts, "elevation")))
            data->elevation = rsvg_css_parse_angle (value) / 180.0 * M_PI;
        if ((value = rsvg_property_bag_lookup (atts, "limitingConeAngle")))
            data->limitingconeAngle = rsvg_css_parse_angle (value) / 180.0 * M_PI;
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            data->x = data->pointsAtX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            data->y = data->pointsAtX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "z")))
            data->z = data->pointsAtX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "pointsAtX")))
            data->pointsAtX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "pointsAtY")))
            data->pointsAtY = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "pointsAtZ")))
            data->pointsAtZ = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "specularExponent")))
            data->specularExponent = g_ascii_strtod (value, NULL);
    }
}

RsvgNode *
rsvg_new_node_light_source (char type)
{
    RsvgNodeLightSource *data;
    data = g_new (RsvgNodeLightSource, 1);
    _rsvg_node_init (&data->super, RSVG_NODE_TYPE_LIGHT_SOURCE);
    data->super.free = _rsvg_node_free;
    data->super.set_atts = rsvg_node_light_source_set_atts;
    data->specularExponent = 1;
    if (type == 's')
        data->type = SPOTLIGHT;
    else if (type == 'd')
        data->type = DISTANTLIGHT;
    else
        data->type = POINTLIGHT;
    data->limitingconeAngle = 180;
    return &data->super;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveDiffuseLighting RsvgFilterPrimitiveDiffuseLighting;

struct _RsvgFilterPrimitiveDiffuseLighting {
    RsvgFilterPrimitive super;
    gdouble dx, dy;
    double diffuseConstant;
    double surfaceScale;
    guint32 lightingcolour;
};

static void
rsvg_filter_primitive_diffuse_lighting_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    gint x, y;
    float dy, dx, rawdy, rawdx;
    gdouble z;
    gint rowstride, height, width;
    gdouble factor, surfaceScale;
    vector3 lightcolour, L, N;
    vector3 colour;
    cairo_matrix_t iaffine;
    RsvgNodeLightSource *source = NULL;
    RsvgIRect boundarys;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveDiffuseLighting *upself;

    cairo_surface_t *output, *in;

    unsigned int i;

    for (i = 0; i < self->super.children->len; i++) {
        RsvgNode *temp;

        temp = g_ptr_array_index (self->super.children, i);
        if (RSVG_NODE_TYPE (temp) == RSVG_NODE_TYPE_LIGHT_SOURCE) {
            source = (RsvgNodeLightSource *) temp;
        }
    }
    if (source == NULL)
        return;

    iaffine = ctx->paffine;
    if (cairo_matrix_invert (&iaffine) != CAIRO_STATUS_SUCCESS)
      return;

    upself = (RsvgFilterPrimitiveDiffuseLighting *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
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

    colour.x = ((guchar *) (&upself->lightingcolour))[2] / 255.0;
    colour.y = ((guchar *) (&upself->lightingcolour))[1] / 255.0;
    colour.z = ((guchar *) (&upself->lightingcolour))[0] / 255.0;

    surfaceScale = upself->surfaceScale / 255.0;

    if (upself->dy < 0 || upself->dx < 0) {
        dx = 1;
        dy = 1;
        rawdx = 1;
        rawdy = 1;
    } else {
        dx = upself->dx * ctx->paffine.xx;
        dy = upself->dy * ctx->paffine.yy;
        rawdx = upself->dx;
        rawdy = upself->dy;
    }

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = surfaceScale * (double) in_pixels[y * rowstride + x * 4 + ctx->channelmap[3]];
            L = get_light_direction (source, x, y, z, &iaffine, ctx->ctx);
            N = get_surface_normal (in_pixels, boundarys, x, y,
                                    dx, dy, rawdx, rawdy, upself->surfaceScale,
                                    rowstride, ctx->channelmap[3]);
            lightcolour = get_light_colour (source, colour, x, y, z, &iaffine, ctx->ctx);
            factor = dotproduct (N, L);

            output_pixels[y * rowstride + x * 4 + ctx->channelmap[0]] =
                MAX (0, MIN (255, upself->diffuseConstant * factor * lightcolour.x * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[1]] =
                MAX (0, MIN (255, upself->diffuseConstant * factor * lightcolour.y * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[2]] =
                MAX (0, MIN (255, upself->diffuseConstant * factor * lightcolour.z * 255.0));
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[3]] = 255;
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_diffuse_lighting_free (RsvgNode * self)
{
    RsvgFilterPrimitiveDiffuseLighting *upself;

    upself = (RsvgFilterPrimitiveDiffuseLighting *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_diffuse_lighting_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                                 RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveDiffuseLighting *filter;

    filter = (RsvgFilterPrimitiveDiffuseLighting *) self;


    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "kernelUnitLength")))
            rsvg_css_parse_number_optional_number (value, &filter->dx, &filter->dy);
        if ((value = rsvg_property_bag_lookup (atts, "lighting-color")))
            filter->lightingcolour = rsvg_css_parse_color (value, 0);
        if ((value = rsvg_property_bag_lookup (atts, "diffuseConstant")))
            filter->diffuseConstant = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "surfaceScale")))
            filter->surfaceScale = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_diffuse_lighting (void)
{
    RsvgFilterPrimitiveDiffuseLighting *filter;
    filter = g_new (RsvgFilterPrimitiveDiffuseLighting, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_DIFFUSE_LIGHTING);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->surfaceScale = 1;
    filter->diffuseConstant = 1;
    filter->dx = 1;
    filter->dy = 1;
    filter->lightingcolour = 0xFFFFFFFF;
    filter->super.render = &rsvg_filter_primitive_diffuse_lighting_render;
    filter->super.super.free = &rsvg_filter_primitive_diffuse_lighting_free;
    filter->super.super.set_atts = rsvg_filter_primitive_diffuse_lighting_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveSpecularLighting RsvgFilterPrimitiveSpecularLighting;

struct _RsvgFilterPrimitiveSpecularLighting {
    RsvgFilterPrimitive super;
    double specularConstant;
    double specularExponent;
    double surfaceScale;
    guint32 lightingcolour;
};

static void
rsvg_filter_primitive_specular_lighting_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    gint x, y;
    gdouble z, surfaceScale;
    gint rowstride, height, width;
    gdouble factor, max, base;
    vector3 lightcolour, colour;
    vector3 L;
    cairo_matrix_t iaffine;
    RsvgIRect boundarys;
    RsvgNodeLightSource *source = NULL;

    guchar *in_pixels;
    guchar *output_pixels;

    RsvgFilterPrimitiveSpecularLighting *upself;

    cairo_surface_t *output, *in;

    unsigned int i;

    for (i = 0; i < self->super.children->len; i++) {
        RsvgNode *temp;
        temp = g_ptr_array_index (self->super.children, i);
        if (RSVG_NODE_TYPE (temp) == RSVG_NODE_TYPE_LIGHT_SOURCE) {
            source = (RsvgNodeLightSource *) temp;
        }
    }
    if (source == NULL)
        return;

    iaffine = ctx->paffine;
    if (cairo_matrix_invert (&iaffine) != CAIRO_STATUS_SUCCESS)
      return;

    upself = (RsvgFilterPrimitiveSpecularLighting *) self;
    boundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    in = rsvg_filter_get_in (self->in, ctx);
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

    colour.x = ((guchar *) (&upself->lightingcolour))[2] / 255.0;
    colour.y = ((guchar *) (&upself->lightingcolour))[1] / 255.0;
    colour.z = ((guchar *) (&upself->lightingcolour))[0] / 255.0;

    surfaceScale = upself->surfaceScale / 255.0;

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            z = in_pixels[y * rowstride + x * 4 + 3] * surfaceScale;
            L = get_light_direction (source, x, y, z, &iaffine, ctx->ctx);
            L.z += 1;
            L = normalise (L);

            lightcolour = get_light_colour (source, colour, x, y, z, &iaffine, ctx->ctx);
            base = dotproduct (get_surface_normal (in_pixels, boundarys, x, y,
                                                   1, 1, 1.0 / ctx->paffine.xx,
                                                   1.0 / ctx->paffine.yy, upself->surfaceScale,
                                                   rowstride, ctx->channelmap[3]), L);

            factor = upself->specularConstant * pow (base, upself->specularExponent) * 255;

            max = 0;
            if (max < lightcolour.x)
                max = lightcolour.x;
            if (max < lightcolour.y)
                max = lightcolour.y;
            if (max < lightcolour.z)
                max = lightcolour.z;

            max *= factor;
            if (max > 255)
                max = 255;
            if (max < 0)
                max = 0;

            output_pixels[y * rowstride + x * 4 + ctx->channelmap[0]] = lightcolour.x * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[1]] = lightcolour.y * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[2]] = lightcolour.z * max;
            output_pixels[y * rowstride + x * 4 + ctx->channelmap[3]] = max;

        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_specular_lighting_free (RsvgNode * self)
{
    RsvgFilterPrimitiveSpecularLighting *upself;

    upself = (RsvgFilterPrimitiveSpecularLighting *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_specular_lighting_set_atts (RsvgNode * self, RsvgHandle * ctx,
                                                  RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveSpecularLighting *filter;

    filter = (RsvgFilterPrimitiveSpecularLighting *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "lighting-color")))
            filter->lightingcolour = rsvg_css_parse_color (value, 0);
        if ((value = rsvg_property_bag_lookup (atts, "specularConstant")))
            filter->specularConstant = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "specularExponent")))
            filter->specularExponent = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "surfaceScale")))
            filter->surfaceScale = g_ascii_strtod (value, NULL);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}


RsvgNode *
rsvg_new_filter_primitive_specular_lighting (void)
{
    RsvgFilterPrimitiveSpecularLighting *filter;
    filter = g_new (RsvgFilterPrimitiveSpecularLighting, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_SPECULAR_LIGHTING);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->surfaceScale = 1;
    filter->specularConstant = 1;
    filter->specularExponent = 1;
    filter->lightingcolour = 0xFFFFFFFF;
    filter->super.render = &rsvg_filter_primitive_specular_lighting_render;
    filter->super.super.free = &rsvg_filter_primitive_specular_lighting_free;
    filter->super.super.set_atts = rsvg_filter_primitive_specular_lighting_set_atts;
    return (RsvgNode *) filter;
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveTile
 RsvgFilterPrimitiveTile;

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
rsvg_filter_primitive_tile_render (RsvgFilterPrimitive * self, RsvgFilterContext * ctx)
{
    guchar i;
    gint x, y, rowstride;
    RsvgIRect boundarys, oboundarys;

    RsvgFilterPrimitiveOutput input;

    guchar *in_pixels;
    guchar *output_pixels;

    cairo_surface_t *output, *in;

    oboundarys = rsvg_filter_primitive_get_bounds (self, ctx);

    input = rsvg_filter_get_result (self->in, ctx);
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

    rsvg_filter_store_result (self->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_tile_free (RsvgNode * self)
{
    RsvgFilterPrimitiveTile *upself;

    upself = (RsvgFilterPrimitiveTile *) self;
    g_string_free (upself->super.result, TRUE);
    g_string_free (upself->super.in, TRUE);
    _rsvg_node_free (self);
}

static void
rsvg_filter_primitive_tile_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgFilterPrimitiveTile *filter;

    filter = (RsvgFilterPrimitiveTile *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "in")))
            g_string_assign (filter->super.in, value);
        if ((value = rsvg_property_bag_lookup (atts, "result")))
            g_string_assign (filter->super.result, value);
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            filter->super.x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            filter->super.y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            filter->super.width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            filter->super.height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &filter->super.super);
    }
}

RsvgNode *
rsvg_new_filter_primitive_tile (void)
{
    RsvgFilterPrimitiveTile *filter;
    filter = g_new (RsvgFilterPrimitiveTile, 1);
    _rsvg_node_init (&filter->super.super, RSVG_NODE_TYPE_FILTER_PRIMITIVE_TILE);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->super.x.factor = filter->super.y.factor = filter->super.width.factor =
        filter->super.height.factor = 'n';
    filter->super.render = &rsvg_filter_primitive_tile_render;
    filter->super.super.free = &rsvg_filter_primitive_tile_free;
    filter->super.super.set_atts = rsvg_filter_primitive_tile_set_atts;
    return (RsvgNode *) filter;
}
