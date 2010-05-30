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
#include "rsvg-bpath-util.h"
#include "rsvg-path.h"

#include <math.h>
#include <string.h>

typedef struct RsvgCairoClipRender RsvgCairoClipRender;

struct RsvgCairoClipRender {
    RsvgRender super;
    cairo_t *cr;
    RsvgCairoRender *parent;
};

static void
rsvg_cairo_clip_apply_affine (RsvgCairoClipRender *render, const double affine[6])
{
    cairo_matrix_t matrix;
    gboolean nest = render->cr != render->parent->initial_cr;

    cairo_matrix_init (&matrix,
                       affine[0], affine[1],
                       affine[2], affine[3],
                       affine[4] + (nest ? 0 : render->parent->offset_x),
                       affine[5] + (nest ? 0 : render->parent->offset_y));
    cairo_set_matrix (render->cr, &matrix);
}

static void
rsvg_cairo_clip_render_path (RsvgDrawingCtx * ctx, const RsvgBpathDef * bpath_def)
{
    RsvgCairoClipRender *render = (RsvgCairoClipRender *) ctx->render;
    RsvgState *state = rsvg_current_state (ctx);
    cairo_t *cr;
    RsvgBpath *bpath;
    int i;

    cr = render->cr;

    rsvg_cairo_clip_apply_affine (render, state->affine);

    if (rsvg_current_state (ctx)->clip_rule == FILL_RULE_EVENODD)
        cairo_set_fill_rule (((RsvgCairoRender *) ctx->render)->cr, CAIRO_FILL_RULE_EVEN_ODD);
    else                        /* state->fill_rule == FILL_RULE_NONZERO */
        cairo_set_fill_rule (((RsvgCairoRender *) ctx->render)->cr, CAIRO_FILL_RULE_WINDING);

    for (i = 0; i < bpath_def->n_bpath; i++) {
        bpath = &bpath_def->bpath[i];

        switch (bpath->code) {
        case RSVG_MOVETO:
            cairo_close_path (cr);
            /* fall-through */
        case RSVG_MOVETO_OPEN:
            cairo_move_to (cr, bpath->x3, bpath->y3);
            break;
        case RSVG_CURVETO:
            cairo_curve_to (cr, bpath->x1, bpath->y1, bpath->x2, bpath->y2, bpath->x3, bpath->y3);
            break;
        case RSVG_LINETO:
            cairo_line_to (cr, bpath->x3, bpath->y3);
            break;
        case RSVG_END:
            break;
        }
    }
}

static void
rsvg_cairo_clip_render_image (RsvgDrawingCtx * ctx,
                              const GdkPixbuf * pixbuf,
                              double pixbuf_x, double pixbuf_y, double w, double h)
{
}


static void
rsvg_cairo_clip_render_free (RsvgRender * self)
{
    RsvgCairoClipRender *me = (RsvgCairoClipRender *) self;
    g_free (me);
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
    RsvgCairoClipRender *cairo_render = g_new0 (RsvgCairoClipRender, 1);

    cairo_render->super.free = rsvg_cairo_clip_render_free;
    cairo_render->super.create_pango_context = rsvg_cairo_create_pango_context;
    cairo_render->super.render_pango_layout = rsvg_cairo_render_pango_layout;
    cairo_render->super.render_image = rsvg_cairo_clip_render_image;
    cairo_render->super.render_path = rsvg_cairo_clip_render_path;
    cairo_render->super.pop_discrete_layer = rsvg_cairo_clip_pop_discrete_layer;
    cairo_render->super.push_discrete_layer = rsvg_cairo_clip_push_discrete_layer;
    cairo_render->super.add_clipping_rect = rsvg_cairo_clip_add_clipping_rect;
    cairo_render->super.get_image_of_node = NULL;
    cairo_render->cr = cr;
    cairo_render->parent = parent;

    return &cairo_render->super;
}

void
rsvg_cairo_clip (RsvgDrawingCtx * ctx, RsvgClipPath * clip, RsvgBbox * bbox)
{
    RsvgCairoRender *save = (RsvgCairoRender *) ctx->render;
    double affinesave[6];
    int i;
    ctx->render = rsvg_cairo_clip_render_new (save->cr, save);

    /* Horribly dirty hack to have the bbox premultiplied to everything */
    if (clip->units == objectBoundingBox) {
        double bbtransform[6];
        bbtransform[0] = bbox->w;
        bbtransform[1] = 0.;
        bbtransform[2] = 0.;
        bbtransform[3] = bbox->h;
        bbtransform[4] = bbox->x;
        bbtransform[5] = bbox->y;
        for (i = 0; i < 6; i++)
            affinesave[i] = clip->super.state->affine[i];
        _rsvg_affine_multiply (clip->super.state->affine, bbtransform, clip->super.state->affine);
    }

    rsvg_state_push (ctx);
    _rsvg_node_draw_children ((RsvgNode *) clip, ctx, 0);
    rsvg_state_pop (ctx);

    if (clip->units == objectBoundingBox)
        for (i = 0; i < 6; i++)
            clip->super.state->affine[i] = affinesave[i];

    g_free (ctx->render);
    cairo_clip (save->cr);
    ctx->render = &save->super;
}
