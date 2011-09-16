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

#include "config.h"

#include "rsvg-marker.h"
#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-css.h"
#include "rsvg-defs.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-image.h"
#include "rsvg-path.h"

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
    _rsvg_node_init (&marker->super, RSVG_NODE_TYPE_MARKER);
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
    cairo_matrix_t affine, taffine;
    unsigned int i;
    gdouble rotation;
    RsvgState *state = rsvg_current_state (ctx);

    cairo_matrix_init_translate (&taffine, x, y);
    cairo_matrix_multiply (&affine, &taffine, &state->affine);

    if (self->orientAuto)
        rotation = orient;
    else
        rotation = self->orient * M_PI / 180.;

    cairo_matrix_init_rotate (&taffine, rotation);
    cairo_matrix_multiply (&affine, &taffine, &affine);

    if (self->bbox) {
        cairo_matrix_init_scale (&taffine, linewidth, linewidth);
        cairo_matrix_multiply (&affine, &taffine, &affine);
    }

    if (self->vbox.active) {

        double w, h, x, y;
        w = _rsvg_css_normalize_length (&self->width, ctx, 'h');
        h = _rsvg_css_normalize_length (&self->height, ctx, 'v');
        x = 0;
        y = 0;

        rsvg_preserve_aspect_ratio (self->preserve_aspect_ratio,
                                    self->vbox.rect.width,
                                    self->vbox.rect.height,
                                    &w, &h, &x, &y);

        x = -self->vbox.rect.x * w / self->vbox.rect.width;
        y = -self->vbox.rect.y * h / self->vbox.rect.height;

        cairo_matrix_init (&taffine,
                           w / self->vbox.rect.width,
                           0,
                           0,
                           h / self->vbox.rect.height,
                           x,
                           y);
        cairo_matrix_multiply (&affine, &taffine, &affine);
        _rsvg_push_view_box (ctx, self->vbox.rect.width, self->vbox.rect.height);
    }

    cairo_matrix_init_translate (&taffine,
                                 -_rsvg_css_normalize_length (&self->refX, ctx, 'h'),
                                 -_rsvg_css_normalize_length (&self->refY, ctx, 'v'));
    cairo_matrix_multiply (&affine, &taffine, &affine);

    rsvg_state_push (ctx);
    state = rsvg_current_state (ctx);

    rsvg_state_reinit (state);

    rsvg_state_reconstruct (state, &self->super);

    state->affine = affine;

    rsvg_push_discrete_layer (ctx);

    state = rsvg_current_state (ctx);

    if (!state->overflow) {
        if (self->vbox.active)
            rsvg_add_clipping_rect (ctx, self->vbox.rect.x, self->vbox.rect.y,
                                    self->vbox.rect.width, self->vbox.rect.height);
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

        if (val && RSVG_NODE_TYPE (val) == RSVG_NODE_TYPE_MARKER)
            return val;
    }
    return NULL;
}

void
rsvg_render_markers (RsvgDrawingCtx * ctx,
                     const cairo_path_t *path)
{
    double x, y;
    double lastx, lasty;
    double linewidth;
    cairo_path_data_type_t code, nextcode;

    RsvgState *state;
    RsvgMarker *startmarker;
    RsvgMarker *middlemarker;
    RsvgMarker *endmarker;
    cairo_path_data_t *data, *nextdata, *end;
    cairo_path_data_t nextp;

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

    if (path->num_data <= 0)
        return;

    end = &path->data[path->num_data];
    data = &path->data[0];
    nextcode = data[0].header.type;
    if (data[0].header.length > 1)
        nextp = data[data[0].header.length - 1];
    else
        nextp.point.x = nextp.point.y = 0.;

    for ( ; data < end; data = nextdata) {
        lastx = x;
        lasty = y;
        x = nextp.point.x;
        y = nextp.point.y;
        code = nextcode;

        nextdata = data + data->header.length;
        if (nextdata < end) {
            nextcode = nextdata->header.type;
            if (nextdata->header.length > 1) {
                nextp = nextdata[nextdata->header.length - 1];
            } else {
                /* keep nextp unchanged */
            }
        } else {
            nextcode = CAIRO_PATH_MOVE_TO;
        }

        if (nextcode == CAIRO_PATH_MOVE_TO ||
            code == CAIRO_PATH_CLOSE_PATH) {
            if (endmarker) {
                if (code == CAIRO_PATH_CURVE_TO) {
                    rsvg_marker_render (endmarker, x, y,
                                        atan2 (y - data[2].point.y,
                                               x - data[2].point.x),
                                        linewidth, ctx);
                } else {
                    rsvg_marker_render (endmarker, x, y,
                                        atan2 (y - lasty, x - lastx),
                                        linewidth, ctx);
                }
            }
        } else if (code == CAIRO_PATH_MOVE_TO ||
                   code == CAIRO_PATH_CLOSE_PATH) {
            if (startmarker) {
                if (nextcode == CAIRO_PATH_CURVE_TO) {
                    rsvg_marker_render (startmarker, x, y,
                                        atan2 (nextdata[1].point.y - y,
                                               nextdata[1].point.x - x),
                                        linewidth,
                                        ctx);
                } else {
                    rsvg_marker_render (startmarker, x, y,
                                        atan2 (nextp.point.y - y, nextp.point.x - x),
                                        linewidth,
                                        ctx);
                }
            }
        } else {
            if (middlemarker) {
                double xdifin, ydifin, xdifout, ydifout, intot, outtot, angle;

                if (code == CAIRO_PATH_CURVE_TO) {
                    xdifin = x - data[2].point.x;
                    ydifin = y - data[2].point.y;
                } else {
                    xdifin = x - lastx;
                    ydifin = y - lasty;
                }
                if (nextcode == CAIRO_PATH_CURVE_TO) {
                    xdifout = nextdata[1].point.x - x;
                    ydifout = nextdata[1].point.y - y;
                } else {
                    xdifout = nextp.point.x - x;
                    ydifout = nextp.point.y - y;
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
