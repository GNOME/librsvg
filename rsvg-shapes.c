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

   Author: Raph Levien <raph@artofcode.com>
*/
#include <string.h>
#include <math.h>

#include "rsvg-shapes.h"
#include "rsvg-styles.h"
#include "rsvg-css.h"
#include "rsvg-private.h"
#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-defs.h"

#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_vpath_bpath.h>
#include <libart_lgpl/art_render_svp.h>
#include <libart_lgpl/art_svp_vpath.h>
#include <libart_lgpl/art_svp_intersect.h>
#include <libart_lgpl/art_svp_vpath.h>
#include <libart_lgpl/art_rgb_affine.h>
#include <libart_lgpl/art_rgb_rgba_affine.h>

/* 4/3 * (1-cos 45)/sin 45 = 4/3 * sqrt(2) - 1 */
#define RSVG_ARC_MAGIC ((double) 0.5522847498)

struct _RsvgDefsPath {
 	RsvgDefVal super;
	RsvgState  state;
 	char       *d;
};
 
typedef struct _RsvgDefsPath RsvgDefsPath;

/**
 * rsvg_close_vpath: Close a vector path.
 * @src: Source vector path.
 *
 * Closes any open subpaths in the vector path.
 *
 * Return value: Closed vector path, allocated with g_new.
 **/
static ArtVpath *
rsvg_close_vpath (const ArtVpath *src)
{
	ArtVpath *result;
	int n_result, n_result_max;
	int src_ix;
	double beg_x, beg_y;
	gboolean open;
	
	n_result = 0;
	n_result_max = 16;
	result = g_new (ArtVpath, n_result_max);
	
	beg_x = 0;
	beg_y = 0;
	open = FALSE;
	
	for (src_ix = 0; src[src_ix].code != ART_END; src_ix++)
		{
			if (n_result == n_result_max)
				result = g_renew (ArtVpath, result, n_result_max <<= 1);
			result[n_result].code = src[src_ix].code == ART_MOVETO_OPEN ?
				ART_MOVETO : src[src_ix].code;
			result[n_result].x = src[src_ix].x;
			result[n_result].y = src[src_ix].y;
			n_result++;
			if (src[src_ix].code == ART_MOVETO_OPEN)
				{
					beg_x = src[src_ix].x;
					beg_y = src[src_ix].y;
					open = TRUE;
				}
			else if (src[src_ix + 1].code != ART_LINETO)
				{
					if (open && (beg_x != src[src_ix].x || beg_y != src[src_ix].y))
						{
							if (n_result == n_result_max)
								result = g_renew (ArtVpath, result, n_result_max <<= 1);
							result[n_result].code = ART_LINETO;
							result[n_result].x = beg_x;
							result[n_result].y = beg_y;
							n_result++;
						}
					open = FALSE;
				}
		}
	if (n_result == n_result_max)
		result = g_renew (ArtVpath, result, n_result_max <<= 1);
	result[n_result].code = ART_END;
	result[n_result].x = 0.0;
	result[n_result].y = 0.0;
	return result;
}

/* calculates how big an svp is */
static ArtIRect
rsvg_calculate_svp_bounds (const ArtSVP *svp)
{
	int i, j;	
	int bigx, littlex, bigy, littley, assignedonce;
	ArtIRect output;

	bigx = littlex = bigy = littley = assignedonce = 0;	

	for (i = 0; i < svp->n_segs; i++)
		for (j = 0; j < svp->segs[i].n_points; j++)
			{
				if (!assignedonce)
					{
						bigx = svp->segs[i].points[j].x;
						littlex = svp->segs[i].points[j].x;
						bigy = svp->segs[i].points[j].y; 
						littley = svp->segs[i].points[j].y;
						assignedonce = 1;
					}
				if (svp->segs[i].points[j].x > bigx)
					bigx = svp->segs[i].points[j].x;
				if (svp->segs[i].points[j].x < littlex)
					littlex = svp->segs[i].points[j].x;
				if (svp->segs[i].points[j].y > bigy)
					bigy = svp->segs[i].points[j].y; 
				if (svp->segs[i].points[j].y < littley)
					littley = svp->segs[i].points[j].y;
			}
	output.x0 = littlex;
	output.y0 = littley;
	output.x1 = bigx;
	output.y1 = bigy;
	return output;
}

/**
 * rsvg_render_svp: Render an SVP.
 * @ctx: Context in which to render.
 * @svp: SVP to render.
 * @ps: Paint server for rendering.
 * @opacity: Opacity as 0..0xff.
 *
 * Renders the SVP over the pixbuf in @ctx.
 **/
static void
rsvg_render_svp (RsvgHandle *ctx, const ArtSVP *svp,
				 RsvgPaintServer *ps, int opacity)
{
	GdkPixbuf *pixbuf;
	ArtRender *render;
	gboolean has_alpha;
	ArtIRect temprect;
	RsvgPSCtx gradctx;
	
	pixbuf = ctx->pixbuf;
	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	has_alpha = gdk_pixbuf_get_has_alpha (pixbuf);
	
	render = art_render_new (0, 0,
							 gdk_pixbuf_get_width (pixbuf),
							 gdk_pixbuf_get_height (pixbuf),
							 gdk_pixbuf_get_pixels (pixbuf),
							 gdk_pixbuf_get_rowstride (pixbuf),
							 gdk_pixbuf_get_n_channels (pixbuf) -
							 (has_alpha ? 1 : 0),
							 gdk_pixbuf_get_bits_per_sample (pixbuf),
							 has_alpha ? ART_ALPHA_SEPARATE : ART_ALPHA_NONE,
							 NULL);
	
	art_render_svp (render, svp);
	art_render_mask_solid (render, (opacity << 8) + opacity + (opacity >> 7));

	temprect = rsvg_calculate_svp_bounds(svp);

	gradctx.x0 = temprect.x0;
	gradctx.y0 = temprect.y0;
	gradctx.x1 = temprect.x1;
	gradctx.y1 = temprect.y1;
	gradctx.width = ctx->width;
	gradctx.height = ctx->height;

	rsvg_render_paint_server (render, ps, &gradctx);
	art_render_invoke (render);
}

static void
rsvg_render_bpath (RsvgHandle *ctx, const ArtBpath *bpath)
{
	RsvgState *state;
	ArtBpath *affine_bpath;
	ArtVpath *vpath;
	ArtSVP *svp;
	GdkPixbuf *pixbuf;
	gboolean need_tmpbuf;
	int opacity;
	int tmp;

	pixbuf = ctx->pixbuf;
	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	state = rsvg_state_current (ctx);

	/* todo: handle visibility stuff earlier for performance benefits 
	 * handles all path based shapes. will handle text and images separately
	 */
	if (!state->visible)
		return;

	affine_bpath = art_bpath_affine_transform (bpath,
											   state->affine);
	
	vpath = art_bez_path_to_vec (affine_bpath, 0.25);
	art_free (affine_bpath);
	
	need_tmpbuf = (state->fill != NULL) && (state->stroke != NULL) &&
		state->opacity != 0xff;
	
	if (need_tmpbuf)
		rsvg_push_opacity_group (ctx);
	
	if (state->fill != NULL)
		{
			ArtVpath *closed_vpath;
			ArtSVP *svp2;
			ArtSvpWriter *swr;
			
			closed_vpath = rsvg_close_vpath (vpath);
			svp = art_svp_from_vpath (closed_vpath);
			g_free (closed_vpath);
			
			if (state->fill_rule == FILL_RULE_EVENODD)
				swr = art_svp_writer_rewind_new (ART_WIND_RULE_ODDEVEN);
			else /* state->fill_rule == FILL_RULE_NONZERO */
				swr = art_svp_writer_rewind_new (ART_WIND_RULE_NONZERO);

			art_svp_intersector (svp, swr);
			
			svp2 = art_svp_writer_rewind_reap (swr);
			art_svp_free (svp);
			
			opacity = state->fill_opacity;
			if (!need_tmpbuf && state->opacity != 0xff)
				{
					tmp = opacity * state->opacity + 0x80;
					opacity = (tmp + (tmp >> 8)) >> 8;
				}
			rsvg_render_svp (ctx, svp2, state->fill, opacity);
			art_svp_free (svp2);
		}
	
	if (state->stroke != NULL)
		{
			/* todo: libart doesn't yet implement anamorphic scaling of strokes */
			double stroke_width = state->stroke_width *
				art_affine_expansion (state->affine);
			
			if (stroke_width < 0.25)
				stroke_width = 0.25;
			
			/* if the path is dashed, stroke it */
			if (state->dash.n_dash > 0) 
				{
					ArtVpath * dashed_vpath = art_vpath_dash (vpath, &state->dash);
					art_free (vpath);
					vpath = dashed_vpath;
				}
			
			svp = art_svp_vpath_stroke (vpath, state->join, state->cap,
										stroke_width, state->miter_limit, 0.25);
			opacity = state->stroke_opacity;
			if (!need_tmpbuf && state->opacity != 0xff)
				{
					tmp = opacity * state->opacity + 0x80;
					opacity = (tmp + (tmp >> 8)) >> 8;
				}
			rsvg_render_svp (ctx, svp, state->stroke, opacity);
			art_svp_free (svp);
		}
	
	if (need_tmpbuf)
		rsvg_pop_opacity_group (ctx, state->opacity);
	
	art_free (vpath);
}

void
rsvg_render_path(RsvgHandle *ctx, const char *d)
{
	RsvgBpathDef *bpath_def;
	
	bpath_def = rsvg_parse_path (d);
	rsvg_bpath_def_art_finish (bpath_def);
	
	rsvg_render_bpath (ctx, bpath_def->bpath);
	
	rsvg_bpath_def_free (bpath_def);
}

static void 
rsvg_defs_path_free (RsvgDefVal *self)
{
	RsvgDefsPath *z = (RsvgDefsPath *)self;
	rsvg_state_finalize (&z->state);
	g_free (z);
}

/**
 * Todo: Possibly publicly export this (for text)
 */
static void
rsvg_handle_path (RsvgHandle *ctx, const char * d, const char * id)
{
	if (!ctx->in_defs)
		rsvg_render_path (ctx, d);
	else {
		RsvgDefsPath *path;

		if (id == NULL) 
			return;

		path = g_new (RsvgDefsPath, 1);
		path->d = g_strdup(d);
		rsvg_state_clone (&path->state, rsvg_state_current (ctx));
		path->super.type = RSVG_DEF_PATH;
		path->super.free = rsvg_defs_path_free;
		
		rsvg_defs_set (ctx->defs, id, &path->super);
	}
}

void
rsvg_start_path (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	char *d = NULL;
	const char * klazz = NULL, * id = NULL;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "d"))
						d = (char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}
	
	if (d == NULL)
		return;
	
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), "path", klazz, id, atts);
	rsvg_handle_path (ctx, d, id);
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
rsvg_start_any_poly(RsvgHandle *ctx, const xmlChar **atts, gboolean is_polyline)
{
	/* the only difference between polygon and polyline is
	   that a polyline closes the path */
	
	int i;
	const char * verts = (const char *)NULL;
	GString * g = NULL;
	gchar ** pointlist = NULL;
	const char * klazz = NULL, * id = NULL;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					/* support for svg < 1.0 which used verts */
					if (!strcmp ((char *)atts[i], "verts") || !strcmp ((char *)atts[i], "points"))
						verts = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}
	
	if (!verts)
		return;
	
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), (is_polyline ? "polyline" : "polygon"), klazz, id, atts);
	
	/* todo: make the following more memory and CPU friendly */
	g = rsvg_make_poly_point_list (verts);
	pointlist = g_strsplit (g->str, " ", -1);
	g_string_free (g, TRUE);

	/* represent as a "moveto, lineto*, close" path */  
	if (pointlist)
		{
			GString * d = g_string_sized_new (strlen(verts));
			g_string_append_printf (d, "M %s %s ", pointlist[0], pointlist[1] );
			
			for (i = 2; pointlist[i] != NULL && pointlist[i][0] != '\0'; i += 2)
				g_string_append_printf (d, "L %s %s ", pointlist[i], pointlist[i+1]);
			
			if (!is_polyline)
				g_string_append (d, "Z");
			
			g_strfreev(pointlist);
			rsvg_handle_path (ctx, d->str, id);
			g_string_free (d, TRUE);
		}
}

void
rsvg_start_polygon (RsvgHandle *ctx, const xmlChar **atts)
{
	rsvg_start_any_poly (ctx, atts, FALSE);
}

void
rsvg_start_polyline (RsvgHandle *ctx, const xmlChar **atts)
{
	rsvg_start_any_poly (ctx, atts, TRUE);
}

void
rsvg_start_line (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double x1 = 0, y1 = 0, x2 = 0, y2 = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "x1"))
						x1 = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "y1"))
						y1 = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					if (!strcmp ((char *)atts[i], "x2"))
						x2 = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "y2"))
						y2 = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}      
		}
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), "line", klazz, id, atts);
	
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

	rsvg_handle_path (ctx, d->str, id);
	g_string_free (d, TRUE);
}

void
rsvg_start_rect (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double x = 0., y = 0., w = 0, h = 0, rx = 0., ry = 0.;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	gboolean got_rx = FALSE, got_ry = FALSE;
	double font_size;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "x"))
						x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "y"))
						y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "width"))
						w = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "height"))
						h = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "rx")) {
						rx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						got_rx = TRUE;
					}
					else if (!strcmp ((char *)atts[i], "ry")) {
						ry = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
						got_ry = TRUE;
					}
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
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
	
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), "rect", klazz, id, atts);
	
	/* incrementing y by 1 properly draws borders. this is a HACK */
	y += 1.;
	
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

	rsvg_handle_path (ctx, d->str, id);
	g_string_free (d, TRUE);
}

void
rsvg_start_circle (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double cx = 0, cy = 0, r = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;
	
	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "cx"))
						cx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "cy"))
						cy = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "r"))
						r = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, 
															  rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), 
															  font_size);
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}
	
	if (r <= 0.)
		return;
	
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), "circle", klazz, id, atts);
	
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

	rsvg_handle_path (ctx, d->str, id);
	g_string_free (d, TRUE);
}

void
rsvg_start_ellipse (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double cx = 0, cy = 0, rx = 0, ry = 0;
	GString * d = NULL;
	const char * klazz = NULL, * id = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];
	double font_size;
	
	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "cx"))
						cx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "cy"))
						cy = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "rx"))
						rx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "ry"))
						ry = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, font_size);
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}
	
	if (rx <= 0. || ry <= 0.)
		return;
	
	rsvg_parse_style_attrs (ctx, rsvg_state_current (ctx), "ellipse", klazz, id, atts);
	
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

	rsvg_handle_path (ctx, d->str, id);
	g_string_free (d, TRUE);
}

/* TODO 1: issue with affining alpha images - this is gdkpixbuf's fault...
 * TODO 2: issue with rotating images - do we want to rotate the whole
 *         canvas 2x to get this right, only to have #1 bite us?
 */
void
rsvg_start_image (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double x = 0., y = 0., w = -1., h = -1.;
	const char * href = NULL;
	const char * klazz = NULL, * id = NULL;
	
	GdkPixbuf *img;
	GError *err = NULL;
	
	gboolean has_alpha;
	guchar *rgb = NULL;
	int dest_rowstride;
	double tmp_affine[6];
	RsvgState *state;

	/* skip over defs entries for now */
	if (ctx->in_defs) return;

	state = rsvg_state_current (ctx);
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "x"))
						x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
					else if (!strcmp ((char *)atts[i], "y"))
						y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
					else if (!strcmp ((char *)atts[i], "width"))
						w = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
					else if (!strcmp ((char *)atts[i], "height"))
						h = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
					/* path is used by some older adobe illustrator versions */
					else if (!strcmp ((char *)atts[i], "path") || !strcmp((char *)atts[i], "xlink:href"))
						href = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}
	
	if (!href || x < 0. || y < 0. || w <= 0. || h <= 0.)
		return;
	
	rsvg_parse_style_attrs (ctx, state, "image", klazz, id, atts);
	
	/* figure out if image is visible or not */
	if (!state->visible)
		return;

	img = gdk_pixbuf_new_from_file (href, &err);
	
	if (!img)
		{
			if (err)
				{
					g_warning ("Couldn't load pixbuf (%s): %s\n", href, err->message);
					g_error_free (err);
				}
			return;
		}
	
	/* scale/resize the dest image */
	art_affine_scale (tmp_affine, (double)w / (double)gdk_pixbuf_get_width (img),
					  (double)h / (double)gdk_pixbuf_get_height (img));
	art_affine_multiply (state->affine, tmp_affine, state->affine);
	
	has_alpha = gdk_pixbuf_get_has_alpha (img);
	dest_rowstride = (int)(w * (has_alpha ? 4 : 3) + 3) & ~3;
	rgb = g_new (guchar, h * dest_rowstride);
	
	if(has_alpha)
		art_rgb_rgba_affine (rgb, 0, 0, w, h, dest_rowstride,
							 gdk_pixbuf_get_pixels (img),
							 gdk_pixbuf_get_width (img),
							 gdk_pixbuf_get_height (img),
							 gdk_pixbuf_get_rowstride (img),
							 state->affine,
							 ART_FILTER_NEAREST,
							 NULL);
	else
		art_rgb_affine (rgb, 0, 0, w, h, dest_rowstride,
						gdk_pixbuf_get_pixels (img),
						gdk_pixbuf_get_width (img),
						gdk_pixbuf_get_height (img),
						gdk_pixbuf_get_rowstride (img),
						state->affine,
						ART_FILTER_NEAREST,
						NULL);
	
	g_object_unref (G_OBJECT (img));
	img = gdk_pixbuf_new_from_data (rgb, GDK_COLORSPACE_RGB, has_alpha, 8, w, h, dest_rowstride, NULL, NULL);
	
	if (!img)
		{
			g_free (rgb);
			return;
		}
	
	gdk_pixbuf_copy_area (img, 0, 0,
						  gdk_pixbuf_get_width (img) * state->affine[0],
						  gdk_pixbuf_get_height (img) * state->affine[3],
						  ctx->pixbuf, 
						  state->affine[4] + x,
						  state->affine[5] + y);
	
	g_object_unref (G_OBJECT (img));
	g_free (rgb);
}

void 
rsvg_start_use (RsvgHandle *ctx, const xmlChar **atts)
{
	RsvgState *state = rsvg_state_current (ctx);
	const char * klazz = NULL, *id = NULL, *xlink_href = NULL;
	double x = 0, y = 0, width = 0, height = 0;
	int i;
	double affine[6];
	gboolean got_width = FALSE, got_height = FALSE;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "x"))
						x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
					else if (!strcmp ((char *)atts[i], "y"))
						y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
					else if (!strcmp ((char *)atts[i], "width")) {
						width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
						got_width = TRUE;
					}
					else if (!strcmp ((char *)atts[i], "height")) {
						height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);					
						got_height = TRUE;
					}
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "xlink:href"))
						xlink_href = (const char *)atts[i + 1];
				}
		}
	
	/* < 0 is an error, 0 disables rendering. TODO: handle positive values correctly */
	if (got_width || got_height)
		if (width <= 0. || height <= 0.)
			return;
	
	if (xlink_href != NULL)
		{
			RsvgDefVal * parent = rsvg_defs_lookup (ctx->defs, xlink_href+1);
			if (parent != NULL)
				switch(parent->type)
					{
					case RSVG_DEF_PATH:
						{
							RsvgDefsPath *path = (RsvgDefsPath*)parent;

							/* combine state definitions */
							rsvg_state_clone (state, &path->state);
							art_affine_translate (affine, x, y);
							art_affine_multiply (state->affine, affine, state->affine);

							rsvg_parse_style_attrs (ctx, state, "use", klazz, id, atts);
							if (state->opacity != 0xff)
								rsvg_push_opacity_group (ctx);

							/* always want to render inside of a <use/> */
							rsvg_render_path (ctx, path->d);
							break;
						}
					default:
						g_warning ("Unhandled defs entry/type %s %d\n", id, parent->type);
						return;
					}
		}
}
