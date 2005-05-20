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

static void 
rsvg_pserver_free (RsvgNode *self)
{
	g_ptr_array_free(self->children, TRUE);
	g_free (self);
}

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
	rsvg_pattern_fix_fallback(pattern);
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

static void
rsvg_stop_set_atts (RsvgNode *self, RsvgHandle *ctx,
					RsvgPropertyBag *atts)
{
	double offset = 0;
	RsvgState state;
	const char *value;
	RsvgGradientStop * stop;

	rsvg_state_init(&state);
	stop = (RsvgGradientStop *)self;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "offset")))
				{
					/* either a number [0,1] or a percentage */
					offset = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (ctx), 1., 0.);
					
					if (offset < 0.)
						offset = 0.;
					else if (offset > 1.)
						offset = 1.;
					stop->offset = offset;
				}
			if ((value = rsvg_property_bag_lookup (atts, "style")))
				rsvg_parse_style (ctx, &state, value);
			
			if ((value = rsvg_property_bag_lookup (atts, "stop-color")))
				if (!strcmp(value, "currentColor"))
					stop->is_current_color = TRUE;
			rsvg_parse_style_pairs (ctx, &state, atts);
		}
	stop->rgba = (state.stop_color << 8) | state.stop_opacity;	
	rsvg_state_finalize(&state);
}

static void
rsvg_stop_free(RsvgNode * self)
{
	g_free(self);
}

RsvgNode *
rsvg_new_stop (void)
{
	RsvgGradientStop * stop = g_new(RsvgGradientStop, 1);
	stop->super.free = rsvg_stop_free;
	stop->super.set_atts = rsvg_stop_set_atts;
	stop->offset = 0;
	stop->rgba = 0;
	stop->is_current_color = 0;
	return &stop->super;
}

static void
rsvg_linear_gradient_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgLinearGradient *grad = (RsvgLinearGradient *)self;
	const char *value;
	double font_size = rsvg_state_current_font_size(ctx);
		
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				rsvg_defs_register_name (ctx->defs, value, self);
			if ((value = rsvg_property_bag_lookup (atts, "x1"))){
				grad->x1 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				grad->hasx1 = TRUE;
			}				
			if ((value = rsvg_property_bag_lookup (atts, "y1"))){
				grad->y1 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				grad->hasy1 = TRUE;
			}				
			if ((value = rsvg_property_bag_lookup (atts, "x2"))){
				grad->x2 = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				grad->hasx2 = TRUE;
			}	
			if ((value = rsvg_property_bag_lookup (atts, "y2"))){
				grad->y2 = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				grad->hasy2 = TRUE;
			}	
		if ((value = rsvg_property_bag_lookup (atts, "spreadMethod")))
				{
					if (!strcmp (value, "pad")) {
						grad->spread = RSVG_GRADIENT_PAD;
					}
					else if (!strcmp (value, "reflect")) {
						grad->spread = RSVG_GRADIENT_REFLECT;
					}
					else if (!strcmp (value, "repeat")) {
						grad->spread = RSVG_GRADIENT_REPEAT;
					}
					grad->hasspread = TRUE;
				}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				rsvg_defs_add_resolver (ctx->defs, &grad->fallback, value);
			if ((value = rsvg_property_bag_lookup (atts, "gradientTransform"))){
				rsvg_parse_transform (grad->affine, value);
				grad->hastransform = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "color")))
				grad->current_color = rsvg_css_parse_color (value, 0);
			if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					grad->obj_bbox = FALSE;
				else
					grad->obj_bbox = TRUE;
				grad->hasbbox = TRUE;
			}
		}
}


RsvgNode *
rsvg_new_linear_gradient (void)
{
	RsvgLinearGradient *grad = NULL;
	grad = g_new (RsvgLinearGradient, 1);
	grad->super.type = RSVG_NODE_LINGRAD;
	grad->super.free = rsvg_pserver_free;
	_rsvg_affine_identity(grad->affine);
	grad->has_current_color = FALSE;
	grad->x1 = 0;
	grad->y1 = 0;
	grad->x2 = 1.0;
	grad->y2 = 0;
	grad->fallback = NULL;
	grad->obj_bbox = TRUE;
	grad->spread = RSVG_GRADIENT_PAD;
	grad->super.children = g_ptr_array_new();
	grad->super.set_atts = rsvg_linear_gradient_set_atts;
	grad->super.draw = _rsvg_node_draw_nothing;
	grad->hasx1 = grad->hasy1 = grad->hasx2 = grad->hasy2 = grad->hasbbox = grad->hasspread = grad->hastransform = FALSE;
	return &grad->super;
}

static void
rsvg_radial_gradient_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgRadialGradient *grad = (RsvgRadialGradient *) self;
	const char *value;
	double font_size = rsvg_state_current_font_size(ctx);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				rsvg_defs_register_name (ctx->defs, value, self);
			if ((value = rsvg_property_bag_lookup (atts, "cx"))){
				grad->cx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);	
				grad->hascx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "cy"))){
				grad->cy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);	
				grad->hascy = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "r"))){
				grad->r = rsvg_css_parse_normalized_length (value, rsvg_dpi_percentage (ctx), 1, font_size);	
				grad->hasr = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fx"))){
				grad->fx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);	
				grad->hasfx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fy"))){
				grad->fy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				grad->hasfy = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				rsvg_defs_add_resolver (ctx->defs, &grad->fallback, value);	
			if ((value = rsvg_property_bag_lookup (atts, "gradientTransform"))){
				rsvg_parse_transform (grad->affine, value);
				grad->hastransform = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "color"))){
				grad->current_color = rsvg_css_parse_color (value, 0);
			}
			if ((value = rsvg_property_bag_lookup (atts, "spreadMethod"))){
				if (!strcmp (value, "pad"))
					grad->spread = RSVG_GRADIENT_PAD;
				else if (!strcmp (value, "reflect"))
					grad->spread = RSVG_GRADIENT_REFLECT;
				else if (!strcmp (value, "repeat"))
					grad->spread = RSVG_GRADIENT_REPEAT;
				grad->hasspread = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					grad->obj_bbox = FALSE;
				else
					grad->obj_bbox = TRUE;
				grad->hasbbox = TRUE;
			}
		}
}

RsvgNode *
rsvg_new_radial_gradient (void)
{

	RsvgRadialGradient * grad = g_new (RsvgRadialGradient, 1);
	grad->super.type = RSVG_NODE_RADGRAD;
	grad->super.free = rsvg_pserver_free;
	_rsvg_affine_identity(grad->affine);
	grad->has_current_color = FALSE;
	grad->obj_bbox = TRUE;
	grad->spread = RSVG_GRADIENT_PAD;
	grad->fallback = NULL;
	grad->cx = 0.5;
	grad->cy = 0.5;
	grad->r = 0.5;
	grad->fx = 0.5;
	grad->fy = 0.5;
	grad->super.children = g_ptr_array_new();
	grad->super.set_atts = rsvg_radial_gradient_set_atts;
	grad->hascx = grad->hascy = grad->hasfx = grad->hasfy = grad->hasr = grad->hasbbox = grad->hasspread = grad->hastransform = FALSE;
	grad->super.draw = _rsvg_node_draw_nothing;
	return &grad->super;
}

static void
rsvg_pattern_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgPattern *pattern = (RsvgPattern *)self;
	const char  *value;
	double font_size = rsvg_state_current_font_size(ctx);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				rsvg_defs_register_name (ctx->defs, value, self);
			if ((value = rsvg_property_bag_lookup (atts, "viewBox"))){
				pattern->vbox = rsvg_css_parse_vbox (value, 
													 &pattern->vbx, 
													 &pattern->vby,
													 &pattern->vbw, 
													 &pattern->vbh);
				pattern->hasvbox = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "x"))){
				pattern->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				pattern->hasx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y"))){
				pattern->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				pattern->hasy = TRUE;			
			}
			if ((value = rsvg_property_bag_lookup (atts, "width"))){
				pattern->width = rsvg_css_parse_normalized_length (value, ctx->dpi_x, 1, font_size);
				pattern->haswidth = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "height"))){
				pattern->height = rsvg_css_parse_normalized_length (value, ctx->dpi_y, 1, font_size);
				pattern->hasheight = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				rsvg_defs_add_resolver (ctx->defs, (RsvgNode **)&pattern->fallback, value);
			if ((value = rsvg_property_bag_lookup (atts, "patternTransform"))){
				rsvg_parse_transform (pattern->affine, value);
				pattern->hastransform = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "patternUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					pattern->obj_bbox = FALSE;
				else
					pattern->obj_bbox = TRUE;
				pattern->hasbbox = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "patternContentUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					pattern->obj_cbbox = FALSE;
				else
					pattern->obj_cbbox = TRUE;	
				pattern->hascbox = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio"))){
				pattern->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
				pattern->hasaspect = TRUE;
			}
		}
}


RsvgNode *
rsvg_new_pattern (void)
{
	RsvgPattern *pattern = g_new (RsvgPattern, 1);
	pattern->obj_bbox = TRUE;
	pattern->obj_cbbox = FALSE;
	pattern->x = 0;
	pattern->y = 0;
	pattern->width = 0;
	pattern->height = 0;
	pattern->vbx = 0;
	pattern->vby = 0;
	pattern->vbw = 1;
	pattern->vbh = 1;
	pattern->fallback = NULL;
	pattern->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	pattern->vbox = FALSE;
	_rsvg_affine_identity(pattern->affine);
	pattern->super.draw = _rsvg_node_draw_nothing;
	pattern->super.state = g_new(RsvgState, 1);
	rsvg_state_init(pattern->super.state);
	pattern->super.children = g_ptr_array_new();
	pattern->super.free = rsvg_pserver_free;
	pattern->super.set_atts = rsvg_pattern_set_atts;
	pattern->super.type = RSVG_NODE_PATTERN;
	pattern->hasx = pattern->hasy = pattern->haswidth = pattern->hasheight = pattern->hasvbox = pattern->hasbbox = pattern->hascbox = pattern->hasaspect = pattern->hastransform = pattern->hasaspect = FALSE; 
	return &pattern->super;
}

void
rsvg_linear_gradient_fix_fallback(RsvgLinearGradient * grad)
{
	RsvgNode * ufallback;
	int i;
	ufallback = grad->fallback;
	while (ufallback != NULL)
		{
			if (ufallback->type == RSVG_NODE_LINGRAD){
				RsvgLinearGradient * fallback = (RsvgLinearGradient *)ufallback;
				if (!grad->hasx1 && fallback->hasx1){
					grad->hasx1 = TRUE;
					grad->x1 = fallback->x1;
				}
				if (!grad->hasy1 && fallback->hasy1){
					grad->hasy1 = TRUE;
					grad->y1 = fallback->y1;
				}
				if (!grad->hasx2 && fallback->hasx2){
					grad->hasx2 = TRUE;
					grad->x2 = fallback->x2;
				}
				if (!grad->hasy2 && fallback->hasy2){
					grad->hasy2 = TRUE;
					grad->y2 = fallback->y2;
				}
				if (!grad->hastransform && fallback->hastransform){
					grad->hastransform = TRUE;
					for (i = 0; i < 6; i++)
						grad->affine[i] = fallback->affine[i];
				}
				if (!grad->hasspread && fallback->hasspread){
					grad->hasspread = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->hasbbox && fallback->hasbbox){
					grad->hasbbox = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->super.children->len && fallback->super.children->len){
					grad->super.children = fallback->super.children;
				}
				ufallback = fallback->fallback; 
			}
			else if (ufallback->type == RSVG_NODE_RADGRAD){
				RsvgRadialGradient * fallback = (RsvgRadialGradient *)ufallback;
				if (!grad->hastransform && fallback->hastransform){
					grad->hastransform = TRUE;
					for (i = 0; i < 6; i++)
						grad->affine[i] = fallback->affine[i];
				}
				if (!grad->hasspread && fallback->hasspread){
					grad->hasspread = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->hasbbox && fallback->hasbbox){
					grad->hasbbox = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->super.children->len && fallback->super.children->len){
					grad->super.children = fallback->super.children;
				}
				ufallback = fallback->fallback; 
			}
		}
}

void
rsvg_radial_gradient_fix_fallback(RsvgRadialGradient * grad)
{
	RsvgNode * ufallback;
	int i;
	ufallback = grad->fallback;
	while (ufallback != NULL)
		{
			if (ufallback->type == RSVG_NODE_RADGRAD){
				RsvgRadialGradient * fallback = (RsvgRadialGradient *)ufallback;
				if (!grad->hascx && fallback->hascx){
					grad->hascx = TRUE;
					grad->cx = fallback->cx;
				}
				if (!grad->hascy && fallback->hascy){
					grad->hascy = TRUE;
					grad->cy = fallback->cy;
				}
				if (!grad->hasfx && fallback->hasfx){
					grad->hasfx = TRUE;
					grad->fx = fallback->fx;
				}
				if (!grad->hasfy && fallback->hasfy){
					grad->hasfy = TRUE;
					grad->fy = fallback->fy;
				}
				if (!grad->hasr && fallback->hasr){
					grad->hasr = TRUE;
					grad->r = fallback->r;
				}
				if (!grad->hastransform && fallback->hastransform){
					grad->hastransform = TRUE;
					for (i = 0; i < 6; i++)
						grad->affine[i] = fallback->affine[i];
				}
				if (!grad->hasspread && fallback->hasspread){
					grad->hasspread = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->hasbbox && fallback->hasbbox){
					grad->hasbbox = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->super.children->len && fallback->super.children->len){
					grad->super.children = fallback->super.children;
				}
				ufallback = fallback->fallback; 
			}
			else if (ufallback->type == RSVG_NODE_LINGRAD){
				RsvgLinearGradient * fallback = (RsvgLinearGradient *)ufallback;
				if (!grad->hastransform && fallback->hastransform){
					grad->hastransform = TRUE;
					for (i = 0; i < 6; i++)
						grad->affine[i] = fallback->affine[i];
				}
				if (!grad->hasspread && fallback->hasspread){
					grad->hasspread = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->hasbbox && fallback->hasbbox){
					grad->hasbbox = TRUE;
					grad->obj_bbox = fallback->obj_bbox;
				}
				if (!grad->super.children->len && fallback->super.children->len){
					grad->super.children = fallback->super.children;
				}
				ufallback = fallback->fallback; 
			}
		}
}


void
rsvg_pattern_fix_fallback(RsvgPattern * pattern)
{
	RsvgPattern * fallback;
	int i;
	for (fallback = pattern->fallback; fallback != NULL; fallback = fallback->fallback)
		{
			if (!pattern->hasx && fallback->hasx){
				pattern->hasx = TRUE;
				pattern->x = fallback->x;
			}
			if (!pattern->hasy && fallback->hasy){
				pattern->hasy = TRUE;
				pattern->y = fallback->y;
			}
			if (!pattern->haswidth && fallback->haswidth){
				pattern->haswidth = TRUE;
				pattern->width = fallback->width;
			}
			if (!pattern->hasheight && fallback->hasheight){
				pattern->hasheight = TRUE;
				pattern->height = fallback->height;
			}
			if (!pattern->hasvbox && fallback->hasvbox){
				pattern->hasvbox = TRUE;
				pattern->vbx = fallback->vbx;
				pattern->vby = fallback->vby;
				pattern->vbw = fallback->vbw;
				pattern->vbh = fallback->vbh;
				pattern->vbox = fallback->vbox;
			}
			if (!pattern->hasaspect && fallback->hasaspect){
				pattern->hasaspect = TRUE;
				pattern->preserve_aspect_ratio = fallback->preserve_aspect_ratio;
			}
			if (!pattern->hastransform && fallback->hastransform){
				pattern->hastransform = TRUE;
				for (i = 0; i < 6; i++)
					pattern->affine[i] = fallback->affine[i];
			}
			if (!pattern->hasbbox && fallback->hasbbox){
				pattern->hasbbox = TRUE;
				pattern->obj_bbox = fallback->obj_bbox;
			}
			if (!pattern->hascbox && fallback->hascbox){
				pattern->hascbox = TRUE;
				pattern->obj_cbbox = fallback->obj_cbbox;
			}
			if (!pattern->super.children->len && fallback->super.children->len){
				pattern->super.children = fallback->super.children;
			}
		}
}
