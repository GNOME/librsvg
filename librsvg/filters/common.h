/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.h : Provides filters

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

   Author: Caleb Moore <calebmm@tpg.com.au>
*/

#ifndef RSVG_FILTERS_COMMON_H
#define RSVG_FILTERS_COMMON_H

#include "../rsvg-private.h"
#include "../rsvg-filter.h"

G_BEGIN_DECLS 

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

G_GNUC_INTERNAL
cairo_surface_t *_rsvg_image_surface_new (int width, int height);

G_GNUC_INTERNAL
void filter_primitive_set_x_y_width_height_atts (RsvgFilterPrimitive *prim, RsvgPropertyBag *atts);

G_GNUC_INTERNAL
guchar get_interp_pixel (guchar * src, gdouble ox, gdouble oy, guchar ch, RsvgIRect boundarys,
                         guint rowstride);

G_GNUC_INTERNAL
void render_child_if_filter_primitive (RsvgNode *node, RsvgFilterContext *filter_ctx);

G_GNUC_INTERNAL
void rsvg_alpha_blt (cairo_surface_t *src,
                     gint srcx,
                     gint srcy,
                     gint srcwidth,
                     gint srcheight,
                     cairo_surface_t *dst,
                     gint dstx,
                     gint dsty);

G_GNUC_INTERNAL
gboolean rsvg_art_affine_image (cairo_surface_t *img,
                                cairo_surface_t *intermediate,
                                cairo_matrix_t *affine,
                                double w,
                                double h);

G_GNUC_INTERNAL
void rsvg_filter_context_free (RsvgFilterContext * ctx);

G_GNUC_INTERNAL
void rsvg_filter_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate, gboolean clipping);

G_GNUC_INTERNAL
void rsvg_filter_fix_coordinate_system (RsvgFilterContext * ctx, RsvgState * state, RsvgBbox *bbox);

G_GNUC_INTERNAL
void rsvg_filter_free (gpointer impl);

G_GNUC_INTERNAL
void rsvg_filter_free_pair (gpointer value);

G_GNUC_INTERNAL
cairo_surface_t *rsvg_filter_get_in (GString * name, RsvgFilterContext * ctx);

G_GNUC_INTERNAL
RsvgFilterPrimitiveOutput rsvg_filter_get_result (GString * name, RsvgFilterContext * ctx);

G_GNUC_INTERNAL
void rsvg_filter_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts);

G_GNUC_INTERNAL
void rsvg_filter_store_output (GString * name, RsvgFilterPrimitiveOutput result, RsvgFilterContext * ctx);

G_GNUC_INTERNAL
void rsvg_filter_store_result (GString * name,
                               cairo_surface_t *surface,
                               RsvgFilterContext * ctx);

G_GNUC_INTERNAL
void rsvg_filter_primitive_free (gpointer impl);

G_GNUC_INTERNAL
RsvgIRect rsvg_filter_primitive_get_bounds (RsvgFilterPrimitive * self, RsvgFilterContext * ctx);

G_END_DECLS

#endif
