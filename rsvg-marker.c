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
rsvg_marker_free(RsvgDefVal* self)
{
	RsvgMarker *marker;
	marker = (RsvgMarker *)self;
	g_free(self);
}

void 
rsvg_start_marker (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *klazz = NULL, *id = NULL, *value;
	RsvgMarker *marker;
	double font_size;
	double x = 0., y = 0., w = 0., h = 0.;
	double vbx = 0., vby = 0., vbw = 1., vbh = 1.;
	gboolean obj_bbox = TRUE;
	RsvgState state;
	gboolean got_x, got_y, got_bbox, got_vbox, got_width, got_height;
	got_x = got_y = got_bbox = got_vbox = got_width = got_height = FALSE;	

	font_size = rsvg_state_current_font_size (ctx);
	marker = g_new (RsvgMarker, 1);
		
	rsvg_state_init(&state);

	marker->orient = 0;
	marker->orientAuto = FALSE;
	marker->overflow = FALSE;
	marker->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					got_vbox = rsvg_css_parse_vbox (value, &vbx, &vby,
													&vbw, &vbh);
				}
			if ((value = rsvg_property_bag_lookup (atts, "refX"))) {
				x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				got_x = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "refY"))) {
				y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				got_y = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "markerWidth"))) {
				w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				got_width = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "markerHeight"))) {
				h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				got_height = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "orient"))) {
				if (!strcmp (value, "auto"))
					marker->orientAuto = TRUE;
				else
					marker->orient = rsvg_css_parse_angle(value);
			}
			if ((value = rsvg_property_bag_lookup (atts, "markerUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_bbox = FALSE;
				else
					obj_bbox = TRUE;					
				got_bbox = TRUE;
			}	
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
				marker->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
			if ((value = rsvg_property_bag_lookup (atts, "overflow")))
				marker->overflow = rsvg_css_parse_overflow(value);
		}
	
	if (got_x)
		marker->refX = x;
	else
		marker->refX = 0;

	if (got_y)
		marker->refY = y;
	else
		marker->refY = 0;

	if (got_width)
		marker->width = w;
	else
		marker->width = 1;

	if (got_height)
		marker->height = h;
	else
		marker->height = 1;

	if (got_bbox)
		marker->bbox = obj_bbox;
	else
		marker->bbox = TRUE;

	if (got_vbox)
		{
			marker->vbx = vbx;
			marker->vby = vby;
			marker->vbw = vbw;
			marker->vbh = vbh;
			marker->vbox = TRUE;
			ctx->width = vbw;
			ctx->height = vbh;
		}
	else
		marker->vbox = FALSE;
	
	/* set up the defval stuff */
	marker->super.type = RSVG_DEF_MARKER;

	marker->contents =	(RsvgDefsDrawable *)rsvg_push_part_def_group(ctx, NULL, state);

	rsvg_state_init (&marker->contents->state);
	marker->super.free = rsvg_marker_free;

	rsvg_defs_set (ctx->defs, id, &marker->super);
}

static void
rsvg_state_reassemble(RsvgDefsDrawable * self, RsvgState * state)
{
	RsvgState store;
	if (self == NULL)
		{
			return;
		}
	rsvg_state_reassemble(self->parent, state);

	rsvg_state_clone (&store, &self->state);
	rsvg_state_reinherit(&store, state);
	rsvg_state_finalize(state);
	*state = store;
}

void 
rsvg_marker_render (RsvgMarker *self, gdouble x, gdouble y, gdouble orient, gdouble linewidth, RsvgDrawingCtx *ctx)
{
	gdouble affine[6];
	gdouble taffine[6];
	int i;
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

	/*don't inherit anything from the current context*/
	rsvg_state_finalize(state);
	rsvg_state_init(state);
	rsvg_state_reassemble((RsvgDefsDrawable *)self->contents, state);

	rsvg_state_push(ctx);
	state = rsvg_state_current(ctx);
	
	for (i = 0; i < 6; i++)
		{
			state->affine[i] = affine[i];
		}

	rsvg_defs_drawable_draw (self->contents, ctx, 3);
	
	rsvg_state_pop(ctx);
}

RsvgDefVal *
rsvg_marker_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
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
					
					if (val && val->type == RSVG_DEF_MARKER)
						return (RsvgDefVal *) val;
				}
		}
	return NULL;
}

