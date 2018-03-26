/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-text.c: Text handling routines for RSVG

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

   Author: Raph Levien <raph@artofcode.com>
*/

#include <string.h>

#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-css.h"
#include "rsvg-shapes.h"

/* Implemented in rust/src/text.rs */
extern gboolean rsvg_node_tref_measure (RsvgNode *node, RsvgDrawingCtx *ctx, double *length);
extern void rsvg_node_tref_render (RsvgNode *node, RsvgDrawingCtx * ctx, double *x, double *y);
extern gboolean rsvg_node_tspan_measure (RsvgNode *node, RsvgDrawingCtx *ctx, double *length, gboolean usetextonly);
extern void rsvg_node_tspan_render (RsvgNode *node, RsvgDrawingCtx * ctx, double *x, double *y, gboolean usetextonly);
extern double rsvg_node_chars_measure (RsvgNode *node, RsvgDrawingCtx *ctx);
extern void rsvg_node_chars_render (RsvgNode *node, RsvgDrawingCtx * ctx, double *x, double *y);

void
rsvg_text_render_children (RsvgNode       *self,
                           RsvgDrawingCtx *ctx,
                           gdouble        *x,
                           gdouble        *y,
                           gboolean        usetextonly);

static void
draw_text_child (RsvgNode       *node,
                 RsvgDrawingCtx *ctx,
                 gdouble        *x,
                 gdouble        *y,
                 gboolean        usetextonly)
{
    RsvgNodeType type = rsvg_node_get_type (node);

    if (type == RSVG_NODE_TYPE_CHARS) {
        rsvg_node_chars_render (node, ctx, x, y);
    } else {
        if (usetextonly) {
            rsvg_text_render_children (node, ctx, x, y, usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                rsvg_node_tspan_render (node, ctx, x, y, usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                rsvg_node_tref_render (node, ctx, x, y);
            }
        }
    }
}

/* This function is responsible of selecting render for a text element including its children and giving it the drawing context */
void
rsvg_text_render_children (RsvgNode       *self,
                           RsvgDrawingCtx *ctx,
                           gdouble        *x,
                           gdouble        *y,
                           gboolean        usetextonly)
{
    RsvgNodeChildrenIter *iter;
    RsvgNode *child;

    rsvg_push_discrete_layer (ctx);

    iter = rsvg_node_children_iter_begin (self);

    while (rsvg_node_children_iter_next (iter, &child)) {
        draw_text_child (child, ctx, x, y, usetextonly);
        child = rsvg_node_unref (child);
    }

    rsvg_node_children_iter_end (iter);

    rsvg_pop_discrete_layer (ctx);
}

gboolean
rsvg_text_measure_children (RsvgNode       *self,
                            RsvgDrawingCtx *ctx,
                            gdouble        *length,
                            gboolean        usetextonly);

static gboolean
compute_child_length (RsvgNode       *node,
                      RsvgDrawingCtx *ctx,
                      gdouble        *length,
                      gboolean        usetextonly)
{
    RsvgNodeType type = rsvg_node_get_type (node);
    gboolean done;

    done = FALSE;

    rsvg_state_push (ctx);
    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), 0);

    if (type == RSVG_NODE_TYPE_CHARS) {
        *length += rsvg_node_chars_measure (node, ctx);
    } else {
        if (usetextonly) {
            done = rsvg_text_measure_children (node, ctx, length, usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                done = rsvg_node_tspan_measure (node, ctx, length, usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                done = rsvg_node_tref_measure (node, ctx, length);
            }
        }
    }

    rsvg_state_pop (ctx);

    return done;
}

gboolean
rsvg_text_measure_children (RsvgNode       *self,
                            RsvgDrawingCtx *ctx,
                            gdouble        *length,
                            gboolean        usetextonly)
{
    RsvgNodeChildrenIter *iter;
    RsvgNode *child;
    gboolean done = FALSE;

    iter = rsvg_node_children_iter_begin (self);

    while (rsvg_node_children_iter_next (iter, &child)) {
        done = compute_child_length (child, ctx, length, usetextonly);
        child = rsvg_node_unref (child);

        if (done) {
            break;
        }
    }

    rsvg_node_children_iter_end (iter);

    return done;
}
