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
rsvg_node_drawable_path_free (RsvgNode *self)
{
	RsvgNodePath *z = (RsvgNodePath *)self;
	rsvg_state_finalize (z->super.state);
	g_free(z->super.state);
	g_free (z->d);
	g_free (z);
}

static void 
rsvg_node_drawable_path_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	RsvgNodePath *path = (RsvgNodePath*)self;

	rsvg_state_reinherit_top(ctx, self->state, dominate);

	rsvg_render_path (ctx, path->d);
	
}

void
rsvg_handle_path (RsvgHandle *ctx, const char * d, const char * id, RsvgState state)
{
	RsvgNodePath *path;
	
	path = g_new (RsvgNodePath, 1);
	path->d = g_strdup(d);
	path->super.state = g_new(RsvgState, 1);
	*path->super.state = state;
	path->super.type = RSVG_NODE_PATH;
	path->super.free = rsvg_node_drawable_path_free;
	path->super.draw = rsvg_node_drawable_path_draw;
	rsvg_defs_set (ctx->defs, id, &path->super);
	
	path->super.parent = (RsvgNode *)ctx->current_defs_group;
	if (path->super.parent != NULL)
		rsvg_node_drawable_group_pack((RsvgNodeGroup *)path->super.parent, 
									  &path->super);
}

void
rsvg_start_path (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value, *d = NULL;
	RsvgState state;
	rsvg_state_init(&state);

	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "d")))
				d = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "path", klazz, id, atts);
		}
	
	if (d == NULL)
		return;
	
	rsvg_handle_path (ctx, d, id, state);
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

static void
rsvg_start_any_poly(RsvgHandle *ctx, RsvgPropertyBag *atts, gboolean is_polyline)
{
	/* the only difference between polygon and polyline is
	   that a polyline closes the path */
	
	const char * verts = (const char *)NULL;
	GString * g = NULL;
	gchar ** pointlist = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	gsize pointlist_len = 0;
	RsvgState state;
	rsvg_state_init(&state);

	if (rsvg_property_bag_size (atts))
		{
			/* support for svg < 1.0 which used verts */
			if ((value = rsvg_property_bag_lookup (atts, "verts")) || (value = rsvg_property_bag_lookup (atts, "points")))
				verts = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, (is_polyline ? "polyline" : "polygon"), klazz, id, atts);
		}
	
	if (!verts)
		return;	
	
	/* todo: make the following more memory and CPU friendly */
	g = rsvg_make_poly_point_list (verts);
	pointlist = g_strsplit (g->str, " ", -1);
	g_string_free (g, TRUE);

	if (pointlist)
		{
			while(pointlist[pointlist_len] != NULL)
				pointlist_len++;
		}

	/* represent as a "moveto, lineto*, close" path */  
	if (pointlist_len >= 2)
		{
			gsize i;
			GString * d = g_string_sized_new (strlen(verts));
			g_string_append_printf (d, "M %s %s ", pointlist[0], pointlist[1] );
			
			for (i = 2; pointlist[i] != NULL && pointlist[i][0] != '\0'; i += 2)
				g_string_append_printf (d, "L %s %s ", pointlist[i], pointlist[i+1]);
			
			if (!is_polyline)
				g_string_append (d, "Z");
			
			rsvg_handle_path (ctx, d->str, id, state);
			g_string_free (d, TRUE);
		}
	
	if (pointlist)
		g_strfreev(pointlist);
}

void
rsvg_start_polygon (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	rsvg_start_any_poly (ctx, atts, FALSE);
}

void
rsvg_start_polyline (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	rsvg_start_any_poly (ctx, atts, TRUE);
}

void
rsvg_start_line (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x1 = 0, y1 = 0, x2 = 0, y2 = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;
	RsvgState state;
	rsvg_state_init(&state);

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x1")))
				x1 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y1")))
				y1 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "x2")))
				x2 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y2")))
				y2 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "line", klazz, id, atts);
		}
	
	/* emulate a line using a path */
	/* ("M %f %f L %f %f", x1, y1, x2, y2) */
	d = g_string_new ("M ");   

	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x1));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y1));
	g_string_append (d, " L ");	
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x2));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y2));    

	rsvg_handle_path (ctx, d->str, id, state);
	g_string_free (d, TRUE);
}

void
rsvg_start_rect (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x = 0., y = 0., w = 0, h = 0, rx = 0., ry = 0.;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	gboolean got_rx = FALSE, got_ry = FALSE;
	double font_size;
	RsvgState state;
	rsvg_state_init(&state);

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "rx"))) {
				rx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
				got_rx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "ry"))) {
				ry = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
				got_ry = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "rect", klazz, id, atts);
		}

	if (got_rx && !got_ry)
		ry = rx;
	else if (got_ry && !got_rx)
		rx = ry;	

	if (w == 0. || h == 0. || rx < 0. || ry < 0.)
		return;

	if (rx > fabs(w / 2.))
		rx = fabs(w / 2.);
	if (ry > fabs(h / 2.))
		ry = fabs(h / 2.);   
	
	/* incrementing y by 1 properly draws borders. this is a HACK */
	y += .01;
	
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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x+w));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y+ry));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y+h-ry));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y+ry));

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
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x+rx));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

	g_string_append (d, " Z");

	rsvg_handle_path (ctx, d->str, id, state);
	g_string_free (d, TRUE);
}

void
rsvg_start_circle (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double cx = 0, cy = 0, r = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;
	RsvgState state;
	rsvg_state_init(&state);	

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "cx")))
				cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "cy")))
				cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "r")))
				r = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (ctx), 
													  rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), 
													  font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "circle", klazz, id, atts);
		}
	
	if (r <= 0.)
		return;   
	
	/* approximate a circle using 4 bezier curves */

	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx+r));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cy));

	g_string_append (d, " C ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), cx+r));
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

	rsvg_handle_path (ctx, d->str, id, state);
	g_string_free (d, TRUE);
}

void
rsvg_start_ellipse (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double cx = 0, cy = 0, rx = 0, ry = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;
	RsvgState state;
	rsvg_state_init(&state);	

	font_size = rsvg_state_current_font_size (ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "cx")))
				cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "cy")))
				cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "rx")))
				rx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "ry")))
				ry = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
						id = value;

			rsvg_parse_style_attrs (ctx, &state, "ellipse", klazz, id, atts);
		}
	
	if (rx <= 0. || ry <= 0.)
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

	rsvg_handle_path (ctx, d->str, id, state);
	g_string_free (d, TRUE);
}
