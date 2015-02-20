/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-shapes.c: Draw SVG shapes

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
#include <string.h>
#include <math.h>
#include <errno.h>
#include <stdio.h>

#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-css.h"
#include "rsvg-defs.h"
#include "rsvg-path.h"

/* 4/3 * (1-cos 45)/sin 45 = 4/3 * sqrt(2) - 1 */
#define RSVG_ARC_MAGIC ((double) 0.5522847498)

static void
rsvg_node_path_free (RsvgNode * self)
{
    RsvgNodePath *path = (RsvgNodePath *) self;
    if (path->path)
        rsvg_cairo_path_destroy (path->path);
    _rsvg_node_finalize (&path->super);
    g_free (path);
}

static void
rsvg_node_path_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodePath *path = (RsvgNodePath *) self;

    if (!path->path)
        return;

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    rsvg_render_path (ctx, path->path);
}

static void
rsvg_node_path_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodePath *path = (RsvgNodePath *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "d"))) {
            if (path->path)
                rsvg_cairo_path_destroy (path->path);
            path->path = rsvg_parse_path (value);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "path", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_path (void)
{
    RsvgNodePath *path;
    path = g_new (RsvgNodePath, 1);
    _rsvg_node_init (&path->super, RSVG_NODE_TYPE_PATH);
    path->path = NULL;
    path->super.free = rsvg_node_path_free;
    path->super.draw = rsvg_node_path_draw;
    path->super.set_atts = rsvg_node_path_set_atts;

    return &path->super;
}

struct _RsvgNodePoly {
    RsvgNode super;
    cairo_path_t *path;
};

typedef struct _RsvgNodePoly RsvgNodePoly;

static cairo_path_t *
_rsvg_node_poly_build_path (const char *value,
                            gboolean close_path);

static void
_rsvg_node_poly_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgNodePoly *poly = (RsvgNodePoly *) self;
    const char *klazz = NULL, *id = NULL, *value;

    if (rsvg_property_bag_size (atts)) {
        /* support for svg < 1.0 which used verts */
        if ((value = rsvg_property_bag_lookup (atts, "verts"))
            || (value = rsvg_property_bag_lookup (atts, "points"))) {
            if (poly->path)
                rsvg_cairo_path_destroy (poly->path);
            poly->path = _rsvg_node_poly_build_path (value,
                                                     RSVG_NODE_TYPE (self) == RSVG_NODE_TYPE_POLYGON);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state,
                                RSVG_NODE_TYPE (self) == RSVG_NODE_TYPE_POLYLINE ? "polyline" : "polygon",
                                klazz, id, atts);
    }

}

static cairo_path_t *
_rsvg_node_poly_build_path (const char *value,
                            gboolean close_path)
{
    double *pointlist;
    guint pointlist_len, i;
    RsvgPathBuilder builder;
    cairo_path_t *path;

    pointlist = rsvg_css_parse_number_list (value, &pointlist_len);
    if (pointlist == NULL)
        return NULL;

    if (pointlist_len < 2) {
        g_free (pointlist);
        return NULL;
    }

    /* Calculate the number of cairo_path_data_t we'll need:
     *
     *     pointlist_len / 2 -> number of commands
     *     pointlist_len / 2 -> number of points
     * +   1                 -> closepath
     * ---------------------------------------------
     *     pointlist_len + 1 -> total
     */
    rsvg_path_builder_init (&builder, pointlist_len + 1);

    rsvg_path_builder_move_to (&builder, pointlist[0], pointlist[1]);

    for (i = 2; i < pointlist_len; i += 2) {
        double x, y;

        x = pointlist[i];

        /* We expect points to come in coordinate pairs.  But if there is a
         * missing part of one pair in a corrupt SVG, we'll have an incomplete
         * list.  In that case, we reuse the last-known Y coordinate.
         */
        if (i + 1 < pointlist_len)
            y = pointlist[i + 1];
        else
            y = pointlist[i - 1];

        rsvg_path_builder_line_to (&builder, x, y);
    }

    if (close_path)
        rsvg_path_builder_close_path (&builder);

    path = rsvg_path_builder_finish (&builder);
    g_free (pointlist);

    return path;
}

static void
_rsvg_node_poly_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodePoly *poly = (RsvgNodePoly *) self;

    if (poly->path == NULL)
        return;

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    rsvg_render_path (ctx, poly->path);
}

static void
_rsvg_node_poly_free (RsvgNode * self)
{
    RsvgNodePoly *poly = (RsvgNodePoly *) self;
    if (poly->path)
        rsvg_cairo_path_destroy (poly->path);
    _rsvg_node_finalize (&poly->super);
    g_free (poly);
}

static RsvgNode *
rsvg_new_any_poly (RsvgNodeType type)
{
    RsvgNodePoly *poly;
    poly = g_new (RsvgNodePoly, 1);
    _rsvg_node_init (&poly->super, type);
    poly->super.free = _rsvg_node_poly_free;
    poly->super.draw = _rsvg_node_poly_draw;
    poly->super.set_atts = _rsvg_node_poly_set_atts;
    poly->path = NULL;
    return &poly->super;
}

RsvgNode *
rsvg_new_polygon (void)
{
    return rsvg_new_any_poly (RSVG_NODE_TYPE_POLYGON);
}

RsvgNode *
rsvg_new_polyline (void)
{
    return rsvg_new_any_poly (RSVG_NODE_TYPE_POLYLINE);
}


struct _RsvgNodeLine {
    RsvgNode super;
    RsvgLength x1, x2, y1, y2;
};

typedef struct _RsvgNodeLine RsvgNodeLine;

static void
_rsvg_node_line_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeLine *line = (RsvgNodeLine *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x1")))
            line->x1 = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y1")))
            line->y1 = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "x2")))
            line->x2 = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y2")))
            line->y2 = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "line", klazz, id, atts);
    }
}

static void
_rsvg_node_line_draw (RsvgNode * overself, RsvgDrawingCtx * ctx, int dominate)
{
    cairo_path_t *path;
    RsvgPathBuilder builder;
    RsvgNodeLine *self = (RsvgNodeLine *) overself;
    double x1, y1, x2, y2;

    rsvg_path_builder_init (&builder, 4);

    x1 = _rsvg_css_normalize_length (&self->x1, ctx, 'h');
    y1 = _rsvg_css_normalize_length (&self->y1, ctx, 'v');
    x2 = _rsvg_css_normalize_length (&self->x2, ctx, 'h');
    y2 = _rsvg_css_normalize_length (&self->y2, ctx, 'v');

    rsvg_path_builder_move_to (&builder, x1, y1);
    rsvg_path_builder_line_to (&builder, x2, y2);

    path = rsvg_path_builder_finish (&builder);

    rsvg_state_reinherit_top (ctx, overself->state, dominate);

    rsvg_render_path (ctx, path);
    rsvg_cairo_path_destroy (path);
}

RsvgNode *
rsvg_new_line (void)
{
    RsvgNodeLine *line;
    line = g_new (RsvgNodeLine, 1);
    _rsvg_node_init (&line->super, RSVG_NODE_TYPE_LINE);
    line->super.draw = _rsvg_node_line_draw;
    line->super.set_atts = _rsvg_node_line_set_atts;
    line->x1 = line->x2 = line->y1 = line->y2 = _rsvg_css_parse_length ("0");
    return &line->super;
}

struct _RsvgNodeRect {
    RsvgNode super;
    RsvgLength x, y, w, h, rx, ry;
    gboolean got_rx, got_ry;
};

typedef struct _RsvgNodeRect RsvgNodeRect;

static void
_rsvg_node_rect_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeRect *rect = (RsvgNodeRect *) self;

    /* FIXME: negative w/h/rx/ry is an error, per http://www.w3.org/TR/SVG11/shapes.html#RectElement */
    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            rect->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            rect->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            rect->w = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            rect->h = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "rx"))) {
            rect->rx = _rsvg_css_parse_length (value);
            rect->got_rx = TRUE;
        }
        if ((value = rsvg_property_bag_lookup (atts, "ry"))) {
            rect->ry = _rsvg_css_parse_length (value);
            rect->got_ry = TRUE;
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "rect", klazz, id, atts);
    }
}

static void
_rsvg_node_rect_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    double x, y, w, h, rx, ry;
    double half_w, half_h;
    RsvgPathBuilder builder;
    cairo_path_t *path;
    RsvgNodeRect *rect = (RsvgNodeRect *) self;

    x = _rsvg_css_normalize_length (&rect->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&rect->y, ctx, 'v');

    /* FIXME: negative w/h/rx/ry is an error, per http://www.w3.org/TR/SVG11/shapes.html#RectElement
     * For now we'll just take the absolute value.
     */
    w = fabs (_rsvg_css_normalize_length (&rect->w, ctx, 'h'));
    h = fabs (_rsvg_css_normalize_length (&rect->h, ctx, 'v'));
    rx = fabs (_rsvg_css_normalize_length (&rect->rx, ctx, 'h'));
    ry = fabs (_rsvg_css_normalize_length (&rect->ry, ctx, 'v'));

    if (w == 0. || h == 0.)
        return;

    if (rect->got_rx)
        rx = rx;
    else
        rx = ry;

    if (rect->got_ry)
        ry = ry;
    else
        ry = rx;

    half_w = w / 2;
    half_h = h / 2;

    if (rx > half_w)
        rx = half_w;

    if (ry > half_h)
        ry = half_h;

    if (rx == 0)
        ry = 0;
    else if (ry == 0)
        rx = 0;

    if (rx == 0) {
        /* Easy case, no rounded corners */

        rsvg_path_builder_init (&builder, 11);

        rsvg_path_builder_move_to (&builder, x, y);
        rsvg_path_builder_line_to (&builder, x + w, y);
        rsvg_path_builder_line_to (&builder, x + w, y + h);
        rsvg_path_builder_line_to (&builder, x, y + h);
        rsvg_path_builder_line_to (&builder, x, y);
        rsvg_path_builder_close_path (&builder);
    } else {
        double top_x1, top_x2, top_y;
        double bottom_x1, bottom_x2, bottom_y;
        double left_x, left_y1, left_y2;
        double right_x, right_y1, right_y2;

        /* Hard case, rounded corners
         *
         *      (top_x1, top_y)                   (top_x2, top_y)
         *     *--------------------------------*
         *    /                                  \
         *   * (left_x, left_y1)                  * (right_x, right_y1)
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   |                                    |
         *   * (left_x, left_y2)                  * (right_x, right_y2)
         *    \                                  /
         *     *--------------------------------*
         *      (bottom_x1, bottom_y)            (bottom_x2, bottom_y)
         */

        top_x1 = x + rx;
        top_x2 = x + w - rx;
        top_y  = y;

        bottom_x1 = top_x1;
        bottom_x2 = top_x2;
        bottom_y  = y + h;

        left_x = x;
        left_y1 = y + ry;
        left_y2 = y + h - ry;

        right_x = x + w;
        right_y1 = left_y1;
        right_y2 = left_y2;

        rsvg_path_builder_init (&builder, 32); /* an estimate; the arc segments may grow the array anyway */

        rsvg_path_builder_move_to (&builder, top_x1, top_y);
        rsvg_path_builder_line_to (&builder, top_x2, top_y);

        rsvg_path_builder_arc (&builder,
                               top_x2, top_y,
                               rx, ry, 0, FALSE, TRUE,
                               right_x, right_y1);

        rsvg_path_builder_line_to (&builder, right_x, right_y2);

        rsvg_path_builder_arc (&builder,
                               right_x, right_y2,
                               rx, ry, 0, FALSE, TRUE,
                               bottom_x2, bottom_y);

        rsvg_path_builder_line_to (&builder, bottom_x1, bottom_y);

        rsvg_path_builder_arc (&builder,
                               bottom_x1, bottom_y,
                               rx, ry, 0, FALSE, TRUE,
                               left_x, left_y2);

        rsvg_path_builder_line_to (&builder, left_x, left_y1);

        rsvg_path_builder_arc (&builder,
                               left_x, left_y1,
                               rx, ry, 0, FALSE, TRUE,
                               top_x1, top_y);

        rsvg_path_builder_close_path (&builder);
    }

    path = rsvg_path_builder_finish (&builder);

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, path);
    rsvg_cairo_path_destroy (path);
}

RsvgNode *
rsvg_new_rect (void)
{
    RsvgNodeRect *rect;
    rect = g_new (RsvgNodeRect, 1);
    _rsvg_node_init (&rect->super, RSVG_NODE_TYPE_RECT);
    rect->super.draw = _rsvg_node_rect_draw;
    rect->super.set_atts = _rsvg_node_rect_set_atts;
    rect->x = rect->y = rect->w = rect->h = rect->rx = rect->ry = _rsvg_css_parse_length ("0");
    rect->got_rx = rect->got_ry = FALSE;
    return &rect->super;
}

struct _RsvgNodeCircle {
    RsvgNode super;
    RsvgLength cx, cy, r;
};

typedef struct _RsvgNodeCircle RsvgNodeCircle;

static void
_rsvg_node_circle_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeCircle *circle = (RsvgNodeCircle *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "cx")))
            circle->cx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "cy")))
            circle->cy = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "r")))
            circle->r = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "circle", klazz, id, atts);
    }
}

static void
_rsvg_node_circle_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    cairo_path_t *path;
    RsvgNodeCircle *circle = (RsvgNodeCircle *) self;
    double cx, cy, r;
    RsvgPathBuilder builder;

    cx = _rsvg_css_normalize_length (&circle->cx, ctx, 'h');
    cy = _rsvg_css_normalize_length (&circle->cy, ctx, 'v');
    r = _rsvg_css_normalize_length (&circle->r, ctx, 'o');

    if (r <= 0)
        return;

    /* approximate a circle using 4 bezier curves */

    rsvg_path_builder_init (&builder, 19);

    rsvg_path_builder_move_to (&builder, cx + r, cy);

    rsvg_path_builder_curve_to (&builder,
                                cx + r, cy + r * RSVG_ARC_MAGIC,
                                cx + r * RSVG_ARC_MAGIC, cy + r,
                                cx, cy + r);

    rsvg_path_builder_curve_to (&builder,
                                cx - r * RSVG_ARC_MAGIC, cy + r,
                                cx - r, cy + r * RSVG_ARC_MAGIC,
                                cx - r, cy);

    rsvg_path_builder_curve_to (&builder,
                                cx - r, cy - r * RSVG_ARC_MAGIC,
                                cx - r * RSVG_ARC_MAGIC, cy - r,
                                cx, cy - r);

    rsvg_path_builder_curve_to (&builder,
                                cx + r * RSVG_ARC_MAGIC, cy - r,
                                cx + r, cy - r * RSVG_ARC_MAGIC,
                                cx + r, cy);

    rsvg_path_builder_close_path (&builder);

    path = rsvg_path_builder_finish (&builder);

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, path);
    rsvg_cairo_path_destroy (path);
}

RsvgNode *
rsvg_new_circle (void)
{
    RsvgNodeCircle *circle;
    circle = g_new (RsvgNodeCircle, 1);
    _rsvg_node_init (&circle->super, RSVG_NODE_TYPE_CIRCLE);
    circle->super.draw = _rsvg_node_circle_draw;
    circle->super.set_atts = _rsvg_node_circle_set_atts;
    circle->cx = circle->cy = circle->r = _rsvg_css_parse_length ("0");
    return &circle->super;
}

struct _RsvgNodeEllipse {
    RsvgNode super;
    RsvgLength cx, cy, rx, ry;
};

typedef struct _RsvgNodeEllipse RsvgNodeEllipse;

static void
_rsvg_node_ellipse_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeEllipse *ellipse = (RsvgNodeEllipse *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "cx")))
            ellipse->cx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "cy")))
            ellipse->cy = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "rx")))
            ellipse->rx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "ry")))
            ellipse->ry = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "ellipse", klazz, id, atts);
    }
}

static void
_rsvg_node_ellipse_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeEllipse *ellipse = (RsvgNodeEllipse *) self;
    cairo_path_t *path;
    double cx, cy, rx, ry;
    RsvgPathBuilder builder;

    cx = _rsvg_css_normalize_length (&ellipse->cx, ctx, 'h');
    cy = _rsvg_css_normalize_length (&ellipse->cy, ctx, 'v');
    rx = _rsvg_css_normalize_length (&ellipse->rx, ctx, 'h');
    ry = _rsvg_css_normalize_length (&ellipse->ry, ctx, 'v');

    if (rx <= 0 || ry <= 0)
        return;

    /* approximate an ellipse using 4 bezier curves */

    rsvg_path_builder_init (&builder, 19);

    rsvg_path_builder_move_to (&builder, cx + rx, cy);

    rsvg_path_builder_curve_to (&builder,
                                cx + rx, cy - RSVG_ARC_MAGIC * ry,
                                cx + RSVG_ARC_MAGIC * rx, cy - ry,
                                cx, cy - ry);

    rsvg_path_builder_curve_to (&builder,
                                cx - RSVG_ARC_MAGIC * rx, cy - ry,
                                cx - rx, cy - RSVG_ARC_MAGIC * ry,
                                cx - rx, cy);

    rsvg_path_builder_curve_to (&builder,
                                cx - rx, cy + RSVG_ARC_MAGIC * ry,
                                cx - RSVG_ARC_MAGIC * rx, cy + ry,
                                cx, cy + ry);

    rsvg_path_builder_curve_to (&builder,
                                cx + RSVG_ARC_MAGIC * rx, cy + ry,
                                cx + rx, cy + RSVG_ARC_MAGIC * ry,
                                cx + rx, cy);

    rsvg_path_builder_close_path (&builder);

    path = rsvg_path_builder_finish (&builder);

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, path);
    rsvg_cairo_path_destroy (path);
}

RsvgNode *
rsvg_new_ellipse (void)
{
    RsvgNodeEllipse *ellipse;
    ellipse = g_new (RsvgNodeEllipse, 1);
    _rsvg_node_init (&ellipse->super, RSVG_NODE_TYPE_ELLIPSE);
    ellipse->super.draw = _rsvg_node_ellipse_draw;
    ellipse->super.set_atts = _rsvg_node_ellipse_set_atts;
    ellipse->cx = ellipse->cy = ellipse->rx = ellipse->ry = _rsvg_css_parse_length ("0");
    return &ellipse->super;
}
