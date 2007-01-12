/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

/* 4/3 * (1-cos 45)/sin 45 = 4/3 * sqrt(2) - 1 */
#define RSVG_ARC_MAGIC ((double) 0.5522847498)

static void
rsvg_node_path_free (RsvgNode * self)
{
    RsvgNodePath *z = (RsvgNodePath *) self;
    if (z->d)
        g_free (z->d);
    _rsvg_node_finalize (&z->super);
    g_free (z);
}

static void
rsvg_node_path_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodePath *path = (RsvgNodePath *) self;
    if (!path->d)
        return;

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    rsvg_render_path (ctx, path->d);
}

static void
rsvg_node_path_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodePath *path = (RsvgNodePath *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "d"))) {
            if (path->d)
                g_free (path->d);
            path->d = g_strdup (value);
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
    _rsvg_node_init (&path->super);
    path->d = NULL;
    path->super.free = rsvg_node_path_free;
    path->super.draw = rsvg_node_path_draw;
    path->super.set_atts = rsvg_node_path_set_atts;

    return &path->super;
}

struct _RsvgNodePoly {
    RsvgNode super;
    gdouble *pointlist;
    gboolean is_polyline;
    guint pointlist_len;
};

typedef struct _RsvgNodePoly RsvgNodePoly;

static void
_rsvg_node_poly_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgNodePoly *poly = (RsvgNodePoly *) self;
    const char *klazz = NULL, *id = NULL, *value;

    if (rsvg_property_bag_size (atts)) {
        /* support for svg < 1.0 which used verts */
        if ((value = rsvg_property_bag_lookup (atts, "verts"))
            || (value = rsvg_property_bag_lookup (atts, "points"))) {
            poly->pointlist = rsvg_css_parse_number_list (value, &poly->pointlist_len);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, (poly->is_polyline ? "polyline" : "polygon"),
                                klazz, id, atts);
    }

}

static void
_rsvg_node_poly_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodePoly *poly = (RsvgNodePoly *) self;
    gsize i;
    GString *d;
    char buf[G_ASCII_DTOSTR_BUF_SIZE];

    /* represent as a "moveto, lineto*, close" path */
    if (poly->pointlist_len < 2)
        return;

    d = g_string_new (NULL);

    /*      "M %f %f " */
    g_string_append (d, " M ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), poly->pointlist[0]));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), poly->pointlist[1]));

    /* "L %f %f " */
    for (i = 2; i < poly->pointlist_len; i += 2) {
        g_string_append (d, " L ");
        g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), poly->pointlist[i]));
        g_string_append_c (d, ' ');
        g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), poly->pointlist[i + 1]));
    }

    if (!poly->is_polyline)
        g_string_append (d, " Z");

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, d->str);

    g_string_free (d, TRUE);
}

static void
_rsvg_node_poly_free (RsvgNode * self)
{
    RsvgNodePoly *z = (RsvgNodePoly *) self;
    if (z->pointlist)
        g_free (z->pointlist);
    _rsvg_node_finalize (&z->super);
    g_free (z);
}


static RsvgNode *
rsvg_new_any_poly (gboolean is_polyline)
{
    RsvgNodePoly *poly;
    poly = g_new (RsvgNodePoly, 1);
    _rsvg_node_init (&poly->super);
    poly->super.free = _rsvg_node_poly_free;
    poly->super.draw = _rsvg_node_poly_draw;
    poly->super.set_atts = _rsvg_node_poly_set_atts;
    poly->pointlist = NULL;
    poly->is_polyline = is_polyline;
    poly->pointlist_len = 0;
    return &poly->super;
}

RsvgNode *
rsvg_new_polygon (void)
{
    return rsvg_new_any_poly (FALSE);
}

RsvgNode *
rsvg_new_polyline (void)
{
    return rsvg_new_any_poly (TRUE);
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
    GString *d;
    char buf[G_ASCII_DTOSTR_BUF_SIZE];
    RsvgNodeLine *self = (RsvgNodeLine *) overself;

    /* emulate a line using a path */
    /* ("M %f %f L %f %f", x1, y1, x2, y2) */
    d = g_string_new ("M ");

    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf),
                                        _rsvg_css_normalize_length (&self->x1, ctx, 'h')));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf),
                                        _rsvg_css_normalize_length (&self->y1, ctx, 'v')));
    g_string_append (d, " L ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf),
                                        _rsvg_css_normalize_length (&self->x2, ctx, 'h')));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf),
                                        _rsvg_css_normalize_length (&self->y2, ctx, 'v')));

    rsvg_state_reinherit_top (ctx, overself->state, dominate);
    rsvg_render_path (ctx, d->str);

    g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_line (void)
{
    RsvgNodeLine *line;
    line = g_new (RsvgNodeLine, 1);
    _rsvg_node_init (&line->super);
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
    GString *d = NULL;
    RsvgNodeRect *rect = (RsvgNodeRect *) self;
    char buf[G_ASCII_DTOSTR_BUF_SIZE];

    x = _rsvg_css_normalize_length (&rect->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&rect->y, ctx, 'v');
    w = _rsvg_css_normalize_length (&rect->w, ctx, 'h');
    h = _rsvg_css_normalize_length (&rect->h, ctx, 'v');
    rx = _rsvg_css_normalize_length (&rect->rx, ctx, 'h');
    ry = _rsvg_css_normalize_length (&rect->ry, ctx, 'v');

    if (rect->got_rx)
        rx = rx;
    else
        rx = ry;
    if (rect->got_ry)
        ry = ry;
    else
        ry = rx;

    if (rx > fabs (w / 2.))
        rx = fabs (w / 2.);
    if (ry > fabs (h / 2.))
        ry = fabs (h / 2.);

    if (rx == 0)
        ry = 0;
    else if (ry == 0)
        rx = 0;

    /* emulate a rect using a path */
    d = g_string_new ("M ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

    g_string_append (d, " H ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + w - rx));

    g_string_append (d, " A");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 1.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + w));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y + ry));

    g_string_append (d, " V ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y + h - ry));

    g_string_append (d, " A");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 1.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + w - rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y + h));

    g_string_append (d, " H ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + rx));

    g_string_append (d, " A");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 1.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y + h - ry));

    g_string_append (d, " V ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y + ry));

    g_string_append (d, " A");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 0.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), 1.));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

    g_string_append (d, " Z");

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, d->str);
    g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_rect (void)
{
    RsvgNodeRect *rect;
    rect = g_new (RsvgNodeRect, 1);
    _rsvg_node_init (&rect->super);
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
    GString *d = NULL;
    RsvgNodeCircle *circle = (RsvgNodeCircle *) self;
    char buf[G_ASCII_DTOSTR_BUF_SIZE];
    double cx, cy, r;

    cx = _rsvg_css_normalize_length (&circle->cx, ctx, 'h');
    cy = _rsvg_css_normalize_length (&circle->cy, ctx, 'v');
    r = _rsvg_css_normalize_length (&circle->r, ctx, 'o');

    if (r <= 0)
        return;

    /* approximate a circle using 4 bezier curves */

    d = g_string_new ("M ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + r));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - r));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - r * RSVG_ARC_MAGIC));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + r));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " Z");

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, d->str);

    g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_circle (void)
{
    RsvgNodeCircle *circle;
    circle = g_new (RsvgNodeCircle, 1);
    _rsvg_node_init (&circle->super);
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
    GString *d = NULL;
    char buf[G_ASCII_DTOSTR_BUF_SIZE];
    double cx, cy, rx, ry;

    cx = _rsvg_css_normalize_length (&ellipse->cx, ctx, 'h');
    cy = _rsvg_css_normalize_length (&ellipse->cy, ctx, 'v');
    rx = _rsvg_css_normalize_length (&ellipse->rx, ctx, 'h');
    ry = _rsvg_css_normalize_length (&ellipse->ry, ctx, 'v');

    if (rx <= 0 || ry <= 0)
        return;
    /* approximate an ellipse using 4 bezier curves */

    d = g_string_new ("M ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - RSVG_ARC_MAGIC * ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + RSVG_ARC_MAGIC * rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - ry));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - RSVG_ARC_MAGIC * rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy - RSVG_ARC_MAGIC * ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + RSVG_ARC_MAGIC * ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx - RSVG_ARC_MAGIC * rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + ry));

    g_string_append (d, " C ");
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + RSVG_ARC_MAGIC * rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy + RSVG_ARC_MAGIC * ry));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx + rx));
    g_string_append_c (d, ' ');
    g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

    g_string_append (d, " Z");

    rsvg_state_reinherit_top (ctx, self->state, dominate);
    rsvg_render_path (ctx, d->str);
    g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_ellipse (void)
{
    RsvgNodeEllipse *ellipse;
    ellipse = g_new (RsvgNodeEllipse, 1);
    _rsvg_node_init (&ellipse->super);
    ellipse->super.draw = _rsvg_node_ellipse_draw;
    ellipse->super.set_atts = _rsvg_node_ellipse_set_atts;
    ellipse->cx = ellipse->cy = ellipse->rx = ellipse->ry = _rsvg_css_parse_length ("0");
    return &ellipse->super;
}
