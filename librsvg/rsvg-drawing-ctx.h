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
    RsvgState *state;
    GError **error;
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
RsvgDrawingCtx *rsvg_drawing_ctx_new (cairo_t *cr, RsvgHandle *handle);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_free (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
cairo_t *rsvg_drawing_ctx_get_cairo_context (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_set_cairo_context (RsvgDrawingCtx *ctx, cairo_t *cr);

G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_is_cairo_context_nested (RsvgDrawingCtx *ctx, cairo_t *cr);

G_GNUC_INTERNAL
RsvgState *rsvg_drawing_ctx_get_current_state   (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_set_current_state         (RsvgDrawingCtx * ctx, RsvgState *state);

/* Implemented in rust/src/drawing_ctx.rs */
G_GNUC_INTERNAL
void       rsvg_drawing_ctx_state_pop           (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void       rsvg_drawing_ctx_state_push          (RsvgDrawingCtx * ctx);

G_GNUC_INTERNAL
RsvgNode *rsvg_drawing_ctx_acquire_node         (RsvgDrawingCtx * ctx, const char *url);
G_GNUC_INTERNAL
RsvgNode *rsvg_drawing_ctx_acquire_node_of_type (RsvgDrawingCtx * ctx, const char *url, RsvgNodeType type);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_release_node              (RsvgDrawingCtx * ctx, RsvgNode *node);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx, RsvgNode *node);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_draw_node_from_stack (RsvgDrawingCtx *ctx,
                                            RsvgNode *node,
                                            int dominate,
                                            gboolean clipping);

G_GNUC_INTERNAL
double rsvg_drawing_ctx_get_width (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
double rsvg_drawing_ctx_get_height (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_offset (RsvgDrawingCtx *draw_ctx, double *x, double *y);

G_GNUC_INTERNAL
RsvgBbox *rsvg_drawing_ctx_get_bbox (RsvgDrawingCtx *draw_ctx);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_insert_bbox (RsvgDrawingCtx *draw_ctx, RsvgBbox *bbox);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_push_view_box (RsvgDrawingCtx * ctx, double w, double h);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_pop_view_box  (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_view_box_size (RsvgDrawingCtx *ctx, double *out_width, double *out_height);

G_GNUC_INTERNAL
void rsvg_drawing_ctx_get_dpi (RsvgDrawingCtx *ctx, double *out_dpi_x, double *out_dpi_y);

G_GNUC_INTERNAL
PangoContext *rsvg_drawing_ctx_get_pango_context (RsvgDrawingCtx *ctx);

G_GNUC_INTERNAL
void         rsvg_drawing_ctx_push_render_stack (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void         rsvg_drawing_ctx_pop_render_stack (RsvgDrawingCtx *ctx);

G_GNUC_INTERNAL
cairo_surface_t *rsvg_drawing_ctx_get_surface_of_node (RsvgDrawingCtx *ctx,
                                                       RsvgNode *drawable,
                                                       double width,
                                                       double height);

G_END_DECLS

#endif /*RSVG_DRAWING_CTX_H */
