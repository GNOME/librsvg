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
#include "rsvg-path-builder.h"
#include "rsvg-structure.h"

#include <math.h>
#include <string.h>
#include <pango/pangocairo.h>

typedef struct RsvgCairoClipRender RsvgCairoClipRender;

struct RsvgCairoClipRender {
    RsvgCairoRender super;
    RsvgCairoRender *parent;
};

#define RSVG_CAIRO_CLIP_RENDER(render) (_RSVG_RENDER_CIC ((render), RSVG_RENDER_TYPE_CAIRO_CLIP, RsvgCairoClipRender))

static void
rsvg_cairo_clip_render_pango_layout (RsvgDrawingCtx * ctx, PangoLayout * layout, double x, double y)
{
    RsvgCairoClipRender *render = RSVG_CAIRO_CLIP_RENDER (ctx->render);
    RsvgCairoRender *cairo_render = &render->super;
    cairo_matrix_t affine;
    PangoGravity gravity = pango_context_get_gravity (pango_layout_get_context (layout));
    double rotation;

    affine = rsvg_drawing_ctx_get_current_state_affine (ctx);
    rsvg_cairo_render_set_affine (cairo_render, &affine);

    rotation = pango_gravity_to_rotation (gravity);

    cairo_save (cairo_render->cr);
    cairo_move_to (cairo_render->cr, x, y);
    if (rotation != 0.)
        cairo_rotate (cairo_render->cr, -rotation);

    pango_cairo_update_layout (cairo_render->cr, layout);
    pango_cairo_layout_path (cairo_render->cr, layout);

    cairo_restore (cairo_render->cr);
}

static void
rsvg_cairo_clip_render_path_builder (RsvgDrawingCtx * ctx, RsvgPathBuilder *builder)
{
    rsvg_draw_path_builder (ctx, builder, TRUE);
}

static void
rsvg_cairo_clip_render_surface (RsvgDrawingCtx *ctx,
                                cairo_surface_t *surface,
                                double src_x,
                                double src_y, 
                                double w, 
                                double h)
{
}


static void
rsvg_cairo_clip_render_free (RsvgRender * self)
{
    RsvgCairoClipRender *clip_render = RSVG_CAIRO_CLIP_RENDER (self);

    g_free (clip_render);
}

static void
rsvg_cairo_clip_push_discrete_layer (RsvgDrawingCtx * ctx)
{
}

static void
rsvg_cairo_clip_pop_discrete_layer (RsvgDrawingCtx * ctx)
{
}

static void
rsvg_cairo_clip_add_clipping_rect (RsvgDrawingCtx * ctx, double x, double y, double w, double h)
{
}

static RsvgRender *
rsvg_cairo_clip_render_new (cairo_t *cr, RsvgCairoRender *parent)
{
    RsvgCairoClipRender *clip_render = g_new0 (RsvgCairoClipRender, 1);
    RsvgCairoRender *cairo_render = &clip_render->super;
    RsvgRender *render = &cairo_render->super;

    g_assert (parent->super.type == RSVG_RENDER_TYPE_CAIRO);

    render->type = RSVG_RENDER_TYPE_CAIRO_CLIP;
    render->free = rsvg_cairo_clip_render_free;
    render->get_pango_context = rsvg_cairo_get_pango_context;
    render->render_pango_layout = rsvg_cairo_clip_render_pango_layout;
    render->render_path_builder = rsvg_cairo_clip_render_path_builder;
    render->render_surface = rsvg_cairo_clip_render_surface;
    render->pop_discrete_layer = rsvg_cairo_clip_pop_discrete_layer;
    render->push_discrete_layer = rsvg_cairo_clip_push_discrete_layer;
    render->add_clipping_rect = rsvg_cairo_clip_add_clipping_rect;
    render->get_surface_of_node = NULL;

    cairo_render->initial_cr = parent->initial_cr;
    cairo_render->cr         = cr;
    cairo_render->width      = parent->width;
    cairo_render->height     = parent->height;
    cairo_render->offset_x   = parent->offset_x;
    cairo_render->offset_y   = parent->offset_y;
    cairo_render->cr_stack   = NULL;
    cairo_render->bbox       = parent->bbox;
    cairo_render->bb_stack   = NULL;

    /* We don't copy or ref the following two; we just share them */
#ifdef HAVE_PANGO_FT2
    cairo_render->font_config_for_testing = parent->font_config_for_testing;
    cairo_render->font_map_for_testing    = parent->font_map_for_testing;
#endif

    clip_render->parent = parent;

    return render;
}

void
rsvg_cairo_clip (RsvgDrawingCtx * ctx, RsvgNode *node_clip_path, RsvgBbox * bbox)
{
    RsvgCairoClipRender *clip_render;
    RsvgCairoRender *save = RSVG_CAIRO_RENDER (ctx->render);
    cairo_matrix_t affinesave;
    RsvgState *clip_path_state;
    cairo_t *cr;
    RsvgCoordUnits clip_units;

    g_assert (rsvg_node_get_type (node_clip_path) == RSVG_NODE_TYPE_CLIP_PATH);
    clip_units = rsvg_node_clip_path_get_units (node_clip_path);

    cr = save->cr;
    clip_render = RSVG_CAIRO_CLIP_RENDER (rsvg_cairo_clip_render_new (cr, save));
    ctx->render = &clip_render->super.super;

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
        affinesave = clip_path_state->affine;
        cairo_matrix_multiply (&clip_path_state->affine, &bbtransform, &clip_path_state->affine);
    }

    rsvg_state_push (ctx);
    rsvg_node_draw_children (node_clip_path, ctx, 0);
    rsvg_state_pop (ctx);

    if (clip_units == objectBoundingBox)
        clip_path_state->affine = affinesave;

    g_assert (clip_render->super.cr_stack == NULL);
    g_assert (clip_render->super.bb_stack == NULL);
    g_assert (clip_render->super.surfaces_stack == NULL);

    g_free (ctx->render);
    cairo_clip (cr);
    ctx->render = &save->super;
}
