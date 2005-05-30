/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
rsvg_node_path_free (RsvgNode *self)
{
	RsvgNodePath *z = (RsvgNodePath *)self;
	rsvg_state_finalize (z->super.state);
	g_free(z->super.state);
	if (z->d)
		g_free (z->d);
	g_free (z);
}

static void 
rsvg_node_path_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
					 int dominate)
{
	RsvgNodePath *path = (RsvgNodePath*)self;
	if (!path->d)
		return;

	rsvg_state_reinherit_top(ctx, self->state, dominate);

	rsvg_render_path (ctx, path->d);	
}

static void
rsvg_node_path_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	RsvgNodePath * path = (RsvgNodePath *)self;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "d")))
				{
					if (path->d)
						g_free(path->d);
					path->d = g_strdup(value);
				}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "path", klazz, id, atts);
		}
}

RsvgNode *
rsvg_new_path (void)
{
	RsvgNodePath *path;	
	path = g_new (RsvgNodePath, 1);
	path->d = NULL;
	path->super.state = g_new(RsvgState, 1);
	rsvg_state_init(path->super.state);
	path->super.children = NULL;
	path->super.type = RSVG_NODE_PATH;
	path->super.free = rsvg_node_path_free;
	path->super.draw = rsvg_node_path_draw;
	path->super.set_atts = rsvg_node_path_set_atts;
	
	return &path->super;
}

static GString *
rsvg_make_poly_point_list(const char * points)
{
	guint idx = 0, size = strlen(points);
	GString * str = g_string_sized_new (size);
	
	while (idx < size) 
		{
			/* scan for first point */
			while (!g_ascii_isdigit (points[idx]) && (points[idx] != '.') 
				   && (points[idx] != '-') && (idx < size))
				idx++;
			
			/* now build up the point list (everything until next letter!) */
			if (idx < size && points[idx] == '-')
				g_string_append_c (str, points[idx++]); /* handle leading '-' */
			while ((g_ascii_isdigit (points[idx]) || (points[idx] == '.')) && (idx < size)) 
				g_string_append_c (str, points[idx++]);
			
			g_string_append_c (str, ' ');
		}
	
	return str;
}

struct _RsvgNodePoly
{
	RsvgNode super;
	double * pointlist;
	gboolean is_polyline;
	guint pointlist_len;	
};

typedef struct _RsvgNodePoly RsvgNodePoly;

static void
_rsvg_node_poly_set_atts(RsvgNode * self, RsvgHandle *ctx, 
						 RsvgPropertyBag *atts)
{
	RsvgNodePoly * poly = (RsvgNodePoly *)self;
	const char * klazz = NULL, * id = NULL, *value;

	if (rsvg_property_bag_size (atts))
		{
			/* support for svg < 1.0 which used verts */
			if ((value = rsvg_property_bag_lookup (atts, "verts")) || (value = rsvg_property_bag_lookup (atts, "points")))
				{
					guint i;
					GString * g = NULL;
					gsize pointlist_len = 0;
					gchar ** pointlist = NULL;

					if (poly->pointlist)
						g_free(poly->pointlist);

					g = rsvg_make_poly_point_list (value);
					pointlist = g_strsplit (g->str, " ", -1);
					g_string_free (g, TRUE);
					
					if (pointlist)
						{
							while(pointlist[pointlist_len] != NULL)
								pointlist_len++;
						}
					if (pointlist_len > 0)
						pointlist_len--;

					poly->pointlist = g_new(double, pointlist_len);
					poly->pointlist_len = pointlist_len;			

					for (i = 0; i < pointlist_len; i++)
						poly->pointlist[i] = atof(pointlist[i]);

					if (pointlist)
						g_strfreev(pointlist);
				}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, (poly->is_polyline ? "polyline" : "polygon"), klazz, id, atts);
		}
	
}

static void
_rsvg_node_poly_draw(RsvgNode * self, RsvgDrawingCtx *ctx, 
					 int dominate)
{
	RsvgNodePoly * poly = (RsvgNodePoly *)self;
	/* represent as a "moveto, lineto*, close" path */  
	if (poly->pointlist_len < 2)
		return;

	gsize i;
	GString * d = g_string_new ("");
	g_string_append_printf (d, "M %f %f ", poly->pointlist[0], poly->pointlist[1] );
	
	for (i = 2; i < poly->pointlist_len; i += 2)
		g_string_append_printf (d, "L %f %f ", poly->pointlist[i], poly->pointlist[i+1]);
	
	if (!poly->is_polyline)
		g_string_append (d, "Z");
	
	rsvg_state_reinherit_top(ctx, self->state, dominate);
	rsvg_render_path (ctx, d->str);

	g_string_free (d, TRUE);
}

static void 
_rsvg_node_poly_free (RsvgNode *self)
{
	RsvgNodePoly *z = (RsvgNodePoly *)self;
	rsvg_state_finalize (z->super.state);
	g_free(z->super.state);
	if (z->pointlist)
		g_free (z->pointlist);
	g_free (z);
}


static RsvgNode *
rsvg_new_any_poly(gboolean is_polyline)
{
	RsvgNodePoly *poly;
	poly = g_new (RsvgNodePoly, 1);
	poly->super.children = NULL;
	poly->super.state = g_new(RsvgState, 1);
	rsvg_state_init(poly->super.state);
	poly->super.type = RSVG_NODE_PATH;
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


struct _RsvgNodeLine
{
	RsvgNode super;
	double x1, x2, y1, y2;
};

typedef struct _RsvgNodeLine RsvgNodeLine;

static void
_rsvg_node_line_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, *id = NULL, *value;
	double font_size;
	RsvgNodeLine * line = (RsvgNodeLine *) self;

	font_size = rsvg_state_current_font_size (ctx);
	

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x1")))
				line->x1 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y1")))
				line->y1 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "x2")))
				line->x2 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y2")))
				line->y2 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}
				
			rsvg_parse_style_attrs (ctx, self->state, "line", klazz, id, atts);
		}
}

static void
_rsvg_node_line_draw(RsvgNode * overself, RsvgDrawingCtx *ctx, 
					 int dominate)
{
	GString * d;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	RsvgNodeLine * self = (RsvgNodeLine *)overself;

	/* emulate a line using a path */
	/* ("M %f %f L %f %f", x1, y1, x2, y2) */
	d = g_string_new ("M ");   

	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), self->x1));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), self->y1));
	g_string_append (d, " L ");	
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), self->x2));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), self->y2));

	rsvg_state_reinherit_top(ctx, overself->state, dominate);
	rsvg_render_path (ctx, d->str);

	g_string_free (d, TRUE);	
}

RsvgNode *
rsvg_new_line (void)
{
	RsvgNodeLine *line;
	line = g_new (RsvgNodeLine, 1);
	line->super.children = NULL;
	line->super.state = g_new(RsvgState, 1);
	rsvg_state_init(line->super.state);
	line->super.type = RSVG_NODE_PATH;
	line->super.free = rsvg_node_free;
	line->super.draw = _rsvg_node_line_draw;
	line->super.set_atts = _rsvg_node_line_set_atts;
	line->x1 = line->x2 = line->y1 = line->y2 = 0;
	return &line->super;
}

struct _RsvgNodeRect
{
	RsvgNode super;
	double x, y, w, h, rx, ry;
	gboolean got_rx, got_ry;
};

typedef struct _RsvgNodeRect RsvgNodeRect;

static void
_rsvg_node_rect_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	double font_size;
	RsvgNodeRect * rect = (RsvgNodeRect *)self;

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				rect->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				rect->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size) + 0.01;
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				rect->w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				rect->h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "rx"))) {
				rect->rx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
				rect->got_rx = TRUE;	
			}
			if ((value = rsvg_property_bag_lookup (atts, "ry"))) {
				rect->ry = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
				rect->got_ry = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "rect", klazz, id, atts);
		}
}

static void
_rsvg_node_rect_draw(RsvgNode * self, RsvgDrawingCtx *ctx, 
					 int dominate)
{
	double rx, ry;	
	GString * d = NULL;
	RsvgNodeRect * rect = (RsvgNodeRect *)self;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];

	if (rect->got_rx)
		rx = rect->rx;		
	else
		rx = rect->ry;
	if (rect->got_ry)
		ry = rect->ry;		
	else
		ry = rect->rx;

	if (rx > fabs(rect->w / 2.))
		rx = fabs(rect->w / 2.);
	if (ry > fabs(rect->h / 2.))
		ry = fabs(rect->h / 2.);

	/* emulate a rect using a path */
	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x + rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x + rect->w - rx));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x+rect->w));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y+ry));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y+rect->h-ry));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x + rect->w - rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y + rect->h));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x + rx));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y +rect-> h - ry));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y+ry));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->x+rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), rect->y));

	g_string_append (d, " Z");

	rsvg_state_reinherit_top(ctx, self->state, dominate);
	rsvg_render_path (ctx, d->str);
	g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_rect (void)
{
	RsvgNodeRect *rect;
	rect = g_new (RsvgNodeRect, 1);
	rect->super.children = NULL;
	rect->super.state = g_new(RsvgState, 1);
	rsvg_state_init(rect->super.state);
	rect->super.type = RSVG_NODE_PATH;
	rect->super.free = rsvg_node_free;
	rect->super.draw = _rsvg_node_rect_draw;
	rect->super.set_atts = _rsvg_node_rect_set_atts;
	rect->x = rect->y = rect->w = rect->h = rect->rx = rect->ry = 0;
	rect->got_rx = rect->got_ry = FALSE;
	return &rect->super;
}

struct _RsvgNodeCircle
{
	RsvgNode super;
	double cx, cy, r;
};

typedef struct _RsvgNodeCircle RsvgNodeCircle;

static void
_rsvg_node_circle_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	double font_size;
	RsvgNodeCircle * circle = (RsvgNodeCircle *)self;

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "cx")))
				circle->cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "cy")))
				circle->cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "r")))
				circle->r = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (ctx), 
															  rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), 
															  font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "circle", klazz, id, atts);
		}
}

static void
_rsvg_node_circle_draw(RsvgNode * self, RsvgDrawingCtx *ctx, 
					   int dominate)
{
	GString * d = NULL;
	RsvgNodeCircle * circle = (RsvgNodeCircle *)self;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];

	if (circle->r <= 0)
		return;
	
	/* approximate a circle using 4 bezier curves */

	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx+circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx+circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy + circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx + circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy + circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy + circle->r));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx - circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy + circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx - circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy + circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx - circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle-> cx - circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy - circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx - circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy - circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy - circle->r));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx + circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy - circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx + circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy - circle->r * RSVG_ARC_MAGIC));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cx + circle->r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), circle->cy));

	g_string_append (d, " Z");

	rsvg_state_reinherit_top(ctx, self->state, dominate);
	rsvg_render_path (ctx, d->str);

	g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_circle (void)
{
	RsvgNodeCircle *circle;
	circle = g_new (RsvgNodeCircle, 1);
	circle->super.children = NULL;
	circle->super.state = g_new(RsvgState, 1);
	rsvg_state_init(circle->super.state);
	circle->super.type = RSVG_NODE_PATH;
	circle->super.free = rsvg_node_free;
	circle->super.draw = _rsvg_node_circle_draw;
	circle->super.set_atts = _rsvg_node_circle_set_atts;
	circle->cx = circle->cy = circle->r = 0;
	return &circle->super;
}

struct _RsvgNodeEllipse
{
	RsvgNode super;
	double cx, cy, rx, ry;
};

typedef struct _RsvgNodeEllipse RsvgNodeEllipse;

static void
_rsvg_node_ellipse_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	double font_size;
	RsvgNodeEllipse * ellipse = (RsvgNodeEllipse *)self;

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "cx")))
				ellipse->cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "cy")))
				ellipse->cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "rx")))
				ellipse->rx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "ry")))
				ellipse->ry = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "ellipse", klazz, id, atts);
		}	
}

static void
_rsvg_node_ellipse_draw(RsvgNode * self, RsvgDrawingCtx *ctx, 
						int dominate)
{
	RsvgNodeEllipse * ellipse = (RsvgNodeEllipse *)self;
	GString * d = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	
	if (ellipse->rx <= 0 || ellipse->ry <= 0)
		return;
	/* approximate an ellipse using 4 bezier curves */

	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy - RSVG_ARC_MAGIC * ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + RSVG_ARC_MAGIC * ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy - ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy - ellipse->ry));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx - RSVG_ARC_MAGIC * ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy - ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx - ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy - RSVG_ARC_MAGIC * ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx - ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx - ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy + RSVG_ARC_MAGIC * ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx - RSVG_ARC_MAGIC * ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy + ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy + ellipse->ry));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + RSVG_ARC_MAGIC * ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy + ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy + RSVG_ARC_MAGIC * ellipse->ry));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cx + ellipse->rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), ellipse->cy));

	g_string_append (d, " Z");
	
	rsvg_state_reinherit_top(ctx, self->state, dominate);
	rsvg_render_path (ctx, d->str);
	g_string_free (d, TRUE);
}

RsvgNode *
rsvg_new_ellipse (void)
{
	RsvgNodeEllipse *ellipse;
	ellipse = g_new (RsvgNodeEllipse, 1);
	ellipse->super.children = NULL;
	ellipse->super.state = g_new(RsvgState, 1);
	rsvg_state_init(ellipse->super.state);
	ellipse->super.type = RSVG_NODE_PATH;
	ellipse->super.free = rsvg_node_free;
	ellipse->super.draw = _rsvg_node_ellipse_draw;
	ellipse->super.set_atts = _rsvg_node_ellipse_set_atts;
	ellipse->cx = ellipse->cy = ellipse->rx = ellipse->ry = 0;
	return &ellipse->super;
}
