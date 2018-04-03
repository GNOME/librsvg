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
#include "rsvg-cairo-clip.h"
#include "rsvg-cairo-render.h"
#include "rsvg-styles.h"
#include "rsvg-structure.h"

#include <math.h>
#include <string.h>
#include <pango/pangocairo.h>

void
rsvg_cairo_clip (RsvgDrawingCtx * ctx, RsvgNode *node_clip_path, RsvgBbox * bbox)
{
    RsvgCairoRender *save = RSVG_CAIRO_RENDER (ctx->render);
    cairo_matrix_t affinesave;
    RsvgState *clip_path_state;
    cairo_t *cr;
    RsvgCoordUnits clip_units;
    GList *orig_cr_stack;
    GList *orig_bb_stack;
    GList *orig_surfaces_stack;
    RsvgBbox orig_bbox;

    g_assert (rsvg_node_get_type (node_clip_path) == RSVG_NODE_TYPE_CLIP_PATH);
    clip_units = rsvg_node_clip_path_get_units (node_clip_path);

    cr = save->cr;

    clip_path_state = rsvg_node_get_state (node_clip_path);

    /* Horribly dirty hack to have the bbox premultiplied to everything */
    if (clip_units == objectBoundingBox) {
        cairo_matrix_t bbtransform;
        cairo_matrix_init (&bbtransform,
                           bbox->rect.width,
                           0,
                           0,
                           bbox->rect.height,
                           bbox->rect.x,
                           bbox->rect.y);
        affinesave = rsvg_state_get_affine (clip_path_state);
        cairo_matrix_multiply (&bbtransform, &bbtransform, &affinesave);
        rsvg_state_set_affine (clip_path_state, bbtransform);
    }

    orig_cr_stack = save->cr_stack;
    orig_bb_stack = save->bb_stack;
    orig_surfaces_stack = save->surfaces_stack;

    orig_bbox = save->bbox;

    rsvg_drawing_ctx_state_push (ctx);
    rsvg_node_draw_children (node_clip_path, ctx, 0, TRUE);
    rsvg_drawing_ctx_state_pop (ctx);

    if (clip_units == objectBoundingBox) {
        rsvg_state_set_affine (clip_path_state, affinesave);
    }

    g_assert (save->cr_stack == orig_cr_stack);
    g_assert (save->bb_stack == orig_bb_stack);
    g_assert (save->surfaces_stack == orig_surfaces_stack);

    /* FIXME: this is an EPIC HACK to keep the clipping context from
     * accumulating bounding boxes.  We'll remove this later, when we
     * are able to extract bounding boxes from outside the
     * general drawing loop.
     */
    save->bbox = orig_bbox;

    cairo_clip (cr);
}
