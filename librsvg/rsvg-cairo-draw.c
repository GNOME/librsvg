/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-shapes.c: Draw shapes with cairo

   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2005 Caleb Moore <c.moore@student.unsw.edu.au>
   Copyright (C) 2005 Red Hat, Inc.

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

   Authors: Dom Lachowicz <cinamod@hotmail.com>,
            Caleb Moore <c.moore@student.unsw.edu.au>
            Carl Worth <cworth@cworth.org>
*/

#include "config.h"

#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-render.h"
#include "rsvg-cairo-clip.h"
#include "rsvg-styles.h"
#include "rsvg-path-builder.h"
#include "rsvg-filter.h"
#include "rsvg-structure.h"

#include <math.h>
#include <string.h>

#include <pango/pangocairo.h>
#ifdef HAVE_PANGO_FT2
#include <pango/pangofc-fontmap.h>
#endif

/* Implemented in rsvg_internals/src/paint_server.rs */
G_GNUC_INTERNAL
gboolean rsvg_set_source_rsvg_paint_server (RsvgDrawingCtx * ctx,
                                            RsvgPaintServer * ps,
                                            guint8 opacity,
                                            RsvgBbox bbox,
                                            guint32 current_color);

/* Implemented in rsvg_internals/src/draw.rs */
G_GNUC_INTERNAL
void rsvg_cairo_add_clipping_rect (RsvgDrawingCtx *ctx,
                                   double x,
                                   double y,
                                   double w,
                                   double h);

#ifdef HAVE_PANGOFT2
static cairo_font_options_t *
get_font_options_for_testing (void)
{
    cairo_font_options_t *options;

    options = cairo_font_options_create ();
    cairo_font_options_set_antialias (options, CAIRO_ANTIALIAS_GRAY);
    cairo_font_options_set_hint_style (options, CAIRO_HINT_STYLE_FULL);
    cairo_font_options_set_hint_metrics (options, CAIRO_HINT_METRICS_ON);

    return options;
}

static void
set_font_options_for_testing (PangoContext *context)
{
    cairo_font_options_t *font_options;

    font_options = get_font_options_for_testing ();
    pango_cairo_context_set_font_options (context, font_options);
    cairo_font_options_destroy (font_options);
}

static void
create_font_config_for_testing (RsvgCairoRender *render)
{
    const char *font_paths[] = {
        SRCDIR "/tests/resources/Roboto-Regular.ttf",
        SRCDIR "/tests/resources/Roboto-Italic.ttf",
        SRCDIR "/tests/resources/Roboto-Bold.ttf",
        SRCDIR "/tests/resources/Roboto-BoldItalic.ttf",
    };

    int i;

    if (render->font_config_for_testing != NULL)
        return;

    render->font_config_for_testing = FcConfigCreate ();

    for (i = 0; i < G_N_ELEMENTS(font_paths); i++) {
        if (!FcConfigAppFontAddFile (render->font_config_for_testing, (const FcChar8 *) font_paths[i])) {
            g_error ("Could not load font file \"%s\" for tests; aborting", font_paths[i]);
        }
    }
}

static PangoFontMap *
get_font_map_for_testing (RsvgCairoRender *render)
{
    create_font_config_for_testing (render);

    if (!render->font_map_for_testing) {
        render->font_map_for_testing = pango_cairo_font_map_new_for_font_type (CAIRO_FONT_TYPE_FT);
        pango_fc_font_map_set_config (PANGO_FC_FONT_MAP (render->font_map_for_testing),
                                      render->font_config_for_testing);
    }

    return render->font_map_for_testing;
}
#endif

PangoContext *
rsvg_cairo_get_pango_context (RsvgDrawingCtx * ctx)
{
    PangoFontMap *fontmap;
    PangoContext *context;
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);
    double dpi_y;

#ifdef HAVE_PANGOFT2
    if (ctx->is_testing) {
        fontmap = get_font_map_for_testing (render);
    } else {
#endif
        fontmap = pango_cairo_font_map_get_default ();
#ifdef HAVE_PANGOFT2
    }
#endif

    context = pango_font_map_create_context (fontmap);
    pango_cairo_update_context (render->cr, context);

    rsvg_drawing_ctx_get_dpi (ctx, NULL, &dpi_y);
    pango_cairo_context_set_resolution (context, dpi_y);

#ifdef HAVE_PANGOFT2
    if (ctx->is_testing) {
        set_font_options_for_testing (context);
    }
#endif

    return context;
}

cairo_t *
rsvg_cairo_get_cairo_context (RsvgDrawingCtx *ctx)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);

    return render->cr;
}

/* FIXME: Usage of this function is more less a hack.  Some code does this:
 *
 *   save_cr = rsvg_cairo_get_cairo_context (ctx);
 *
 *   some_surface = create_surface ();
 *
 *   cr = cairo_create (some_surface);
 *
 *   rsvg_cairo_set_cairo_context (ctx, cr);
 *
 *   ... draw with ctx but to that temporary surface
 *
 *   rsvg_cairo_set_cairo_context (ctx, save_cr);
 *
 * It would be better to have an explicit push/pop for the cairo_t, or
 * pushing a temporary surface, or something that does not involve
 * monkeypatching the cr directly.
 */
void
rsvg_cairo_set_cairo_context (RsvgDrawingCtx *ctx, cairo_t *cr)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);

    render->cr = cr;
}

static void
rsvg_cairo_generate_mask (cairo_t * cr, RsvgNode *mask, RsvgDrawingCtx *ctx, RsvgBbox *bbox)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);
    cairo_surface_t *surface;
    cairo_t *mask_cr, *save_cr;
    RsvgState *state = rsvg_drawing_ctx_get_current_state (ctx);
    guint8 *pixels;
    guint32 width = render->width, height = render->height;
    guint32 rowstride = width * 4, row, i;
    cairo_matrix_t affinesave;
    RsvgLength mask_x, mask_y, mask_w, mask_h;
    double sx, sy, sw, sh;
    gboolean nest = cr != render->initial_cr;
    RsvgCoordUnits mask_units;
    RsvgCoordUnits content_units;

    g_assert (rsvg_node_get_type (mask) == RSVG_NODE_TYPE_MASK);

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return;
    }

    pixels = cairo_image_surface_get_data (surface);
    rowstride = cairo_image_surface_get_stride (surface);

    mask_units    = rsvg_node_mask_get_units (mask);
    content_units = rsvg_node_mask_get_content_units (mask);

    if (mask_units == objectBoundingBox)
        rsvg_drawing_ctx_push_view_box (ctx, 1, 1);

    mask_x = rsvg_node_mask_get_x (mask);
    mask_y = rsvg_node_mask_get_y (mask);
    mask_w = rsvg_node_mask_get_width (mask);
    mask_h = rsvg_node_mask_get_height (mask);

    sx = rsvg_length_normalize (&mask_x, ctx);
    sy = rsvg_length_normalize (&mask_y, ctx);
    sw = rsvg_length_normalize (&mask_w, ctx);
    sh = rsvg_length_normalize (&mask_h, ctx);

    if (mask_units == objectBoundingBox)
        rsvg_drawing_ctx_pop_view_box (ctx);

    mask_cr = cairo_create (surface);
    save_cr = render->cr;
    render->cr = mask_cr;

    if (mask_units == objectBoundingBox)
        rsvg_cairo_add_clipping_rect (ctx,
                                      sx * bbox->rect.width + bbox->rect.x,
                                      sy * bbox->rect.height + bbox->rect.y,
                                      sw * bbox->rect.width,
                                      sh * bbox->rect.height);
    else
        rsvg_cairo_add_clipping_rect (ctx, sx, sy, sw, sh);

    /* Horribly dirty hack to have the bbox premultiplied to everything */
    if (content_units == objectBoundingBox) {
        cairo_matrix_t bbtransform;
        RsvgState *mask_state;

        cairo_matrix_init (&bbtransform,
                           bbox->rect.width,
                           0,
                           0,
                           bbox->rect.height,
                           bbox->rect.x,
                           bbox->rect.y);

        mask_state = rsvg_node_get_state (mask);

        affinesave = rsvg_state_get_affine (mask_state);
        cairo_matrix_multiply (&bbtransform, &bbtransform, &affinesave);
        rsvg_state_set_affine (mask_state, bbtransform);
        rsvg_drawing_ctx_push_view_box (ctx, 1, 1);
    }

    rsvg_drawing_ctx_state_push (ctx);
    rsvg_node_draw_children (mask, ctx, 0, FALSE);
    rsvg_drawing_ctx_state_pop (ctx);

    if (content_units == objectBoundingBox) {
        RsvgState *mask_state;

        rsvg_drawing_ctx_pop_view_box (ctx);

        mask_state = rsvg_node_get_state (mask);
        rsvg_state_set_affine (mask_state, affinesave);
    }

    render->cr = save_cr;

    for (row = 0; row < height; row++) {
        guint8 *row_data = (pixels + (row * rowstride));
        for (i = 0; i < width; i++) {
            guint32 *pixel = (guint32 *) row_data + i;
            /*
             *  Assuming, the pixel is linear RGB (not sRGB)
             *  y = luminance
             *  Y = 0.2126 R + 0.7152 G + 0.0722 B
             *  1.0 opacity = 255
             *
             *  When Y = 1.0, pixel for mask should be 0xFFFFFFFF
             *  	(you get 1.0 luminance from 255 from R, G and B)
             *
             *	r_mult = 0xFFFFFFFF / (255.0 * 255.0) * .2126 = 14042.45  ~= 14042
             *	g_mult = 0xFFFFFFFF / (255.0 * 255.0) * .7152 = 47239.69  ~= 47240
             *	b_mult = 0xFFFFFFFF / (255.0 * 255.0) * .0722 =  4768.88  ~= 4769
             *
             * 	This allows for the following expected behaviour:
             *  (we only care about the most sig byte)
             *	if pixel = 0x00FFFFFF, pixel' = 0xFF......
             *	if pixel = 0x00020202, pixel' = 0x02......
             *	if pixel = 0x00000000, pixel' = 0x00......
             */
            *pixel = ((((*pixel & 0x00ff0000) >> 16) * 14042 +
                       ((*pixel & 0x0000ff00) >>  8) * 47240 +
                       ((*pixel & 0x000000ff)      ) * 4769    ) * state->opacity);
        }
    }

    cairo_destroy (mask_cr);

    cairo_identity_matrix (cr);
    cairo_mask_surface (cr, surface,
                        nest ? 0 : render->offset_x,
                        nest ? 0 : render->offset_y);
    cairo_surface_destroy (surface);
}

static void
rsvg_cairo_push_render_stack (RsvgDrawingCtx * ctx)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);
    cairo_surface_t *surface;
    cairo_t *child_cr;
    RsvgBbox *bbox;
    RsvgState *state = rsvg_drawing_ctx_get_current_state (ctx);
    gboolean lateclip = FALSE;
    cairo_matrix_t affine;

    if (state->clip_path) {
        RsvgNode *node;
        node = rsvg_drawing_ctx_acquire_node_of_type (ctx, state->clip_path, RSVG_NODE_TYPE_CLIP_PATH);
        if (node) {
            switch (rsvg_node_clip_path_get_units (node)) {
            case userSpaceOnUse:
                rsvg_cairo_clip (ctx, node, NULL);
                break;
            case objectBoundingBox:
                lateclip = TRUE;
                break;

            default:
                g_assert_not_reached ();
                break;
            }

            rsvg_drawing_ctx_release_node (ctx, node);
        }
    }

    if (state->opacity == 0xFF
        && !state->filter && !state->mask && !lateclip && (state->comp_op == CAIRO_OPERATOR_OVER)
        && (state->enable_background == RSVG_ENABLE_BACKGROUND_ACCUMULATE))
        return;

    if (!state->filter) {
        surface = cairo_surface_create_similar (cairo_get_target (render->cr),
                                                CAIRO_CONTENT_COLOR_ALPHA,
                                                render->width, render->height);
    } else {
        surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
                                              render->width, render->height);

        /* The surface reference is owned by the child_cr created below and put on the cr_stack! */
        render->surfaces_stack = g_list_prepend (render->surfaces_stack, surface);
    }

#if 0
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return;
    }
#endif

    child_cr = cairo_create (surface);
    cairo_surface_destroy (surface);

    render->cr_stack = g_list_prepend (render->cr_stack, render->cr);
    render->cr = child_cr;

    bbox = g_new0 (RsvgBbox, 1);
    *bbox = render->bbox;
    render->bb_stack = g_list_prepend (render->bb_stack, bbox);

    affine = rsvg_state_get_affine (state);
    rsvg_bbox_init (&render->bbox, &affine);
}

void
rsvg_cairo_push_discrete_layer (RsvgDrawingCtx * ctx, gboolean clipping)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);

    if (clipping) {
        return;
    }

    cairo_save (render->cr);
    rsvg_cairo_push_render_stack (ctx);
}

static void
rsvg_cairo_pop_render_stack (RsvgDrawingCtx * ctx)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);
    cairo_t *child_cr = render->cr;
    RsvgNode *lateclip = NULL;
    cairo_surface_t *surface = NULL;
    RsvgState *state = rsvg_drawing_ctx_get_current_state (ctx);
    gboolean nest, needs_destroy = FALSE;

    if (state->clip_path) {
        RsvgNode *node;
        node = rsvg_drawing_ctx_acquire_node_of_type (ctx, state->clip_path, RSVG_NODE_TYPE_CLIP_PATH);
        if (node) {
            if (rsvg_node_clip_path_get_units (node) == objectBoundingBox) {
                lateclip = node;
            } else {
                rsvg_drawing_ctx_release_node (ctx, node);
            }
        }
    }

    if (state->opacity == 0xFF
        && !state->filter && !state->mask && !lateclip && (state->comp_op == CAIRO_OPERATOR_OVER)
        && (state->enable_background == RSVG_ENABLE_BACKGROUND_ACCUMULATE))
        return;

    surface = cairo_get_target (child_cr);

    if (state->filter) {
        RsvgNode *filter;
        cairo_surface_t *output;

        output = render->surfaces_stack->data;
        render->surfaces_stack = g_list_delete_link (render->surfaces_stack, render->surfaces_stack);

        filter = rsvg_drawing_ctx_acquire_node_of_type (ctx, state->filter, RSVG_NODE_TYPE_FILTER);
        if (filter) {
            needs_destroy = TRUE;
            surface = rsvg_filter_render (filter, output, ctx, &render->bbox, "2103");
            rsvg_drawing_ctx_release_node (ctx, filter);

            /* Don't destroy the output surface, it's owned by child_cr */
        }
    }

    render->cr = (cairo_t *) render->cr_stack->data;
    render->cr_stack = g_list_delete_link (render->cr_stack, render->cr_stack);

    nest = render->cr != render->initial_cr;
    cairo_identity_matrix (render->cr);
    cairo_set_source_surface (render->cr, surface,
                              nest ? 0 : render->offset_x,
                              nest ? 0 : render->offset_y);

    if (lateclip) {
        rsvg_cairo_clip (ctx, lateclip, &render->bbox);
        rsvg_drawing_ctx_release_node (ctx, lateclip);
    }

    cairo_set_operator (render->cr, state->comp_op);

    if (state->mask) {
        RsvgNode *mask;

        mask = rsvg_drawing_ctx_acquire_node_of_type (ctx, state->mask, RSVG_NODE_TYPE_MASK);
        if (mask) {
            rsvg_cairo_generate_mask (render->cr, mask, ctx, &render->bbox);
            rsvg_drawing_ctx_release_node (ctx, mask);
        }
    } else if (state->opacity != 0xFF)
        cairo_paint_with_alpha (render->cr, (double) state->opacity / 255.0);
    else
        cairo_paint (render->cr);

    cairo_destroy (child_cr);

    rsvg_bbox_insert ((RsvgBbox *) render->bb_stack->data, &render->bbox);

    render->bbox = *((RsvgBbox *) render->bb_stack->data);

    g_free (render->bb_stack->data);
    render->bb_stack = g_list_delete_link (render->bb_stack, render->bb_stack);

    if (needs_destroy) {
        cairo_surface_destroy (surface);
    }
}

void
rsvg_cairo_pop_discrete_layer (RsvgDrawingCtx * ctx, gboolean clipping)
{
    RsvgCairoRender *render = RSVG_CAIRO_RENDER (ctx->render);

    if (clipping) {
        return;
    }

    rsvg_cairo_pop_render_stack (ctx);
    cairo_restore (render->cr);
}

cairo_surface_t *
rsvg_cairo_get_surface_of_node (RsvgDrawingCtx *ctx,
                                RsvgNode *drawable,
                                double width,
                                double height)
{
    cairo_surface_t *surface;
    cairo_t *cr;

    RsvgCairoRender *save_render = (RsvgCairoRender *) ctx->render;
    RsvgCairoRender *render;

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return NULL;
    }

    cr = cairo_create (surface);

    render = rsvg_cairo_render_new (cr, width, height);
    ctx->render = (RsvgRender *) render;

    rsvg_drawing_ctx_draw_node_from_stack (ctx, drawable, 0, FALSE);

    cairo_destroy (cr);

    rsvg_render_free (ctx->render);
    ctx->render = (RsvgRender *) save_render;

    return surface;
}

cairo_surface_t *
rsvg_cairo_surface_from_pixbuf (const GdkPixbuf *pixbuf)
{
    gint width, height, gdk_rowstride, n_channels, cairo_rowstride;
    guchar *gdk_pixels, *cairo_pixels;
    cairo_format_t format;
    cairo_surface_t *surface;
    int j;

    if (pixbuf == NULL)
        return NULL;

    width = gdk_pixbuf_get_width (pixbuf);
    height = gdk_pixbuf_get_height (pixbuf);
    gdk_pixels = gdk_pixbuf_get_pixels (pixbuf);
    gdk_rowstride = gdk_pixbuf_get_rowstride (pixbuf);
    n_channels = gdk_pixbuf_get_n_channels (pixbuf);

    if (n_channels == 3)
        format = CAIRO_FORMAT_RGB24;
    else
        format = CAIRO_FORMAT_ARGB32;

    surface = cairo_image_surface_create (format, width, height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return NULL;
    }

    cairo_pixels = cairo_image_surface_get_data (surface);
    cairo_rowstride = cairo_image_surface_get_stride (surface);

    if (n_channels == 3) {
        for (j = height; j; j--) {
            guchar *p = gdk_pixels;
            guchar *q = cairo_pixels;
            guchar *end = p + 3 * width;

            while (p < end) {
#if G_BYTE_ORDER == G_LITTLE_ENDIAN
                q[0] = p[2];
                q[1] = p[1];
                q[2] = p[0];
#else
                q[1] = p[0];
                q[2] = p[1];
                q[3] = p[2];
#endif
                p += 3;
                q += 4;
            }

            gdk_pixels += gdk_rowstride;
            cairo_pixels += cairo_rowstride;
        }
    } else {
        for (j = height; j; j--) {
            guchar *p = gdk_pixels;
            guchar *q = cairo_pixels;
            guchar *end = p + 4 * width;
            guint t1, t2, t3;

#define MULT(d,c,a,t) G_STMT_START { t = c * a + 0x7f; d = ((t >> 8) + t) >> 8; } G_STMT_END

            while (p < end) {
#if G_BYTE_ORDER == G_LITTLE_ENDIAN
                MULT (q[0], p[2], p[3], t1);
                MULT (q[1], p[1], p[3], t2);
                MULT (q[2], p[0], p[3], t3);
                q[3] = p[3];
#else
                q[0] = p[3];
                MULT (q[1], p[0], p[3], t1);
                MULT (q[2], p[1], p[3], t2);
                MULT (q[3], p[2], p[3], t3);
#endif

                p += 4;
                q += 4;
            }

#undef MULT
            gdk_pixels += gdk_rowstride;
            cairo_pixels += cairo_rowstride;
        }
    }

    cairo_surface_mark_dirty (surface);
    return surface;
}

/* Copied from gtk+/gdk/gdkpixbuf-drawable.c, LGPL 2+.
 *
 * Copyright (C) 1999 Michael Zucchi
 *
 * Authors: Michael Zucchi <zucchi@zedzone.mmc.com.au>
 *          Cody Russell <bratsche@dfw.net>
 *          Federico Mena-Quintero <federico@gimp.org>
 */

static void
convert_alpha (guchar *dest_data,
               int     dest_stride,
               guchar *src_data,
               int     src_stride,
               int     src_x,
               int     src_y,
               int     width,
               int     height)
{
    int x, y;

    src_data += src_stride * src_y + src_x * 4;

    for (y = 0; y < height; y++) {
        guint32 *src = (guint32 *) src_data;

        for (x = 0; x < width; x++) {
          guint alpha = src[x] >> 24;

          if (alpha == 0) {
              dest_data[x * 4 + 0] = 0;
              dest_data[x * 4 + 1] = 0;
              dest_data[x * 4 + 2] = 0;
          } else {
              dest_data[x * 4 + 0] = (((src[x] & 0xff0000) >> 16) * 255 + alpha / 2) / alpha;
              dest_data[x * 4 + 1] = (((src[x] & 0x00ff00) >>  8) * 255 + alpha / 2) / alpha;
              dest_data[x * 4 + 2] = (((src[x] & 0x0000ff) >>  0) * 255 + alpha / 2) / alpha;
          }
          dest_data[x * 4 + 3] = alpha;
      }

      src_data += src_stride;
      dest_data += dest_stride;
    }
}

static void
convert_no_alpha (guchar *dest_data,
                  int     dest_stride,
                  guchar *src_data,
                  int     src_stride,
                  int     src_x,
                  int     src_y,
                  int     width,
                  int     height)
{
    int x, y;

    src_data += src_stride * src_y + src_x * 4;

    for (y = 0; y < height; y++) {
        guint32 *src = (guint32 *) src_data;

        for (x = 0; x < width; x++) {
            dest_data[x * 3 + 0] = src[x] >> 16;
            dest_data[x * 3 + 1] = src[x] >>  8;
            dest_data[x * 3 + 2] = src[x];
        }

        src_data += src_stride;
        dest_data += dest_stride;
    }
}

GdkPixbuf *
rsvg_cairo_surface_to_pixbuf (cairo_surface_t *surface)
{
    cairo_content_t content;
    GdkPixbuf *dest;
    int width, height;

    /* General sanity checks */
    g_assert (cairo_surface_get_type (surface) == CAIRO_SURFACE_TYPE_IMAGE);

    width = cairo_image_surface_get_width (surface);
    height = cairo_image_surface_get_height (surface);
    if (width == 0 || height == 0)
        return NULL;

    content = cairo_surface_get_content (surface) | CAIRO_CONTENT_COLOR;
    dest = gdk_pixbuf_new (GDK_COLORSPACE_RGB,
                          !!(content & CAIRO_CONTENT_ALPHA),
                          8,
                          width, height);

    if (gdk_pixbuf_get_has_alpha (dest))
      convert_alpha (gdk_pixbuf_get_pixels (dest),
                    gdk_pixbuf_get_rowstride (dest),
                    cairo_image_surface_get_data (surface),
                    cairo_image_surface_get_stride (surface),
                    0, 0,
                    width, height);
    else
      convert_no_alpha (gdk_pixbuf_get_pixels (dest),
                        gdk_pixbuf_get_rowstride (dest),
                        cairo_image_surface_get_data (surface),
                        cairo_image_surface_get_stride (surface),
                        0, 0,
                        width, height);

    return dest;
}
