/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-shapes.c: Draw shapes with libart

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

#include <libart_lgpl/art_vpath_bpath.h>
#include <libart_lgpl/art_render_svp.h>
#include <libart_lgpl/art_svp_vpath.h>
#include <libart_lgpl/art_rgb_affine.h>
#include <libart_lgpl/art_rgb_rgba_affine.h>
#include <libart_lgpl/art_rgb_svp.h>
#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_svp_intersect.h>
#include <libart_lgpl/art_svp_ops.h>

#include "rsvg-art-draw.h"
#include "rsvg-art-composite.h"
#include "rsvg-art-render.h"
#include "rsvg-paint-server.h"
#include "rsvg-styles.h"
#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-marker.h"

#include <math.h>

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
struct _RsvgFRect
{
	double x0;
	double y0;
	double x1;
	double y1;
};
typedef struct _RsvgFRect RsvgFRect;

static RsvgFRect
rsvg_calculate_svp_bounds (const ArtSVP *svp, double * useraffine)
{
	int i, j;
	float x, y;
	double affine[6];
	float bigx, littlex, bigy, littley, assignedonce;
	RsvgFRect output;

	art_affine_invert(affine, useraffine);
	bigx = littlex = bigy = littley = assignedonce = 0;	

	for (i = 0; i < svp->n_segs; i++)
		for (j = 0; j < svp->segs[i].n_points; j++)
			{
				x = svp->segs[i].points[j].x * affine[0] + 
					svp->segs[i].points[j].y * affine[2] +
					affine[4];
				y = svp->segs[i].points[j].x * affine[1] + 
					svp->segs[i].points[j].y * affine[3] +
					affine[5];
				if (!assignedonce)
					{
						bigx = x;
						littlex = x;
						bigy = y; 
						littley = y;
						assignedonce = 1;
					}
				if (x > bigx)
					bigx = x;
				if (x < littlex)
					littlex = x;
				if (y > bigy)
					bigy = y; 
				if (y < littley)
					littley = y;
			}
	output.x0 = littlex;
	output.y0 = littley;
	output.x1 = bigx;
	output.y1 = bigy;
	return output;
}

static ArtIRect rsvg_frect_pixelspaceise(RsvgFRect input, double * affine)
{
	ArtIRect temprect;	
	int i, j, basex, basey;
	int assignedonce = 0;
	float x, y;
	
	for (i = 0; i < 2; i++)
		for (j = 0; j < 2; j++)
			{
				x = i ? input.x0 : input.x1;
				y = j ? input.y0 : input.y1;
				basex = affine[0] * x + affine[2] * y + affine[4];
				basey = affine[1] * x + affine[3] * y + affine[5];
				if (assignedonce)
					{
						temprect.x0 = MIN(basex, temprect.x0);
						temprect.y0 = MIN(basey, temprect.y0);
						temprect.x1 = MAX(basex, temprect.x1);
						temprect.y1 = MAX(basey, temprect.y1);
					}
				else
					{	
						temprect.x1 = temprect.x0 = basex;
						temprect.y1 = temprect.y0 = basey;
						assignedonce = 1;
					}
			}
	return temprect;
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
rsvg_render_svp (RsvgDrawingCtx *ctx, ArtSVP *svp,
				 RsvgPaintServer *ps, int opacity)
{
	GdkPixbuf *pixbuf;
	ArtRender *render;
	gboolean has_alpha;
	RsvgFRect temprect;
	ArtIRect temptemprect;
	RsvgPSCtx gradctx;
	RsvgState *state;
	int i;	

	rsvg_state_clip_path_assure(ctx);

	pixbuf = ((RsvgArtRender *)ctx->render)->pixbuf;
	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	state = rsvg_state_current(ctx);

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
	
	temprect = rsvg_calculate_svp_bounds(svp, state->affine);
	
	if (state->clippath != NULL)
		{
		  ArtSVP * svpx;
		  svpx = art_svp_intersect(svp, state->clippath);
			svp = svpx;
		}
	
	art_render_svp (render, svp);
	art_render_mask_solid (render, (opacity << 8) + opacity + (opacity >> 7));

	temptemprect = rsvg_frect_pixelspaceise(temprect, state->affine);
	art_irect_union(&ctx->bbox, &ctx->bbox, &temptemprect);

	gradctx.x0 = temprect.x0;
	gradctx.y0 = temprect.y0;
	gradctx.x1 = temprect.x1;
	gradctx.y1 = temprect.y1;
	gradctx.ctx = ctx;

	for (i = 0; i < 6; i++)
		gradctx.affine[i] = state->affine[i];
	
	gradctx.color = state->current_color;
	rsvg_render_paint_server (render, ps, &gradctx);
	art_render_invoke (render);

	if (state->clippath != NULL) /*we don't need svpx any more*/
		art_free(svp);
}

static ArtSVP *
rsvg_render_filling (RsvgState *state, const ArtVpath *vpath)
{
	ArtVpath *closed_vpath;
	ArtSVP *svp2, *svp;
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
	
	return svp2;
}

static ArtSVP *
rsvg_render_outline (RsvgState *state, ArtVpath *vpath)
{
	ArtSVP * output;

	/* todo: libart doesn't yet implement anamorphic scaling of strokes */
	double stroke_width = state->stroke_width *
		art_affine_expansion (state->affine);

	if (stroke_width < 0.25)
		stroke_width = 0.25;
	
	/* if the path is dashed, stroke it */
	if (state->dash.n_dash > 0) 
		{
			ArtVpath * dashed_vpath = art_vpath_dash (vpath, &state->dash);
			vpath = dashed_vpath;
		}
	
	output = art_svp_vpath_stroke (vpath, state->join, state->cap,
								   stroke_width, state->miter_limit, 0.25);

	if (state->dash.n_dash > 0) 
		art_free (vpath);
	return output;
}

static void
rsvg_render_bpath (RsvgDrawingCtx *ctx, const ArtBpath *bpath)
{
	RsvgState *state;
	ArtBpath *affine_bpath;
	ArtVpath *vpath;
	ArtSVP *svp;
	GdkPixbuf *pixbuf;
	gboolean need_tmpbuf;
	int opacity;
	int tmp;

	pixbuf = ((RsvgArtRender *)ctx->render)->pixbuf;
	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	state = rsvg_state_current (ctx);

	/* todo: handle visibility stuff earlier for performance benefits 
	 * handles all path based shapes. will handle text and images separately
	 */
	if (!state->visible || !state->cond_true)
		return;

	affine_bpath = art_bpath_affine_transform (bpath,
											   state->affine);
	
	vpath = art_bez_path_to_vec (affine_bpath, 0.25);
	art_free (affine_bpath);
	
	need_tmpbuf = ((state->fill != NULL) && (state->stroke != NULL) &&
				   state->opacity != 0xff) || rsvg_art_needs_discrete_layer(state);
	
	if (need_tmpbuf)
		rsvg_push_discrete_layer (ctx);
	
	if (state->fill != NULL)
		{

			opacity = state->fill_opacity;
			if (!need_tmpbuf && state->opacity != 0xff)
				{
					tmp = opacity * state->opacity + 0x80;
					opacity = (tmp + (tmp >> 8)) >> 8;
				}
			svp = rsvg_render_filling(state, vpath);
			rsvg_render_svp (ctx, svp, state->fill, opacity);
			art_svp_free (svp);
		}
	
	if (state->stroke != NULL)
		{
			opacity = state->stroke_opacity;
			if (!need_tmpbuf && state->opacity != 0xff)
				{
					tmp = opacity * state->opacity + 0x80;
					opacity = (tmp + (tmp >> 8)) >> 8;
				}
			svp = rsvg_render_outline(state, vpath);
			rsvg_render_svp (ctx, svp, state->stroke, opacity);
			art_svp_free (svp);
		}

	if (need_tmpbuf)
		rsvg_pop_discrete_layer (ctx);	
	
	art_free (vpath);
}

static ArtSVP *
rsvg_render_bpath_into_svp (RsvgDrawingCtx *ctx, const ArtBpath *bpath)
{
	RsvgState *state;
	ArtBpath *affine_bpath;
	ArtVpath *vpath;
	ArtSVP *svp;
	
	state = rsvg_state_current (ctx);

	affine_bpath = art_bpath_affine_transform (bpath, state->affine);

	vpath = art_bez_path_to_vec (affine_bpath, 0.25);
	art_free (affine_bpath);
	state->fill_rule = state->clip_rule;

	svp = rsvg_render_filling(state, vpath);

	art_free (vpath);
	return svp;
}

static void
rsvg_render_markers(RsvgBpathDef * bpath_def, RsvgDrawingCtx *ctx)
{
	int i;

	double x, y;
	double lastx, lasty;
	double nextx, nexty;	
	double linewidth;

	RsvgState * state;
	RsvgMarker * startmarker;
	RsvgMarker * middlemarker;
	RsvgMarker * endmarker;

	state = rsvg_state_current(ctx);
	
	linewidth = state->stroke_width;
	startmarker = (RsvgMarker *)state->startMarker;
	middlemarker = (RsvgMarker *)state->middleMarker;
	endmarker = (RsvgMarker *)state->endMarker;

	if (!startmarker && !middlemarker && !endmarker)
		return;

	x = 0;
	y = 0;
	nextx = state->affine[0] * bpath_def->bpath[0].x3 + 
		state->affine[2] * bpath_def->bpath[0].y3 + state->affine[4];
	nexty = state->affine[1] * bpath_def->bpath[0].x3 + 
		state->affine[3] * bpath_def->bpath[0].y3 + state->affine[5];

	for (i = 0; i < bpath_def->n_bpath - 1; i++)
		{
			lastx = x;
			lasty = y;
			x = nextx;
			y = nexty;
			nextx = state->affine[0] * bpath_def->bpath[i + 1].x3 + 
				state->affine[2] * bpath_def->bpath[i + 1].y3 + state->affine[4];
			nexty = state->affine[1] * bpath_def->bpath[i + 1].x3 + 
				state->affine[3] * bpath_def->bpath[i + 1].y3 + state->affine[5];

			
			if(bpath_def->bpath[i + 1].code == ART_MOVETO || 
					bpath_def->bpath[i + 1].code == ART_MOVETO_OPEN || 
					bpath_def->bpath[i + 1].code == ART_END)
				{
					if (endmarker)
						rsvg_marker_render (endmarker, x, y, atan2(y - lasty, x - lastx), linewidth, ctx);
				}
			else if (bpath_def->bpath[i].code == ART_MOVETO || bpath_def->bpath[i].code == ART_MOVETO_OPEN)
				{		
					if (startmarker)
						rsvg_marker_render (startmarker, x, y, atan2(nexty - y, nextx - x), linewidth, ctx);
				}
			else
				{			
					if (middlemarker)
						{
							double xdifin, ydifin, xdifout, ydifout, intot, outtot, angle;
							
							xdifin = x - lastx;
							ydifin = y - lasty;
							xdifout = nextx - x;
							ydifout = nexty - y;
							
							intot = sqrt(xdifin * xdifin + ydifin * ydifin);
							outtot = sqrt(xdifout * xdifout + ydifout * ydifout);
							
							xdifin /= intot;
							ydifin /= intot;
							xdifout /= outtot;
							ydifout /= outtot;
							
							angle = atan2((ydifin + ydifout) / 2, (xdifin + xdifout) / 2);
							rsvg_marker_render (middlemarker, x, y, angle, linewidth, ctx);
						}
				}
		}
}

void
rsvg_art_render_path(RsvgDrawingCtx *ctx, const char *d)
{
	RsvgBpathDef *bpath_def;
	
	bpath_def = rsvg_parse_path (d);
	rsvg_bpath_def_art_finish (bpath_def);
	
	rsvg_render_bpath (ctx, bpath_def->bpath);
	
	rsvg_render_markers(bpath_def, ctx);

	rsvg_bpath_def_free (bpath_def);
}

void
rsvg_art_svp_render_path (RsvgDrawingCtx *ctx, const char *d)
{
	RsvgBpathDef *bpath_def;
	RsvgArtSVPRender *render = (RsvgArtSVPRender *)ctx->render;
	ArtSVP *svp2, *svp3;

	bpath_def = rsvg_parse_path (d);
	rsvg_bpath_def_art_finish (bpath_def);
	
	svp2 = 	rsvg_render_bpath_into_svp (ctx, bpath_def->bpath);

	if (render->outline != NULL)
		{
			svp3 = art_svp_union(svp2, render->outline);
			art_free(render->outline);
			render->outline = svp3;
		}
	else
		render->outline = svp2;

	rsvg_bpath_def_free (bpath_def);
}
