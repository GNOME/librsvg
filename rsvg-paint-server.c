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

#include <glib/gmem.h>
#include <glib/gmessages.h>
#include <glib/gstrfuncs.h>
#include <libart_lgpl/art_affine.h>
#include <string.h>
#include <math.h>

#include "rsvg-css.h"

typedef struct _RsvgPaintServerSolid RsvgPaintServerSolid;
typedef struct _RsvgPaintServerLinGrad RsvgPaintServerLinGrad;
typedef struct _RsvgPaintServerRadGrad RsvgPaintServerRadGrad;

struct _RsvgPaintServer {
	int refcnt;
	void (*free) (RsvgPaintServer *self);
	void (*render) (RsvgPaintServer *self, ArtRender *ar, const RsvgPSCtx *ctx);
};

struct _RsvgPaintServerSolid {
	RsvgPaintServer super;
	guint32 rgb;
};

struct _RsvgPaintServerLinGrad {
	RsvgPaintServer super;
	RsvgLinearGradient *gradient;
	ArtGradientLinear *agl;
};

struct _RsvgPaintServerRadGrad {
	RsvgPaintServer super;
	RsvgRadialGradient *gradient;
	ArtGradientRadial *agr;
};

static void
rsvg_paint_server_solid_free (RsvgPaintServer *self)
{
	g_free (self);
}

static void
rsvg_paint_server_solid_render (RsvgPaintServer *self, ArtRender *ar,
								const RsvgPSCtx *ctx)
{
	RsvgPaintServerSolid *z = (RsvgPaintServerSolid *)self;
	guint32 rgb = z->rgb;
	ArtPixMaxDepth color[3];
	
	color[0] = ART_PIX_MAX_FROM_8 (rgb >> 16);
	color[1] = ART_PIX_MAX_FROM_8 ((rgb >> 8) & 0xff);
	color[2] = ART_PIX_MAX_FROM_8 (rgb  & 0xff);
	
	art_render_image_solid (ar, color);
}

static RsvgPaintServer *
rsvg_paint_server_solid (guint32 rgb)
{
	RsvgPaintServerSolid *result = g_new (RsvgPaintServerSolid, 1);
	
	result->super.refcnt = 1;
	result->super.free = rsvg_paint_server_solid_free;
	result->super.render = rsvg_paint_server_solid_render;
	
	result->rgb = rgb;
	
	return &result->super;
}

static void
rsvg_paint_server_lin_grad_free (RsvgPaintServer *self)
{
	RsvgPaintServerLinGrad *z = (RsvgPaintServerLinGrad *)self;
	
	if (z->agl)
		g_free (z->agl->stops);
	g_free (z->agl);
	g_free (self);
}

static ArtGradientStop *
rsvg_paint_art_stops_from_rsvg (RsvgGradientStops *rstops, 
								guint32 current_color)
{
	ArtGradientStop *stops;
	int n_stop = rstops->n_stop;
	int i;
	
	stops = g_new (ArtGradientStop, n_stop);
	for (i = 0; i < n_stop; i++)
		{
			guint32 rgba;
			guint32 r, g, b, a;
			
			stops[i].offset = rstops->stop[i].offset;
			if (!rstops->stop[i].is_current_color)
				rgba = rstops->stop[i].rgba;
			else
				rgba = current_color << 8;
			/* convert from separated to premultiplied alpha */
			a = rstops->stop[i].rgba & 0xff;
			r = (rgba >> 24) * a + 0x80;
			r = (r + (r >> 8)) >> 8;
			g = ((rgba >> 16) & 0xff) * a + 0x80;
			g = (g + (g >> 8)) >> 8;
			b = ((rgba >> 8) & 0xff) * a + 0x80;
			b = (b + (b >> 8)) >> 8;
			stops[i].color[0] = ART_PIX_MAX_FROM_8(r);
			stops[i].color[1] = ART_PIX_MAX_FROM_8(g);
			stops[i].color[2] = ART_PIX_MAX_FROM_8(b);
			stops[i].color[3] = ART_PIX_MAX_FROM_8(a);
		}
	return stops;
}

static void
rsvg_paint_server_lin_grad_render (RsvgPaintServer *self, ArtRender *ar,
								   const RsvgPSCtx *ctx)
{
	RsvgPaintServerLinGrad *z = (RsvgPaintServerLinGrad *)self;
	RsvgLinearGradient *rlg = z->gradient;
	ArtGradientLinear *agl;
	double x1, y1, x2, y2;
	double fx1, fy1, fx2, fy2;
	double dx, dy, scale;
	double affine[6];
	guint32 current_color;
	int i;
	float xchange, ychange, pointlen,unitlen;
	float nx2, ny2;
	float x0, y0;

	agl = z->agl;
	if (agl == NULL)
		{
			if (rlg->has_current_color)
				current_color = rlg->current_color;
			else
				current_color = ctx->color;
			if (rlg->stops->n_stop == 0)
				{
					return;
				}
			agl = g_new (ArtGradientLinear, 1);
			agl->n_stops = rlg->stops->n_stop;
			agl->stops = rsvg_paint_art_stops_from_rsvg (rlg->stops, current_color);
			z->agl = agl;
		}


	if (rlg->obj_bbox) {
		affine[0] = ctx->x1 - ctx->x0;
		affine[1] = 0.;		
		affine[2] = 0.;
		affine[3] = ctx->y1 - ctx->y0;
		affine[4] = ctx->x0;
		affine[5] = ctx->y0;

	} else {
		for (i = 0; i < 6; i++)
			affine[i] = ctx->affine[i];
	}

	fx1 = rlg->x1 * rlg->affine[0] + rlg->y1 * rlg->affine[2] + rlg->affine[4];
	fy1 = rlg->x1 * rlg->affine[1] + rlg->y1 * rlg->affine[3] + rlg->affine[5];
	fx2 = rlg->x2 * rlg->affine[0] + rlg->y2 * rlg->affine[2] + rlg->affine[4];
	fy2 = rlg->x2 * rlg->affine[1] + rlg->y2 * rlg->affine[3] + rlg->affine[5];

	xchange = fx2 - fx1;
	ychange = fy2 - fy1;

	nx2 = fx1 - ychange;
	ny2 = fy1 + xchange;

	/* compute [xy][12] in pixel space */
	x1 = fx1 * affine[0] + fy1 * affine[2] + affine[4];
	y1 = fx1 * affine[1] + fy1 * affine[3] + affine[5];
	x0 = fx2 * affine[0] + fy2 * affine[2] + affine[4];
	y0 = fx2 * affine[1] + fy2 * affine[3] + affine[5];
	x2 = nx2 * affine[0] + ny2 * affine[2] + affine[4];
	y2 = nx2 * affine[1] + ny2 * affine[3] + affine[5];

	pointlen = abs((x2 - x1)*(y1 - y0)  - (x1 - x0)*(y2 - y1)) / 
		sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1));

	xchange = x2 - x1;
	ychange = y2 - y1;
	unitlen = sqrt(xchange*xchange + ychange*ychange);

	x2 = x1 + ychange / unitlen * pointlen;
	y2 = y1 - xchange / unitlen * pointlen;

	/* solve a, b, c so ax1 + by1 + c = 0 and ax2 + by2 + c = 1, maximum
	   gradient is in x1,y1 to x2,y2 dir */
	dx = x2 - x1;
	dy = y2 - y1;

	/* workaround for an evil devide by 0 bug - not sure if this is sufficient */
	if (fabs(dx) + fabs(dy) <= 0.0000001)
		scale = 0.;
	else
		scale = 1.0 / (dx * dx + dy * dy);
	agl->a = dx * scale;
	agl->b = dy * scale;
	agl->c = -(x1 * agl->a + y1 * agl->b);
	
	agl->spread = rlg->spread;
	art_render_gradient_linear (ar, agl, ART_FILTER_NEAREST);
}

static RsvgPaintServer *
rsvg_paint_server_lin_grad (RsvgLinearGradient *gradient)
{
	RsvgPaintServerLinGrad *result = g_new (RsvgPaintServerLinGrad, 1);
	
	result->super.refcnt = 1;
	result->super.free = rsvg_paint_server_lin_grad_free;
	result->super.render = rsvg_paint_server_lin_grad_render;
	
	result->gradient = gradient;
	result->agl = NULL;
	
	return &result->super;
}

static void
rsvg_paint_server_rad_grad_free (RsvgPaintServer *self)
{
	RsvgPaintServerRadGrad *z = (RsvgPaintServerRadGrad *)self;

	if (z->agr)
		g_free (z->agr->stops);
	g_free (z->agr);
	g_free (self);
}

static void
rsvg_paint_server_rad_grad_render (RsvgPaintServer *self, ArtRender *ar,
								   const RsvgPSCtx *ctx)
{
	RsvgPaintServerRadGrad *z = (RsvgPaintServerRadGrad *)self;
	RsvgRadialGradient *rrg = z->gradient;
	ArtGradientRadial *agr;
	double aff1[6], aff2[6], affine[6];
	guint32 current_color;
	int i;

	if (rrg->obj_bbox) {
		affine[0] = ctx->x1 - ctx->x0;
		affine[1] = 0.;		
		affine[2] = 0.;
		affine[3] = ctx->y1 - ctx->y0;
		affine[4] = ctx->x0;
		affine[5] = ctx->y0;

	} else {
		for (i = 0; i < 6; i++)
			affine[i] = ctx->affine[i];
	}

	art_affine_multiply(affine, rrg->affine, affine);

	agr = z->agr;
	if (agr == NULL)
		{
			if (rrg->has_current_color)
				current_color = rrg->current_color;
			else
				current_color = ctx->color;
			if (rrg->stops->n_stop == 0)
				{
					return;
				}
			agr = g_new (ArtGradientRadial, 1);
			agr->n_stops = rrg->stops->n_stop;
			agr->stops = rsvg_paint_art_stops_from_rsvg (rrg->stops, current_color);
			z->agr = agr;
		}
	
	art_affine_scale (aff1, rrg->r, rrg->r);
	art_affine_translate (aff2, rrg->cx, rrg->cy);
	art_affine_multiply (aff1, aff1, aff2);
	art_affine_multiply (aff1, aff1, affine);
	art_affine_invert (agr->affine, aff1);
	
	/* todo: libart doesn't support spreads on radial gradients */

	agr->fx = (rrg->fx - rrg->cx) / rrg->r;
	agr->fy = (rrg->fy - rrg->cy) / rrg->r;
	
	art_render_gradient_radial (ar, agr, ART_FILTER_NEAREST);
}

static RsvgPaintServer *
rsvg_paint_server_rad_grad (RsvgRadialGradient *gradient)
{
	RsvgPaintServerRadGrad *result = g_new (RsvgPaintServerRadGrad, 1);
	
	result->super.refcnt = 1;
	result->super.free = rsvg_paint_server_rad_grad_free;
	result->super.render = rsvg_paint_server_rad_grad_render;
	
	result->gradient = gradient;
	result->agr = NULL;
	
	return &result->super;
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
rsvg_paint_server_parse (RsvgPaintServer * current, const RsvgDefs *defs, const char *str,
						 guint32 current_color)
{
	guint32 rgb;
	
	if (!strcmp (str, "none"))
		return NULL;

	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p)) p++;
			if (*p != '#')
				return NULL;
			p++;
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
				default:
					return NULL;
				}
		}
	else if (current && !strcmp (str, "inherit"))
		{
			rsvg_paint_server_ref (current);
			return current;
		}
	else
	  {
		  if (!strcmp (str, "currentColor"))
			  rgb = current_color;
		  else
			  rgb = rsvg_css_parse_color (str, 0);

		  return rsvg_paint_server_solid (rgb);
	  }
}

/**
 * rsvg_render_paint_server: Render paint server as image source for libart.
 * @ar: Libart render object.
 * @ps: Paint server object.
 *
 * Hooks up @ps as an image source for a libart rendering operation.
 **/
void
rsvg_render_paint_server (ArtRender *ar, RsvgPaintServer *ps,
						  const RsvgPSCtx *ctx)
{
	g_return_if_fail (ar != NULL);
	if (ps != NULL)
		ps->render (ps, ar, ctx);
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
		ps->free (ps);
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
