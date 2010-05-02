/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-marker.c: Marker loading and rendering

   Copyright (C) 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "rsvg-marker.h"
#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-css.h"
#include "rsvg-defs.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-image.h"

#include <string.h>
#include <math.h>
#include <errno.h>

static void
rsvg_node_marker_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgMarker *marker;
    marker = (RsvgMarker *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &marker->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
            marker->vbox = rsvg_css_parse_vbox (value);
        if ((value = rsvg_property_bag_lookup (atts, "refX")))
            marker->refX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "refY")))
            marker->refY = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "markerWidth")))
            marker->width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "markerHeight")))
            marker->height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "orient"))) {
            if (!strcmp (value, "auto"))
                marker->orientAuto = TRUE;
            else
                marker->orient = rsvg_css_parse_angle (value);
        }
        if ((value = rsvg_property_bag_lookup (atts, "markerUnits"))) {
            if (!strcmp (value, "userSpaceOnUse"))
                marker->bbox = FALSE;
            if (!strcmp (value, "strokeWidth"))
                marker->bbox = TRUE;
        }
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            marker->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
        rsvg_parse_style_attrs (ctx, self->state, "marker", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_marker (void)
{
    RsvgMarker *marker;
    marker = g_new (RsvgMarker, 1);
    _rsvg_node_init (&marker->super);
    marker->orient = 0;
    marker->orientAuto = FALSE;
    marker->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    marker->refX = marker->refY = _rsvg_css_parse_length ("0");
    marker->width = marker->height = _rsvg_css_parse_length ("1");
    marker->bbox = TRUE;
    marker->vbox.active = FALSE;
    marker->super.set_atts = rsvg_node_marker_set_atts;
    return &marker->super;
}

void
rsvg_marker_render (RsvgMarker * self, gdouble x, gdouble y, gdouble orient, gdouble linewidth,
		    RsvgDrawingCtx * ctx)
{
    gdouble affine[6];
    gdouble taffine[6];
    unsigned int i;
    gdouble rotation;
    RsvgState *state = rsvg_current_state (ctx);

    _rsvg_affine_translate (taffine, x, y);
    _rsvg_affine_multiply (affine, taffine, state->affine);

    if (self->orientAuto)
        rotation = orient * 180. / M_PI;
    else
        rotation = self->orient;
    _rsvg_affine_rotate (taffine, rotation);
    _rsvg_affine_multiply (affine, taffine, affine);

    if (self->bbox) {
        _rsvg_affine_scale (taffine, linewidth, linewidth);
        _rsvg_affine_multiply (affine, taffine, affine);
    }

    if (self->vbox.active) {

        double w, h, x, y;
        w = _rsvg_css_normalize_length (&self->width, ctx, 'h');
        h = _rsvg_css_normalize_length (&self->height, ctx, 'v');
        x = 0;
        y = 0;

        rsvg_preserve_aspect_ratio (self->preserve_aspect_ratio,
                                    self->vbox.w, self->vbox.h, &w, &h, &x, &y);

        x = -self->vbox.x * w / self->vbox.w;
        y = -self->vbox.y * h / self->vbox.h;

        taffine[0] = w / self->vbox.w;
        taffine[1] = 0.;
        taffine[2] = 0.;
        taffine[3] = h / self->vbox.h;
        taffine[4] = x;
        taffine[5] = y;
        _rsvg_affine_multiply (affine, taffine, affine);
        _rsvg_push_view_box (ctx, self->vbox.w, self->vbox.h);
    }
    _rsvg_affine_translate (taffine,
                            -_rsvg_css_normalize_length (&self->refX, ctx, 'h'),
                            -_rsvg_css_normalize_length (&self->refY, ctx, 'v'));
    _rsvg_affine_multiply (affine, taffine, affine);


    rsvg_state_push (ctx);
    state = rsvg_current_state (ctx);

    rsvg_state_reinit (state);

    rsvg_state_reconstruct (state, &self->super);

    for (i = 0; i < 6; i++)
        state->affine[i] = affine[i];

    rsvg_push_discrete_layer (ctx);

    state = rsvg_current_state (ctx);

    if (!state->overflow) {
        if (self->vbox.active)
            rsvg_add_clipping_rect (ctx, self->vbox.x, self->vbox.y, self->vbox.w, self->vbox.h);
        else
            rsvg_add_clipping_rect (ctx, 0, 0,
                                    _rsvg_css_normalize_length (&self->width, ctx, 'h'),
                                    _rsvg_css_normalize_length (&self->height, ctx, 'v'));
    }

    for (i = 0; i < self->super.children->len; i++) {
        rsvg_state_push (ctx);

        rsvg_node_draw (g_ptr_array_index (self->super.children, i), ctx, 0);

        rsvg_state_pop (ctx);
    }
    rsvg_pop_discrete_layer (ctx);

    rsvg_state_pop (ctx);
    if (self->vbox.active)
        _rsvg_pop_view_box (ctx);
}

RsvgNode *
rsvg_marker_parse (const RsvgDefs * defs, const char *str)
{
    char *name;

    name = rsvg_get_url_string (str);
    if (name) {
        RsvgNode *val;
        val = rsvg_defs_lookup (defs, name);
        g_free (name);

        if (val && (!strcmp (val->type->str, "marker")))
            return val;
    }
    return NULL;
}

void
rsvg_render_markers (const RsvgBpathDef * bpath_def, RsvgDrawingCtx * ctx)
{
    int i;

    double x, y;
    double lastx, lasty;
    double nextx, nexty;
    double linewidth;
    RsvgPathcode code, lastcode, nextcode;

    RsvgState *state;
    RsvgMarker *startmarker;
    RsvgMarker *middlemarker;
    RsvgMarker *endmarker;

    state = rsvg_current_state (ctx);

    linewidth = _rsvg_css_normalize_length (&state->stroke_width, ctx, 'o');
    startmarker = (RsvgMarker *) state->startMarker;
    middlemarker = (RsvgMarker *) state->middleMarker;
    endmarker = (RsvgMarker *) state->endMarker;

    if (linewidth == 0)
        return;

    if (!startmarker && !middlemarker && !endmarker)
        return;

    x = 0;
    y = 0;
    code = RSVG_END;
    nextx = bpath_def->bpath[0].x3;
    nexty = bpath_def->bpath[0].y3;
    nextcode = bpath_def->bpath[0].code;

    for (i = 0; i < bpath_def->n_bpath - 1; i++) {
        lastx = x;
        lasty = y;
        lastcode = code;
        x = nextx;
        y = nexty;
        code = nextcode;
        nextx = bpath_def->bpath[i + 1].x3;
        nexty = bpath_def->bpath[i + 1].y3;
        nextcode = bpath_def->bpath[i + 1].code;

        if (nextcode == RSVG_MOVETO ||
            nextcode == RSVG_MOVETO_OPEN ||
            nextcode == RSVG_END) {
            if (endmarker) {
                if (code == RSVG_CURVETO) {
                    rsvg_marker_render (endmarker, x, y,
                                        atan2 (y - bpath_def->bpath[i].y2,
                                               x - bpath_def->bpath[i].x2),
                                        linewidth, ctx);
                } else {
                    rsvg_marker_render (endmarker, x, y,
                                        atan2 (y - lasty, x - lastx),
                                        linewidth, ctx);
                }
            }
        } else if (code == RSVG_MOVETO ||
                   code == RSVG_MOVETO_OPEN) {
            if (startmarker) {
                if (nextcode == RSVG_CURVETO) {
                    rsvg_marker_render (startmarker, x, y,
                                        atan2 (bpath_def->bpath[i + 1].y1 - y,
                                               bpath_def->bpath[i + 1].x1 - x),
                                        linewidth,
                                        ctx);
                } else {
                    rsvg_marker_render (startmarker, x, y,
                                        atan2 (nexty - y, nextx - x),
                                        linewidth,
					                    ctx);
                }
            }
        } else {
            if (middlemarker) {
                double xdifin, ydifin, xdifout, ydifout, intot, outtot, angle;

                if (code == RSVG_CURVETO) {
                    xdifin = x - bpath_def->bpath[i].x2;
                    ydifin = y - bpath_def->bpath[i].y2;
                } else {
                    xdifin = x - lastx;
                    ydifin = y - lasty;
                }
                if (nextcode == RSVG_CURVETO) {
                    xdifout = bpath_def->bpath[i+1].x1 - x;
                    ydifout = bpath_def->bpath[i+1].y1 - y;
                } else {
                    xdifout = nextx - x;
                    ydifout = nexty - y;
                }

                intot = sqrt (xdifin * xdifin + ydifin * ydifin);
                outtot = sqrt (xdifout * xdifout + ydifout * ydifout);

                xdifin /= intot;
                ydifin /= intot;
                xdifout /= outtot;
                ydifout /= outtot;

                angle = atan2 ((ydifin + ydifout) / 2, (xdifin + xdifout) / 2);
                rsvg_marker_render (middlemarker, x, y, angle, linewidth, ctx);
            }
        }
    }
}
