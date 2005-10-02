/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
rsvg_node_marker_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *klazz = NULL, *id = NULL, *value;
	RsvgMarker *marker;
	double font_size;
	font_size = rsvg_state_current_font_size (ctx);
	marker = (RsvgMarker *)self;
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, id, &marker->super);
				}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					marker->vbox = rsvg_css_parse_vbox (value, &marker->vbx, &marker->vby,
														&marker->vbw, &marker->vbh);
					if (marker->vbox)
						{						
							ctx->width = marker->vbw;
							ctx->height = marker->vbh;
						}
				}
			if ((value = rsvg_property_bag_lookup (atts, "refX")))
				marker->refX = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "refY")))
				marker->refY = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "markerWidth")))
				marker->width = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "markerHeight")))
				marker->height = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "orient"))) {
				if (!strcmp (value, "auto"))
					marker->orientAuto = TRUE;
				else
					marker->orient = rsvg_css_parse_angle(value);
			}
			if ((value = rsvg_property_bag_lookup (atts, "markerUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					marker->bbox = FALSE;
				if (!strcmp (value, "objectBoundingBox"))
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
	_rsvg_node_init(&marker->super);
	marker->orient = 0;
	marker->orientAuto = FALSE;
	marker->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	marker->refX = 0;
	marker->refY = 0;
	marker->width = 1;
	marker->height = 1;
	marker->bbox = TRUE;
	marker->vbox = FALSE;
	marker->super.type = RSVG_NODE_MARKER;
	marker->super.set_atts = rsvg_node_marker_set_atts;
	return &marker->super;
}

void 
rsvg_marker_render (RsvgMarker *self, gdouble x, gdouble y, gdouble orient, gdouble linewidth, RsvgDrawingCtx *ctx)
{
	gdouble affine[6];
	gdouble taffine[6];
	unsigned int i;
	gdouble rotation;
	RsvgState * state = rsvg_state_current(ctx);

	if (self->bbox) {
		_rsvg_affine_scale(affine,linewidth * state->affine[0], 
						 linewidth * state->affine[3]);
	} else {
		for (i = 0; i < 6; i++)
			affine[i] = state->affine[i];
	}	

	if (self->vbox) {

		double w, h, x, y;
		w = self->width;
		h = self->height;
		x = 0;
		y = 0;

		rsvg_preserve_aspect_ratio(self->preserve_aspect_ratio,
								   self->vbw, self->vbh, 
								   &w, &h, &x, &y);		

		x -= self->vbx / self->vbw;
		y -= self->vby / self->vbh;

		taffine[0] = w / self->vbw;
		taffine[1] = 0.;
		taffine[2] = 0.;
		taffine[3] = h / self->vbh;
		taffine[4] = x;
		taffine[5] = y;
		_rsvg_affine_multiply(affine, taffine, affine);		
	}

	_rsvg_affine_translate(taffine, -self->refX, -self->refY);

	_rsvg_affine_multiply(affine, taffine, affine);

	if (self->orientAuto)
		rotation = orient * 180. / M_PI;
	else
		rotation = self->orient;

	_rsvg_affine_rotate(taffine, rotation);
	
	_rsvg_affine_multiply(affine, affine, taffine);

	_rsvg_affine_translate(taffine, x, y);
	
	_rsvg_affine_multiply(affine, affine, taffine);

	rsvg_state_push(ctx);
	state = rsvg_state_current(ctx);

	rsvg_state_finalize(state);
	rsvg_state_init(state);

	rsvg_state_reconstruct(state, &self->super);
	
	for (i = 0; i < 6; i++)
		{
			state->affine[i] = affine[i];
		}

	for (i = 0; i < self->super.children->len; i++)
		{
			rsvg_state_push(ctx);

			rsvg_node_draw (g_ptr_array_index(self->super.children, i), 
									 ctx, 0);
	
			rsvg_state_pop(ctx);
		}		
	
	rsvg_state_pop(ctx);
}

RsvgNode *
rsvg_marker_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgNode *val;
			
			while (g_ascii_isspace (*p))
				p++;

			for (ix = 0; p[ix]; ix++)
				if (p[ix] == ')')
					break;
			
			if (p[ix] == ')')
				{
					name = g_strndup (p, ix);
					val = rsvg_defs_lookup (defs, name);
					g_free (name);
					
					if (val && val->type == RSVG_NODE_MARKER)
						return (RsvgNode *) val;
				}
		}
	return NULL;
}

void
rsvg_render_markers(const RsvgBpathDef * bpath_def, RsvgDrawingCtx *ctx)
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
			
			if(bpath_def->bpath[i + 1].code == RSVG_MOVETO || 
					bpath_def->bpath[i + 1].code == RSVG_MOVETO_OPEN || 
					bpath_def->bpath[i + 1].code == RSVG_END)
				{
					if (endmarker)
						rsvg_marker_render (endmarker, x, y, atan2(y - lasty, x - lastx), linewidth, ctx);
				}
			else if (bpath_def->bpath[i].code == RSVG_MOVETO || bpath_def->bpath[i].code == RSVG_MOVETO_OPEN)
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
