/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-shapes.c: Draw shapes with libart

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/
#ifndef RSVG_DRAWING_CTX_H
#define RSVG_DRAWING_CTX_H

#include "rsvg-private.h"

G_BEGIN_DECLS

/* Contextual information for the drawing phase */
struct RsvgDrawingCtx {
    cairo_t *cr;
    cairo_t *initial_cr;
    GList *cr_stack;
    GList *surfaces_stack;
    RsvgDefs *defs;
    double dpi_x, dpi_y;
    cairo_rectangle_t rect;
    RsvgViewBox vb;
    GSList *vb_stack;
    GSList *drawsub_stack;
    GSList *acquired_nodes;
    gboolean is_testing;
    RsvgBbox *bbox;
    GList *bb_stack;
};

G_GNUC_INTERNAL
RsvgDrawingCtx *rsvg_drawing_ctx_new (cairo_t *cr,
                                      guint width,
                                      guint height,
                                      double vb_width,
                                      double vb_height,
                                      double dpi_x,
                                      double dpi_y,
                                      RsvgDefs *defs,
                                      gboolean testing);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_free (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
cairo_t *rsvg_drawing_ctx_get_cairo_context (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_set_cairo_context (RsvgDrawingCtx *ctx, cairo_t *cr);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_push_cr (RsvgDrawingCtx *ctx, cairo_t *cr);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_pop_cr (RsvgDrawingCtx *ctx);

G_GNUC_INTERNAL
void rsvg_cairo_generate_mask (cairo_t * cr, RsvgNode *mask, RsvgDrawingCtx *ctx);

G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_is_cairo_context_nested (RsvgDrawingCtx *ctx, cairo_t *cr);

G_GNUC_INTERNAL
RsvgNode *rsvg_drawing_ctx_acquire_node         (RsvgDrawingCtx * ctx, const char *url);
G_GNUC_INTERNAL
RsvgNode *rsvg_drawing_ctx_acquire_node_of_type (RsvgDrawingCtx * ctx, const char *url, RsvgNodeType type);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_release_node              (RsvgDrawingCtx * ctx, RsvgNode *node);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx, RsvgNode *node);

G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_should_draw_node_from_stack (RsvgDrawingCtx *ctx,
                                                       RsvgNode *node,
                                                       GSList **out_stacksave);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_restore_stack (RsvgDrawingCtx *ctx,
                                     GSList *stacksave);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
void rsvg_drawing_ctx_draw_node_from_stack (RsvgDrawingCtx *ctx,
                                            RsvgNode *node,
                                            RsvgNode *cascade_from_node,
                                            gboolean clipping);

G_GNUC_INTERNAL
double rsvg_drawing_ctx_get_width (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
double rsvg_drawing_ctx_get_height (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_raw_offset (RsvgDrawingCtx *draw_ctx, double *x, double *y);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_offset (RsvgDrawingCtx *draw_ctx, double *x, double *y);

G_GNUC_INTERNAL
RsvgBbox *rsvg_drawing_ctx_get_bbox (RsvgDrawingCtx *draw_ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_push_bounding_box (RsvgDrawingCtx *draw_ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_pop_bounding_box (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_push_view_box (RsvgDrawingCtx * ctx, double w, double h);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_pop_view_box  (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_view_box_size (RsvgDrawingCtx *ctx, double *out_width, double *out_height);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_push_surface (RsvgDrawingCtx *draw_ctx, cairo_surface_t *surface);
G_GNUC_INTERNAL
cairo_surface_t *rsvg_drawing_ctx_pop_surface (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_dpi (RsvgDrawingCtx *ctx, double *out_dpi_x, double *out_dpi_y);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_draw_node_on_surface (RsvgDrawingCtx *ctx,
                                            RsvgNode *node,
                                            RsvgNode *cascade_from,
                                            cairo_surface_t *surface,
                                            double width,
                                            double height);

G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_is_testing (RsvgDrawingCtx *ctx);

G_GNUC_INTERNAL
GList *rsvg_drawing_ctx_get_cr_stack (RsvgDrawingCtx *ctx);

G_END_DECLS

#endif /*RSVG_DRAWING_CTX_H */
