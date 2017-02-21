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
