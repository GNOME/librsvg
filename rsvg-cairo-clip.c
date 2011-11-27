/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-clip.h"
#include "rsvg-cairo-render.h"
#include "rsvg-styles.h"
#include "rsvg-path.h"

#include <math.h>
#include <string.h>

typedef struct RsvgCairoClipRender RsvgCairoClipRender;

struct RsvgCairoClipRender {
    RsvgCairoRender super;
    RsvgCairoRender *parent;
};

#define RSVG_CAIRO_CLIP_RENDER(render) (_RSVG_RENDER_CIC ((render), RSVG_RENDER_TYPE_CAIRO_CLIP, RsvgCairoClipRender))

static void
rsvg_cairo_clip_apply_affine (RsvgCairoClipRender *render, cairo_matrix_t *affine)
{
    RsvgCairoRender *cairo_render = &render->super;
    cairo_matrix_t matrix;
    gboolean nest = cairo_render->cr != cairo_render->initial_cr;

    cairo_matrix_init (&matrix,
                       affine->xx, affine->yx,
                       affine->xy, affine->yy,
                       affine->x0 + (nest ? 0 : render->parent->offset_x),
                       affine->y0 + (nest ? 0 : render->parent->offset_y));
    cairo_set_matrix (cairo_render->cr, &matrix);
}

static void
rsvg_cairo_clip_render_path (RsvgDrawingCtx * ctx, const cairo_path_t *path)
{
    RsvgCairoClipRender *render = RSVG_CAIRO_CLIP_RENDER (ctx->render);
    RsvgCairoRender *cairo_render = &render->super;
    RsvgState *state = rsvg_current_state (ctx);
    cairo_t *cr;

    cr = cairo_render->cr;

    rsvg_cairo_clip_apply_affine (render, &state->affine);

    cairo_set_fill_rule (cr, rsvg_current_state (ctx)->clip_rule);

    cairo_append_path (cr, path);
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
rsvg_cairo_clip_render_new (cairo_t * cr, RsvgCairoRender *parent)
{
    RsvgCairoClipRender *clip_render = g_new0 (RsvgCairoClipRender, 1);
    RsvgCairoRender *cairo_render = &clip_render->super;
    RsvgRender *render = &cairo_render->super;

    g_assert (parent->super.type == RSVG_RENDER_TYPE_CAIRO);

    render->type = RSVG_RENDER_TYPE_CAIRO_CLIP;
    render->free = rsvg_cairo_clip_render_free;
    render->create_pango_context = rsvg_cairo_create_pango_context;
    render->render_pango_layout = rsvg_cairo_render_pango_layout;
    render->render_surface = rsvg_cairo_clip_render_surface;
    render->render_path = rsvg_cairo_clip_render_path;
    render->pop_discrete_layer = rsvg_cairo_clip_pop_discrete_layer;
    render->push_discrete_layer = rsvg_cairo_clip_push_discrete_layer;
    render->add_clipping_rect = rsvg_cairo_clip_add_clipping_rect;
    render->get_surface_of_node = NULL;
    cairo_render->initial_cr = parent->cr;
    cairo_render->cr = cr;
    clip_render->parent = parent;

    return render;
}

void
rsvg_cairo_clip (RsvgDrawingCtx * ctx, RsvgClipPath * clip, RsvgBbox * bbox)
{
    RsvgCairoRender *save = RSVG_CAIRO_RENDER (ctx->render);
    cairo_matrix_t affinesave;

    ctx->render = rsvg_cairo_clip_render_new (save->cr, save);

    /* Horribly dirty hack to have the bbox premultiplied to everything */
    if (clip->units == objectBoundingBox) {
        cairo_matrix_t bbtransform;
        cairo_matrix_init (&bbtransform,
                           bbox->rect.width,
                           0,
                           0,
                           bbox->rect.height,
                           bbox->rect.x,
                           bbox->rect.y);
        affinesave = clip->super.state->affine;
        cairo_matrix_multiply (&clip->super.state->affine, &bbtransform, &clip->super.state->affine);
    }

    rsvg_state_push (ctx);
    _rsvg_node_draw_children ((RsvgNode *) clip, ctx, 0);
    rsvg_state_pop (ctx);

    if (clip->units == objectBoundingBox)
        clip->super.state->affine = affinesave;

    g_free (ctx->render);
    cairo_clip (save->cr);
    ctx->render = &save->super;
}
