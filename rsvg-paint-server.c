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

static void rsvg_linear_gradient_free (RsvgNode *self);
static void rsvg_radial_gradient_free (RsvgNode *self);
static void rsvg_pattern_free (RsvgNode *self);

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
			RsvgNode *val;
			
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
				case RSVG_NODE_LINGRAD:
					return rsvg_paint_server_lin_grad ((RsvgLinearGradient *)val);
				case RSVG_NODE_RADGRAD:
					return rsvg_paint_server_rad_grad ((RsvgRadialGradient *)val);
				case RSVG_NODE_PATTERN:
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
	clone->super.type = RSVG_NODE_RADGRAD;
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
	if (grad->super.type == RSVG_NODE_RADGRAD) {
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
	clone->super.type = RSVG_NODE_LINGRAD;
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
	if (grad->super.type == RSVG_NODE_LINGRAD) {
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
	clone->super.type = RSVG_NODE_PATTERN;
	clone->super.free = rsvg_pattern_free;
	
	clone->obj_bbox = pattern->obj_bbox;
	clone->obj_cbbox = pattern->obj_cbbox;
	clone->vbox = pattern->vbox;
	for (i = 0; i < 6; i++)
		clone->affine[i] = pattern->affine[i];

	if (((RsvgNodeGroup *)pattern->g)->children->len ||
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

typedef struct _RsvgSaxHandlerGstops {
	RsvgSaxHandler super;
	RsvgSaxHandler *parent;
	RsvgHandle *ctx;
	RsvgGradientStops *stops;
	const char * parent_tag;
} RsvgSaxHandlerGstops;

static void
rsvg_gradient_stop_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_gradient_stop_handler_start (RsvgSaxHandler *self, const xmlChar *name,
								  RsvgPropertyBag *atts)
{
	RsvgSaxHandlerGstops *z = (RsvgSaxHandlerGstops *)self;
	RsvgGradientStops *stops = z->stops;
	double offset = 0;
	gboolean got_offset = FALSE;
	RsvgState state;
	int n_stop;
	gboolean is_current_color = FALSE;
	const char *value;
	
	if (strcmp ((char *)name, "stop"))
		return;
	
	rsvg_state_init(&state);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "offset")))
				{
					/* either a number [0,1] or a percentage */
					offset = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (z->ctx), 1., 0.);
					
					if (offset < 0.)
						offset = 0.;
					else if (offset > 1.)
						offset = 1.;
					
					got_offset = TRUE;
				}
			if ((value = rsvg_property_bag_lookup (atts, "style")))
				rsvg_parse_style (z->ctx, &state, value);
			
			if ((value = rsvg_property_bag_lookup (atts, "stop-color")))
				if (!strcmp(value, "currentColor"))
					is_current_color = TRUE;
			rsvg_parse_style_pairs (z->ctx, &state, atts);
		}
	
	rsvg_state_finalize(&state);
	
	if (!got_offset)
		{
			g_warning (_("gradient stop must specify offset\n"));
			return;
		}
	
	n_stop = stops->n_stop++;
	if (n_stop == 0)
		stops->stop = g_new (RsvgGradientStop, 1);
	else if (!(n_stop & (n_stop - 1)))
		/* double the allocation if size is a power of two */
		stops->stop = g_renew (RsvgGradientStop, stops->stop, n_stop << 1);
	stops->stop[n_stop].offset = offset;
	stops->stop[n_stop].is_current_color = is_current_color;
	stops->stop[n_stop].rgba = (state.stop_color << 8) | state.stop_opacity;
}

static void
rsvg_gradient_stop_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerGstops *z = (RsvgSaxHandlerGstops *)self;
	RsvgHandle *ctx = z->ctx;
	RsvgSaxHandler *prev = z->parent;
	
	if (!strcmp((char *)name, z->parent_tag))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = prev;
				}
		}
}

static RsvgSaxHandler *
rsvg_gradient_stop_handler_new_clone (RsvgHandle *ctx, RsvgGradientStops *stops, 
									  const char * parent)
{
	RsvgSaxHandlerGstops *gstops = g_new0 (RsvgSaxHandlerGstops, 1);
	
	gstops->super.free = rsvg_gradient_stop_handler_free;
	gstops->super.start_element = rsvg_gradient_stop_handler_start;
	gstops->super.end_element = rsvg_gradient_stop_handler_end;
	gstops->ctx = ctx;
	gstops->stops = stops;
	gstops->parent_tag = parent;
	
	gstops->parent = ctx->handler;
	return &gstops->super;
}

static RsvgSaxHandler *
rsvg_gradient_stop_handler_new (RsvgHandle *ctx, RsvgGradientStops **p_stops,
								const char * parent)
{
	RsvgSaxHandlerGstops *gstops = g_new0 (RsvgSaxHandlerGstops, 1);
	RsvgGradientStops *stops = g_new (RsvgGradientStops, 1);
	
	gstops->super.free = rsvg_gradient_stop_handler_free;
	gstops->super.start_element = rsvg_gradient_stop_handler_start;
	gstops->super.end_element = rsvg_gradient_stop_handler_end;
	gstops->ctx = ctx;
	gstops->stops = stops;
	gstops->parent_tag = parent;
	
	stops->n_stop = 0;
	stops->stop = NULL;
	
	gstops->parent = ctx->handler;
	*p_stops = stops;
	return &gstops->super;
}

static void
rsvg_linear_gradient_free (RsvgNode *self)
{
	RsvgLinearGradient *z = (RsvgLinearGradient *)self;
	
	g_free (z->stops->stop);
	g_free (z->stops);
	g_free (self);
}

void
rsvg_start_linear_gradient (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgState state;
	RsvgLinearGradient *grad = NULL;
	const char *id = NULL, *value;
	double x1 = 0., y1 = 0., x2 = 0., y2 = 0.;
	RsvgGradientSpread spread = RSVG_GRADIENT_PAD;
	const char * xlink_href = NULL;
	gboolean obj_bbox = TRUE;
	gboolean got_x1, got_x2, got_y1, got_y2, got_spread, got_transform, got_bbox, cloned, shallow_cloned;
	double affine[6];
	guint32 color = 0;
	gboolean got_color = FALSE;
	int i;

	rsvg_state_init(&state);

	got_x1 = got_x2 = got_y1 = got_y2 = got_spread = got_transform = got_bbox = cloned = shallow_cloned = FALSE;
		
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "x1"))) {
				x1 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_x1 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y1"))) {
				y1 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_y1 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "x2"))) {
				x2 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_x2 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y2"))) {
				y2 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_y2 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "spreadMethod")))
				{
					if (!strcmp (value, "pad")) {
						spread = RSVG_GRADIENT_PAD;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "reflect")) {
						spread = RSVG_GRADIENT_REFLECT;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "repeat")) {
						spread = RSVG_GRADIENT_REPEAT;
						got_spread = TRUE;
					}
				}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				xlink_href = value;
			if ((value = rsvg_property_bag_lookup (atts, "gradientTransform")))
				got_transform = rsvg_parse_transform (affine, value);
			if ((value = rsvg_property_bag_lookup (atts, "color")))
				{
					got_color = TRUE;
					color = rsvg_css_parse_color (value, 0);
				}
			if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_bbox = FALSE;
				got_bbox = TRUE;

			}
			rsvg_parse_style_pairs (ctx, &state, atts);
		}
	
	/* set up 100% as the default if not gotten */
	if (!got_x2) {
		if (obj_bbox)
			x2 = 1.0;
		else
			x2 = rsvg_css_parse_normalized_length ("100%", ctx->dpi_x, (gdouble)ctx->width, state.font_size);
	}

	if (xlink_href != NULL)
		{
			RsvgLinearGradient * parent = (RsvgLinearGradient*)rsvg_defs_lookup (ctx->defs, xlink_href);
			if (parent != NULL)
				{
					cloned = TRUE;
					grad = rsvg_clone_linear_gradient (parent, &shallow_cloned); 
					ctx->handler = rsvg_gradient_stop_handler_new_clone (ctx, grad->stops, "linearGradient");
				}
		}
	
	if (!cloned)
		{
			grad = g_new (RsvgLinearGradient, 1);
			grad->super.type = RSVG_NODE_LINGRAD;
			grad->super.free = rsvg_linear_gradient_free;
			ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops, "linearGradient");
		}
	
	rsvg_defs_set (ctx->defs, id, &grad->super);
	
	if (got_transform)
		for (i = 0; i < 6; i++)
			grad->affine[i] = affine[i];
	else
		_rsvg_affine_identity(grad->affine);

	if (got_color)
		{
			grad->current_color = color;
			grad->has_current_color = TRUE;
		}
	else
		{
			grad->has_current_color = FALSE;
		}

	/* gradient inherits parent/cloned information unless it's explicity gotten */
	grad->obj_bbox = (cloned && !got_bbox) ? grad->obj_bbox : obj_bbox;
	if (!shallow_cloned)
		{
			grad->x1 = (cloned && !got_x1) ? grad->x1 : x1;
			grad->y1 = (cloned && !got_y1) ? grad->y1 : y1;
			grad->x2 = (cloned && !got_x2) ? grad->x2 : x2;
			grad->y2 = (cloned && !got_y2) ? grad->y2 : y2;
		}
	else
		{
			grad->x1 = x1;
			grad->y1 = y1;
			grad->x2 = x2;
			grad->y2 = y2;
		}
	grad->spread = (cloned && !got_spread) ? grad->spread : spread;
}

/* exported to the paint server via rsvg-private.h */
static void
rsvg_radial_gradient_free (RsvgNode *self)
{
	RsvgRadialGradient *z = (RsvgRadialGradient *)self;
	
	g_free (z->stops->stop);
	g_free (z->stops);
	g_free (self);
}

void
rsvg_start_radial_gradient (RsvgHandle *ctx, RsvgPropertyBag *atts, const char * tag) /* tag for conicalGradient */
{
	RsvgState state;
	RsvgRadialGradient *grad = NULL;
	const char *id = NULL;
	double cx = 0., cy = 0., r = 0., fx = 0., fy = 0.;  
	const char * xlink_href = NULL, *value;
	RsvgGradientSpread spread = RSVG_GRADIENT_PAD;
	gboolean obj_bbox = TRUE;
	gboolean got_cx, got_cy, got_r, got_fx, got_fy, got_spread, got_transform, got_bbox, cloned, shallow_cloned;
	guint32 color = 0;
	gboolean got_color = FALSE;
	double affine[6];
	int i;

	rsvg_state_init(&state);

	got_cx = got_cy = got_r = got_fx = got_fy = got_spread = got_transform = got_bbox = cloned = shallow_cloned = FALSE;
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "cx"))) {
				cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_cx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "cy"))) {
				cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_cy = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "r"))) {
				r = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (ctx), 1, 
													  state.font_size);
				got_r = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fx"))) {
				fx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_fx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fy"))) {
				fy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_fy = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				xlink_href = value;
			if ((value = rsvg_property_bag_lookup (atts, "gradientTransform"))) {
				got_transform = rsvg_parse_transform (affine, value);
			}
			if ((value = rsvg_property_bag_lookup (atts, "color")))
				{
					got_color = TRUE;
					color = rsvg_css_parse_color (value, 0);
				}
			if ((value = rsvg_property_bag_lookup (atts, "spreadMethod")))
				{
					if (!strcmp (value, "pad")) {
						spread = RSVG_GRADIENT_PAD;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "reflect")) {
						spread = RSVG_GRADIENT_REFLECT;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "repeat")) {
						spread = RSVG_GRADIENT_REPEAT;
						got_spread = TRUE;
					}
				}
			if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_bbox = FALSE;
				got_bbox = TRUE;
			}
			rsvg_parse_style_pairs (ctx, &state, atts);
		}
	
	if (xlink_href != NULL)
		{
			RsvgRadialGradient * parent = (RsvgRadialGradient*)rsvg_defs_lookup (ctx->defs, xlink_href);
			if (parent != NULL)
				{
					cloned = TRUE;
					grad = rsvg_clone_radial_gradient (parent, &shallow_cloned); 
					ctx->handler = rsvg_gradient_stop_handler_new_clone (ctx, grad->stops, tag);
				}
    }
	if (!cloned)
		{
			grad = g_new (RsvgRadialGradient, 1);
			grad->super.type = RSVG_NODE_RADGRAD;
			grad->super.free = rsvg_radial_gradient_free;
			ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops, tag);		   
		}

	/* setup defaults */
	if (!got_cx) {
		if (obj_bbox)
			cx = 0.5;
		else
			cx = rsvg_css_parse_normalized_length ("50%", ctx->dpi_x, (gdouble)ctx->width, state.font_size);
	}
	if (!got_cy) {
		if (obj_bbox)
			cy = 0.5;
		else
			cy = rsvg_css_parse_normalized_length ("50%", ctx->dpi_y, (gdouble)ctx->height, state.font_size);
	}
	if (!got_r) {
		if (obj_bbox)
			r = 0.5;
		else
			r  = rsvg_css_parse_normalized_length ("50%", rsvg_dpi_percentage (ctx), rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), state.font_size);
	}
	if (!got_fx) {
		fx = cx;
	}
	if (!got_fy) {
		fy = cy;
	}
	
	rsvg_defs_set (ctx->defs, id, &grad->super);

	if (got_transform)
		for (i = 0; i < 6; i++)
			grad->affine[i] = affine[i];
	else
		_rsvg_affine_identity(grad->affine);
	
	if (got_color)
		{
			grad->current_color = color;
			grad->has_current_color = TRUE;
		}
	else
		{
			grad->has_current_color = FALSE;
		}

	/* gradient inherits parent/cloned information unless it's explicity gotten */
	grad->obj_bbox = (cloned && !got_bbox) ? grad->obj_bbox : obj_bbox;
	if (!shallow_cloned)
		{
			grad->cx = (cloned && !got_cx) ? grad->cx : cx;
			grad->cy = (cloned && !got_cy) ? grad->cy : cy;
			grad->r =  (cloned && !got_r)  ? grad->r  : r;
			grad->fx = (cloned && !got_fx) ? grad->fx : fx;
			grad->fy = (cloned && !got_fy) ? grad->fy : fy;
		}
	else
		{
			grad->cx = cx;
			grad->cy = cy;
			grad->r = r;
			grad->fx = fx;
			grad->fy = fy;
		}
	grad->spread = (cloned && !got_spread) ? grad->spread : spread;
}

static void
rsvg_pattern_free (RsvgNode *self)
{
	RsvgPattern *z = (RsvgPattern *)self;
	
	g_free (z);
}

void
rsvg_start_pattern (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgState state;
	RsvgPattern *pattern = NULL;
	const char *id = NULL, *value;
	double x = 0., y = 0., width = 0., height = 0.;
	double vbx = 0., vby = 0., vbw = 1., vbh = 1.;
	const char * xlink_href = NULL;
	gboolean obj_bbox = TRUE;
	gboolean obj_cbbox = FALSE;
	gboolean got_x, got_y, got_width, got_height, got_transform, got_bbox, got_cbbox, cloned, got_vbox, got_aspect_ratio;
	double affine[6];
	int i;
	guint aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;

	rsvg_state_init(&state);

	got_x = got_y = got_width = got_height = got_transform = got_bbox = got_cbbox = cloned = got_vbox = got_aspect_ratio = FALSE;
		
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					got_vbox = rsvg_css_parse_vbox (value, &vbx, &vby,
													&vbw, &vbh);
				}
			if ((value = rsvg_property_bag_lookup (atts, "x"))) {
				x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_x = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y"))) {
				y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_y = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "width"))) {
				width = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, state.font_size);
				got_width = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "height"))) {
				height = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, state.font_size);
				got_height = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				xlink_href = value;
			if ((value = rsvg_property_bag_lookup (atts, "patternTransform")))
				got_transform = rsvg_parse_transform (affine, value);
			if ((value = rsvg_property_bag_lookup (atts, "patternUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_bbox = FALSE;
				else
					obj_bbox = TRUE;
				got_bbox = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "patternContentUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_cbbox = FALSE;
				else
					obj_cbbox = TRUE;					
				got_cbbox = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
				aspect_ratio = rsvg_css_parse_aspect_ratio (value);


		}

	if (xlink_href != NULL)
		{
			RsvgPattern * parent = (RsvgPattern*)rsvg_defs_lookup (ctx->defs, xlink_href);
			if (parent != NULL)
				{
					cloned = TRUE;
					pattern = rsvg_clone_pattern (parent);
				}
		}
	
	if (!cloned)
		{
			pattern = g_new (RsvgPattern, 1);
			pattern->super.type = RSVG_NODE_PATTERN;
			pattern->super.free = rsvg_pattern_free;
			pattern->gfallback = NULL;
		}
	
	rsvg_defs_set (ctx->defs, id, &pattern->super);
	
	if (got_transform)
		for (i = 0; i < 6; i++)
			pattern->affine[i] = affine[i];
	else
		_rsvg_affine_identity(pattern->affine);

	if (got_aspect_ratio)
		pattern->preserve_aspect_ratio = aspect_ratio;
	else
		pattern->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;

	/* gradient inherits parent/cloned information unless it's explicity gotten */
	pattern->obj_bbox = (cloned && !got_bbox) ? pattern->obj_bbox : obj_bbox;
	pattern->obj_cbbox = (cloned && !got_cbbox) ? pattern->obj_cbbox : obj_cbbox;
	pattern->x = (cloned && !got_x) ? pattern->x : x;
	pattern->y = (cloned && !got_y) ? pattern->y : y;
	pattern->width = (cloned && !got_width) ? pattern->width : width;
	pattern->height = (cloned && !got_height) ? pattern->height : height;
	pattern->vbx = (cloned && !got_vbox) ? pattern->vbx : vbx;
	pattern->vby = (cloned && !got_vbox) ? pattern->vby : vby;
	pattern->vbw = (cloned && !got_vbox) ? pattern->vbw : vbw;
	pattern->vbh = (cloned && !got_vbox) ? pattern->vbh : vbh;
	pattern->vbox = (cloned && !got_vbox) ? pattern->vbox : got_vbox;

	pattern->g = (rsvg_push_part_def_group (ctx, NULL, &state));
}

