/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-paint-server.c: Implement the SVG paint server abstraction.
 
   Copyright (C) 2000 Eazel, Inc.
  
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

#include "config.h"
#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "rsvg-paint-server.h"
#include "rsvg-styles.h"
#include "rsvg-image.h"

#include <glib/gmem.h>
#include <glib/gmessages.h>
#include <glib/gstrfuncs.h>
#include <string.h>
#include <math.h>

#include "rsvg-css.h"

static RsvgPaintServer *
rsvg_paint_server_solid (guint32 rgb)
{
	RsvgPaintServer *result = g_new (RsvgPaintServer, 1);
	
	result->refcnt = 1;
	result->type = RSVG_PAINT_SERVER_SOLID;
	result->core.colour = g_new(RsvgSolidColour, 1);
	result->core.colour->rgb = rgb;
	result->core.colour->currentcolour = FALSE;
	
	return result;
}

static RsvgPaintServer *
rsvg_paint_server_solid_current_colour ()
{
	RsvgPaintServer *result = g_new (RsvgPaintServer, 1);
	
	result->refcnt = 1;
	result->type = RSVG_PAINT_SERVER_SOLID;
	result->core.colour = g_new(RsvgSolidColour, 1);
	result->core.colour->currentcolour = TRUE;
	
	return result;
}

static RsvgPaintServer *
rsvg_paint_server_lin_grad (RsvgLinearGradient *gradient)
{
	RsvgPaintServer *result = g_new (RsvgPaintServer, 1);
	
	result->refcnt = 1;
	result->type = RSVG_PAINT_SERVER_LIN_GRAD;
	result->core.lingrad = gradient;
	
	return result;
}

static RsvgPaintServer *
rsvg_paint_server_rad_grad (RsvgRadialGradient *gradient)
{
	RsvgPaintServer *result = g_new (RsvgPaintServer, 1);
	
	result->refcnt = 1;
	result->type = RSVG_PAINT_SERVER_RAD_GRAD;
	result->core.radgrad = gradient;
	
	return result;
}

static RsvgPaintServer *
rsvg_paint_server_pattern (RsvgPattern *pattern)
{
	RsvgPaintServer *result = g_new (RsvgPaintServer, 1);
	
	result->refcnt = 1;
	result->type = RSVG_PAINT_SERVER_PATTERN;
	result->core.pattern = pattern;
	
	return result;
}

/**
 * rsvg_paint_server_parse: Parse an SVG paint specification.
 * @defs: Defs for looking up gradients.
 * @str: The SVG paint specification string to parse.
 *
 * Parses the paint specification @str, creating a new paint server
 * object.
 *
 * Return value: The newly created paint server, or NULL on error.
 **/
RsvgPaintServer *
rsvg_paint_server_parse (gboolean * inherit, const RsvgDefs *defs, const char *str,
						 guint32 current_color)
{
	guint32 rgb;
	if (inherit != NULL)
		*inherit = 1;
	if (!strcmp (str, "none"))
		return NULL;

	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p)) p++;
			for (ix = 0; p[ix]; ix++)
				if (p[ix] == ')') break;
			if (p[ix] != ')')
				return NULL;
			name = g_strndup (p, ix);
			val = rsvg_defs_lookup (defs, name);
			g_free (name);
			if (val == NULL)
				return NULL;
			switch (val->type)
				{
				case RSVG_DEF_LINGRAD:
					return rsvg_paint_server_lin_grad ((RsvgLinearGradient *)val);
				case RSVG_DEF_RADGRAD:
					return rsvg_paint_server_rad_grad ((RsvgRadialGradient *)val);
				case RSVG_DEF_PATTERN:
					return rsvg_paint_server_pattern ((RsvgPattern *)val);
				default:
					return NULL;
				}
		}
	else if (!strcmp (str, "inherit"))
		{
			if (inherit != NULL)
				*inherit = 0;
			return rsvg_paint_server_solid (0);
		}
	else if (!strcmp (str, "currentColor"))
		{	
			RsvgPaintServer * ps;			
			ps = rsvg_paint_server_solid_current_colour ();
			return ps;
		}
	else
		{
			rgb = rsvg_css_parse_color (str, inherit);
			return rsvg_paint_server_solid (rgb);
		}
}

/**
 * rsvg_paint_server_ref: Reference a paint server object.
 * @ps: The paint server object to reference.
 **/
void
rsvg_paint_server_ref (RsvgPaintServer *ps)
{
	if (ps == NULL)
		return;
	ps->refcnt++;
}

/**
 * rsvg_paint_server_unref: Unreference a paint server object.
 * @ps: The paint server object to unreference.
 **/
void
rsvg_paint_server_unref (RsvgPaintServer *ps)
{
	if (ps == NULL)
		return;
	if (--ps->refcnt == 0)
		{
			if (ps->type == RSVG_PAINT_SERVER_SOLID)
				g_free(ps->core.colour);
			g_free (ps);
		}
}

RsvgRadialGradient *
rsvg_clone_radial_gradient (const RsvgRadialGradient *grad, gboolean * shallow_cloned)
{
	RsvgRadialGradient * clone = NULL;
	int i;
	
	clone = g_new0 (RsvgRadialGradient, 1);
	clone->super.type = RSVG_DEF_RADGRAD;
	clone->super.free = rsvg_radial_gradient_free;
	
	clone->obj_bbox = grad->obj_bbox;
	for (i = 0; i < 6; i++)
		clone->affine[i] = grad->affine[i];

	if (grad->stops != NULL) {
		clone->stops = g_new (RsvgGradientStops, 1);
		clone->stops->n_stop = grad->stops->n_stop;
		clone->stops->stop = g_new (RsvgGradientStop, grad->stops->n_stop);
	
		for (i = 0; i < grad->stops->n_stop; i++)
			clone->stops->stop[i] = grad->stops->stop[i];
	}

	clone->spread = grad->spread;

	/* EVIL EVIL - SVG can base LinearGradients on
	   RadialGradients, and vice-versa. it is legal, though:
	   http://www.w3.org/TR/SVG11/pservers.html#LinearGradients
	*/
	if (grad->super.type == RSVG_DEF_RADGRAD) {
		clone->cx = grad->cx;
		clone->cy = grad->cy;
		clone->r  = grad->r;
		clone->fx = grad->fx;
		clone->fy = grad->fy;
		
		*shallow_cloned = FALSE;
	} else {
		*shallow_cloned = TRUE;
	}
	
	return clone;
}

RsvgLinearGradient *
rsvg_clone_linear_gradient (const RsvgLinearGradient *grad, gboolean * shallow_cloned)
{
	RsvgLinearGradient * clone = NULL;
	int i;
	
	clone = g_new0 (RsvgLinearGradient, 1);
	clone->super.type = RSVG_DEF_LINGRAD;
	clone->super.free = rsvg_linear_gradient_free;
	
	clone->obj_bbox = grad->obj_bbox;
	for (i = 0; i < 6; i++)
		clone->affine[i] = grad->affine[i];

	if (grad->stops != NULL) {
		clone->stops = g_new (RsvgGradientStops, 1);
		clone->stops->n_stop = grad->stops->n_stop;
		clone->stops->stop = g_new (RsvgGradientStop, grad->stops->n_stop);
		
		for (i = 0; i < grad->stops->n_stop; i++)
			clone->stops->stop[i] = grad->stops->stop[i];
	}

	clone->spread = grad->spread;

	/* EVIL EVIL - SVG can base LinearGradients on
	   RadialGradients, and vice-versa. it is legal, though:
	   http://www.w3.org/TR/SVG11/pservers.html#LinearGradients
	*/
	if (grad->super.type == RSVG_DEF_LINGRAD) {
		clone->x1 = grad->x1;
		clone->y1 = grad->y1;
		clone->x2 = grad->x2;
		clone->y2 = grad->y2;

		*shallow_cloned = FALSE;
	} else {
		*shallow_cloned = TRUE;
	}

	return clone;
}

RsvgPattern *
rsvg_clone_pattern (const RsvgPattern *pattern)
{
	RsvgPattern * clone = NULL;
	int i;
	
	clone = g_new0 (RsvgPattern, 1);
	clone->super.type = RSVG_DEF_PATTERN;
	clone->super.free = rsvg_pattern_free;
	
	clone->obj_bbox = pattern->obj_bbox;
	clone->obj_cbbox = pattern->obj_cbbox;
	clone->vbox = pattern->vbox;
	for (i = 0; i < 6; i++)
		clone->affine[i] = pattern->affine[i];

	if (((RsvgDefsDrawableGroup *)pattern->g)->children->len ||
		pattern->gfallback == NULL)
		clone->gfallback = pattern->g;
	else
		clone->gfallback = pattern->gfallback;		

	clone->x = pattern->x;
	clone->y = pattern->y;
	clone->width = pattern->width;
	clone->height = pattern->height;
	clone->vbx = pattern->vbx;
	clone->vby = pattern->vby;
	clone->vbw = pattern->vbw;	
	clone->vbh = pattern->vbh;

	return clone;
}
