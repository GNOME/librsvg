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
#include "rsvg-path-builder.h"
#include "rsvg-marker.h"

/* 4/3 * (1-cos 45)/sin 45 = 4/3 * sqrt(2) - 1 */
#define RSVG_ARC_MAGIC ((double) 0.5522847498)

typedef struct _RsvgNodePoly RsvgNodePoly;

struct _RsvgNodePoly {
    RsvgPathBuilder *builder;
};

static RsvgPathBuilder *
rsvg_node_poly_create_builder (const char *value,
                               gboolean close_path);

static void
rsvg_node_poly_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodePoly *poly = impl;
    const char *value;

    /* support for svg < 1.0 which used verts */
    if ((value = rsvg_property_bag_lookup (atts, "verts"))
        || (value = rsvg_property_bag_lookup (atts, "points"))) {
        if (poly->builder)
            rsvg_path_builder_destroy (poly->builder);
        poly->builder = rsvg_node_poly_create_builder (value,
                                                       rsvg_node_get_type (node) == RSVG_NODE_TYPE_POLYGON);
    }
}

static RsvgPathBuilder *
rsvg_node_poly_create_builder (const char *value,
                               gboolean close_path)
{
    double *pointlist;
    guint pointlist_len, i;
    RsvgPathBuilder *builder;

    pointlist = rsvg_css_parse_number_list (value, &pointlist_len);
    if (pointlist == NULL)
        return NULL;

    if (pointlist_len < 2) {
        g_free (pointlist);
        return NULL;
    }

    builder = rsvg_path_builder_new ();

    rsvg_path_builder_move_to (builder, pointlist[0], pointlist[1]);

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

        rsvg_path_builder_line_to (builder, x, y);
    }

    if (close_path)
        rsvg_path_builder_close_path (builder);

    g_free (pointlist);

    return builder;
}

static void
rsvg_node_poly_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodePoly *poly = impl;

    if (poly->builder == NULL)
        return;

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    rsvg_render_path_builder (ctx, poly->builder);
    rsvg_render_markers (ctx, poly->builder);
}

static void
rsvg_node_poly_free (gpointer impl)
{
    RsvgNodePoly *poly = impl;

    if (poly->builder)
        rsvg_path_builder_destroy (poly->builder);

    g_free (poly);
}

static RsvgNode *
rsvg_new_any_poly (RsvgNodeType type, RsvgNode *parent)
{
    RsvgNodePoly *poly;

    poly = g_new0 (RsvgNodePoly, 1);
    poly->builder = NULL;

    return rsvg_rust_cnode_new (type,
                                parent,
                                rsvg_state_new (),
                                poly,
                                rsvg_node_poly_set_atts,
                                rsvg_node_poly_draw,
                                rsvg_node_poly_free);
}

RsvgNode *
rsvg_new_polygon (const char *element_name, RsvgNode *parent)
{
    return rsvg_new_any_poly (RSVG_NODE_TYPE_POLYGON, parent);
}

RsvgNode *
rsvg_new_polyline (const char *element_name, RsvgNode *parent)
{
    return rsvg_new_any_poly (RSVG_NODE_TYPE_POLYLINE, parent);
}

typedef struct _RsvgNodeRect RsvgNodeRect;

struct _RsvgNodeRect {
    RsvgLength x, y, w, h, rx, ry;
    gboolean got_rx, got_ry;
};

static void
rsvg_node_rect_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeRect *rect = impl;
    const char *value;

    /* FIXME: negative w/h/rx/ry is an error, per http://www.w3.org/TR/SVG11/shapes.html#RectElement */
    if ((value = rsvg_property_bag_lookup (atts, "x")))
        rect->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        rect->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "width")))
        rect->w = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "height")))
        rect->h = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "rx"))) {
        rect->rx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        rect->got_rx = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "ry"))) {
        rect->ry = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        rect->got_ry = TRUE;
    }
}

static void
rsvg_node_rect_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeRect *rect = impl;
    double x, y, w, h, rx, ry;
    double half_w, half_h;
    RsvgPathBuilder *builder;

    x = rsvg_length_normalize (&rect->x, ctx);
    y = rsvg_length_normalize (&rect->y, ctx);

    /* FIXME: negative w/h/rx/ry is an error, per http://www.w3.org/TR/SVG11/shapes.html#RectElement
     * For now we'll just take the absolute value.
     */
    w = fabs (rsvg_length_normalize (&rect->w, ctx));
    h = fabs (rsvg_length_normalize (&rect->h, ctx));
    rx = fabs (rsvg_length_normalize (&rect->rx, ctx));
    ry = fabs (rsvg_length_normalize (&rect->ry, ctx));

    if (w == 0. || h == 0.)
        return;

    if (rect->got_rx && !rect->got_ry)
        ry = rx;
    else if (!rect->got_rx && rect->got_ry)
        rx = ry;

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

    builder = rsvg_path_builder_new ();

    if (rx == 0) {
        /* Easy case, no rounded corners */

        rsvg_path_builder_move_to (builder, x, y);
        rsvg_path_builder_line_to (builder, x + w, y);
        rsvg_path_builder_line_to (builder, x + w, y + h);
        rsvg_path_builder_line_to (builder, x, y + h);
        rsvg_path_builder_line_to (builder, x, y);
        rsvg_path_builder_close_path (builder);
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

        rsvg_path_builder_move_to (builder, top_x1, top_y);
        rsvg_path_builder_line_to (builder, top_x2, top_y);

        rsvg_path_builder_arc (builder,
                               top_x2, top_y,
                               rx, ry, 0, FALSE, TRUE,
                               right_x, right_y1);

        rsvg_path_builder_line_to (builder, right_x, right_y2);

        rsvg_path_builder_arc (builder,
                               right_x, right_y2,
                               rx, ry, 0, FALSE, TRUE,
                               bottom_x2, bottom_y);

        rsvg_path_builder_line_to (builder, bottom_x1, bottom_y);

        rsvg_path_builder_arc (builder,
                               bottom_x1, bottom_y,
                               rx, ry, 0, FALSE, TRUE,
                               left_x, left_y2);

        rsvg_path_builder_line_to (builder, left_x, left_y1);

        rsvg_path_builder_arc (builder,
                               left_x, left_y1,
                               rx, ry, 0, FALSE, TRUE,
                               top_x1, top_y);

        rsvg_path_builder_close_path (builder);
    }

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    rsvg_render_path_builder (ctx, builder);

    rsvg_path_builder_destroy (builder);
}

static void
rsvg_node_rect_free (gpointer impl)
{
    RsvgNodeRect *rect = impl;

    g_free (rect);
}

RsvgNode *
rsvg_new_rect (const char *element_name, RsvgNode *parent)
{
    RsvgNodeRect *rect;

    rect = g_new0 (RsvgNodeRect, 1);
    rect->x = rect->y = rect->w = rect->h = rect->rx = rect->ry = rsvg_length_parse ("0", LENGTH_DIR_BOTH);
    rect->got_rx = rect->got_ry = FALSE;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_RECT,
                                parent,
                                rsvg_state_new (),
                                rect,
                                rsvg_node_rect_set_atts,
                                rsvg_node_rect_draw,
                                rsvg_node_rect_free);
}

typedef struct _RsvgNodeCircle RsvgNodeCircle;

struct _RsvgNodeCircle {
    RsvgLength cx, cy, r;
};

static void
rsvg_node_circle_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeCircle *circle = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "cx")))
        circle->cx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "cy")))
        circle->cy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "r")))
        circle->r = rsvg_length_parse (value, LENGTH_DIR_BOTH);
}

static void
rsvg_node_circle_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeCircle *circle = impl;
    double cx, cy, r;
    RsvgPathBuilder *builder;

    cx = rsvg_length_normalize (&circle->cx, ctx);
    cy = rsvg_length_normalize (&circle->cy, ctx);
    r = rsvg_length_normalize (&circle->r, ctx);

    if (r <= 0)
        return;

    /* approximate a circle using 4 bezier curves */

    builder = rsvg_path_builder_new ();

    rsvg_path_builder_move_to (builder, cx + r, cy);

    rsvg_path_builder_curve_to (builder,
                                cx + r, cy + r * RSVG_ARC_MAGIC,
                                cx + r * RSVG_ARC_MAGIC, cy + r,
                                cx, cy + r);

    rsvg_path_builder_curve_to (builder,
                                cx - r * RSVG_ARC_MAGIC, cy + r,
                                cx - r, cy + r * RSVG_ARC_MAGIC,
                                cx - r, cy);

    rsvg_path_builder_curve_to (builder,
                                cx - r, cy - r * RSVG_ARC_MAGIC,
                                cx - r * RSVG_ARC_MAGIC, cy - r,
                                cx, cy - r);

    rsvg_path_builder_curve_to (builder,
                                cx + r * RSVG_ARC_MAGIC, cy - r,
                                cx + r, cy - r * RSVG_ARC_MAGIC,
                                cx + r, cy);

    rsvg_path_builder_close_path (builder);

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    rsvg_render_path_builder (ctx, builder);

    rsvg_path_builder_destroy (builder);
}

static void
rsvg_node_circle_free (gpointer impl)
{
    RsvgNodeCircle *circle = impl;

    g_free (circle);
}

RsvgNode *
rsvg_new_circle (const char *element_name, RsvgNode *parent)
{
    RsvgNodeCircle *circle;

    circle = g_new0 (RsvgNodeCircle, 1);
    circle->cx = circle->cy = circle->r = rsvg_length_parse ("0", LENGTH_DIR_BOTH);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_CIRCLE,
                                parent,
                                rsvg_state_new (),
                                circle,
                                rsvg_node_circle_set_atts,
                                rsvg_node_circle_draw,
                                rsvg_node_circle_free);
}

typedef struct _RsvgNodeEllipse RsvgNodeEllipse;

struct _RsvgNodeEllipse {
    RsvgLength cx, cy, rx, ry;
};

static void
rsvg_node_ellipse_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeEllipse *ellipse = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "cx")))
        ellipse->cx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "cy")))
        ellipse->cy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "rx")))
        ellipse->rx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "ry")))
        ellipse->ry = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
}

static void
rsvg_node_ellipse_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeEllipse *ellipse = impl;
    double cx, cy, rx, ry;
    RsvgPathBuilder *builder;

    cx = rsvg_length_normalize (&ellipse->cx, ctx);
    cy = rsvg_length_normalize (&ellipse->cy, ctx);
    rx = rsvg_length_normalize (&ellipse->rx, ctx);
    ry = rsvg_length_normalize (&ellipse->ry, ctx);

    if (rx <= 0 || ry <= 0)
        return;

    /* approximate an ellipse using 4 bezier curves */

    builder = rsvg_path_builder_new ();

    rsvg_path_builder_move_to (builder, cx + rx, cy);

    rsvg_path_builder_curve_to (builder,
                                cx + rx, cy - RSVG_ARC_MAGIC * ry,
                                cx + RSVG_ARC_MAGIC * rx, cy - ry,
                                cx, cy - ry);

    rsvg_path_builder_curve_to (builder,
                                cx - RSVG_ARC_MAGIC * rx, cy - ry,
                                cx - rx, cy - RSVG_ARC_MAGIC * ry,
                                cx - rx, cy);

    rsvg_path_builder_curve_to (builder,
                                cx - rx, cy + RSVG_ARC_MAGIC * ry,
                                cx - RSVG_ARC_MAGIC * rx, cy + ry,
                                cx, cy + ry);

    rsvg_path_builder_curve_to (builder,
                                cx + RSVG_ARC_MAGIC * rx, cy + ry,
                                cx + rx, cy + RSVG_ARC_MAGIC * ry,
                                cx + rx, cy);

    rsvg_path_builder_close_path (builder);

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    rsvg_render_path_builder (ctx, builder);

    rsvg_path_builder_destroy (builder);
}

static void
rsvg_node_ellipse_free (gpointer impl)
{
    RsvgNodeRect *ellipse = impl;

    g_free (ellipse);
}

RsvgNode *
rsvg_new_ellipse (const char *element_name, RsvgNode *parent)
{
    RsvgNodeEllipse *ellipse;

    ellipse = g_new0 (RsvgNodeEllipse, 1);
    ellipse->cx = ellipse->cy = ellipse->rx = ellipse->ry = rsvg_length_parse ("0", LENGTH_DIR_BOTH);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_ELLIPSE,
                                parent,
                                rsvg_state_new (),
                                ellipse,
                                rsvg_node_ellipse_set_atts,
                                rsvg_node_ellipse_draw,
                                rsvg_node_ellipse_free);
}
