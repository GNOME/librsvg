/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

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

#include "config.h"

#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-text.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"

#include <math.h>
#include <string.h>
#include <stdarg.h>

#include <libart_lgpl/art_affine.h>

#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-paint-server.h"

/*
 * This is configurable at runtime
 */
#define RSVG_DEFAULT_DPI 90.0
static double internal_dpi = RSVG_DEFAULT_DPI;

static void
rsvg_ctx_free_helper (gpointer key, gpointer value, gpointer user_data)
{
	xmlEntityPtr entval = (xmlEntityPtr)value;
	
	/* key == entval->name, so it's implicitly freed below */
	
	g_free ((char *) entval->name);
	g_free ((char *) entval->ExternalID);
	g_free ((char *) entval->SystemID);
	xmlFree (entval->content);
	xmlFree (entval->orig);
	g_free (entval);
}

static void
rsvg_pixmap_destroy (guchar *pixels, gpointer data)
{
	g_free (pixels);
}

static void
rsvg_start_svg (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	int width = -1, height = -1, x = -1, y = -1;
	int rowstride;
	art_u8 *pixels;
	RsvgState *state;
	gint new_width, new_height;
	double x_zoom = 1.;
	double y_zoom = 1.;
	double affine[6];
	const char * value;

	double vbox_x = 0, vbox_y = 0, vbox_w = 0, vbox_h = 0;
	gboolean has_vbox = FALSE;

	if (rsvg_property_bag_size (atts))
		{
			/* x & y should be ignored since we should always be the outermost SVG,
			   at least for now, but i'll include them here anyway */
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					has_vbox = rsvg_css_parse_vbox (value, &vbox_x, &vbox_y,
													&vbox_w, &vbox_h);
				}
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				width = rsvg_css_parse_normalized_length (value, ctx->dpi, vbox_w, 1);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				height = rsvg_css_parse_normalized_length (value, ctx->dpi, vbox_h, 1);
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi, vbox_w, 1);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi, vbox_h, 1);
	
			if (has_vbox && vbox_w > 0. && vbox_h > 0.)
				{
					new_width  = (int)floor (vbox_w);
					new_height = (int)floor (vbox_h);
					
					/* apply the sizing function on the *original* width and height
					   to acquire our real destination size. we'll scale it against
					   the viewBox's coordinates later */
					if (ctx->size_func)
						(* ctx->size_func) (&width, &height, ctx->user_data);
				}
			else
				{
					new_width  = width;
					new_height = height;

					/* bogus hack */
					if (new_width <= 0 || new_height <= 0)
						{
							g_warning (_("rsvg_start_svg: width and height not specified in the SVG"));
							if (new_width <= 0) {width = new_width = 512;}
							if (new_height <= 0) {height = new_height = 512;}
						}

					/* apply the sizing function to acquire our new width and height.
					   we'll scale this against the old values later */
					if (ctx->size_func)
						(* ctx->size_func) (&new_width, &new_height, ctx->user_data);
				}

			/* set these here because % are relative to viewbox */
			ctx->width = new_width;
			ctx->height = new_height;

			if (!has_vbox)
				{
					  x_zoom = (width < 0 || new_width < 0) ? 1 : (double) new_width / width;
					  y_zoom = (height < 0 || new_height < 0) ? 1 : (double) new_height / height;
				}
			else
				{
					x_zoom = (width < 0 || new_width < 0) ? 1 : (double) width / new_width;
					y_zoom = (height < 0 || new_height < 0) ? 1 : (double) height / new_height;

					/* reset these so that we get a properly sized SVG and not a huge one */
					new_width  = (width == -1 ? new_width : width);
					new_height = (height == -1 ? new_height : height);
				}

			/* Scale size of target pixbuf */
			state = rsvg_state_current (ctx);

			art_affine_identity (state->affine);

			if (has_vbox && (vbox_x != 0. || vbox_y != 0.))
				{
					art_affine_translate (affine, - vbox_x, - vbox_y);
					art_affine_multiply (state->affine, state->affine, affine);
				}

			art_affine_scale (affine, x_zoom, y_zoom);
			art_affine_multiply (state->affine, state->affine, affine);

			if (new_width <= 0 || new_height <= 0)
				{
					g_warning (_("rsvg_start_svg: width and height not specified in the SVG, nor supplied by the size callback"));
					if (new_width <= 0) new_width = 512;
					if (new_height <= 0) new_height = 512;
				}

			if (new_width >= INT_MAX / 4)
				{
					/* FIXME: GError here? */
					g_warning (_("rsvg_start_svg: width too large"));
					return;
				}
			rowstride = (new_width * 4 + 3) & ~3;
			if (rowstride > INT_MAX / new_height)
				{
					/* FIXME: GError here? */
					g_warning (_("rsvg_start_svg: width too large"));
					return;
				}

			/* FIXME: Add GError here if size is too big. */

			pixels = g_try_malloc (rowstride * new_height);
			if (pixels == NULL)
				{
					/* FIXME: GError here? */
					g_warning (_("rsvg_start_svg: dimensions too large"));
					return;
				}
			memset (pixels, 0, rowstride * new_height);
			ctx->pixbuf = gdk_pixbuf_new_from_data (pixels,
													GDK_COLORSPACE_RGB,
													TRUE, 8,
													new_width, new_height,
													rowstride,
													rsvg_pixmap_destroy,
													NULL);
		}
}

static void
rsvg_start_g (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgState *state = rsvg_state_current (ctx);
	const char * klazz = NULL, * id = NULL, *value;
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, state, "g", klazz, id, atts);
		}	
  
	rsvg_push_def_group (ctx, id);
	if (!ctx->in_defs)	
		rsvg_push_discrete_layer (ctx);
}

static void
rsvg_end_g (RsvgHandle *ctx)
{
	rsvg_pop_def_group (ctx);
	if (!ctx->in_defs)
		rsvg_pop_discrete_layer (ctx);
}

typedef struct _RsvgSaxHandlerDefs {
	RsvgSaxHandler super;
	RsvgHandle *ctx;
} RsvgSaxHandlerDefs;

typedef struct _RsvgSaxHandlerStyle {
	RsvgSaxHandler super;
	RsvgSaxHandlerDefs *parent;
	RsvgHandle *ctx;
	GString *style;
} RsvgSaxHandlerStyle;

typedef struct _RsvgSaxHandlerGstops {
	RsvgSaxHandler super;
	RsvgSaxHandlerDefs *parent;
	RsvgHandle *ctx;
	RsvgGradientStops *stops;
	const char * parent_tag;
} RsvgSaxHandlerGstops;

/* hide this fact from the general public */
typedef RsvgSaxHandlerDefs RsvgSaxHandlerTitle;
typedef RsvgSaxHandlerDefs RsvgSaxHandlerDesc;

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
	
	rsvg_state_init (&state);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "offset")))
				{
					/* either a number [0,1] or a percentage */
					offset = rsvg_css_parse_normalized_length (value, z->ctx->dpi, 1., 0.);
					
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
	
	rsvg_state_finalize (&state);
	
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
	RsvgSaxHandler *prev = &z->parent->super;
	
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
	
	gstops->parent = (RsvgSaxHandlerDefs*)ctx->handler;
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
	
	gstops->parent = (RsvgSaxHandlerDefs*)ctx->handler;
	*p_stops = stops;
	return &gstops->super;
}

/* exported to the paint server via rsvg-private.h */
void
rsvg_linear_gradient_free (RsvgDefVal *self)
{
	RsvgLinearGradient *z = (RsvgLinearGradient *)self;
	
	g_free (z->stops->stop);
	g_free (z->stops);
	g_free (self);
}

static void
rsvg_start_linear_gradient (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgLinearGradient *grad = NULL;
	const char *id = NULL, *value;
	double x1 = 0., y1 = 0., x2 = 0., y2 = 0.;
	ArtGradientSpread spread = ART_GRADIENT_PAD;
	const char * xlink_href = NULL;
	gboolean obj_bbox = TRUE;
	gboolean got_x1, got_x2, got_y1, got_y2, got_spread, got_transform, got_bbox, cloned, shallow_cloned;
	double affine[6];
	guint32 color = 0;
	gboolean got_color = FALSE;
	int i;

	got_x1 = got_x2 = got_y1 = got_y2 = got_spread = got_transform = got_bbox = cloned = shallow_cloned = FALSE;
		
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "x1"))) {
				x1 = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_x1 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y1"))) {
				y1 = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_y1 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "x2"))) {
				x2 = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_x2 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "y2"))) {
				y2 = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_y2 = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "spreadMethod")))
				{
					if (!strcmp (value, "pad")) {
						spread = ART_GRADIENT_PAD;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "reflect")) {
						spread = ART_GRADIENT_REFLECT;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "repeat")) {
						spread = ART_GRADIENT_REPEAT;
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
		}
	
	/* set up 100% as the default if not gotten */
	if (!got_x2) {
		if (obj_bbox)
			x2 = 1.0;
		else
			x2 = rsvg_css_parse_normalized_length ("100%", ctx->dpi, (gdouble)ctx->width, state->font_size);
	}

	if (xlink_href != NULL)
		{
			RsvgLinearGradient * parent = (RsvgLinearGradient*)rsvg_defs_lookup (ctx->defs, xlink_href+1);
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
			grad->super.type = RSVG_DEF_LINGRAD;
			grad->super.free = rsvg_linear_gradient_free;
			ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops, "linearGradient");
		}
	
	rsvg_defs_set (ctx->defs, id, &grad->super);
	
	if (got_transform)
		for (i = 0; i < 6; i++)
			grad->affine[i] = affine[i];
	else
		art_affine_identity(grad->affine);

	if (got_color)
		{
			grad->current_color = color;
			grad->has_current_color = TRUE;
		}

	/* gradient inherits parent/cloned information unless it's explicity gotten */
	grad->obj_bbox = (cloned && !got_bbox) ? grad->obj_bbox : obj_bbox;
	grad->x1 = (cloned && !got_x1) ? grad->x1 : x1;
	grad->y1 = (cloned && !got_y1) ? grad->y1 : y1;
	grad->x2 = (cloned && !got_x2) ? grad->x2 : x2;
	grad->y2 = (cloned && !got_y2) ? grad->y1 : y2;
	grad->spread = (cloned && !got_spread) ? grad->spread : spread;
}

/* exported to the paint server via rsvg-private.h */
void
rsvg_radial_gradient_free (RsvgDefVal *self)
{
	RsvgRadialGradient *z = (RsvgRadialGradient *)self;
	
	g_free (z->stops->stop);
	g_free (z->stops);
	g_free (self);
}

static void
rsvg_start_radial_gradient (RsvgHandle *ctx, RsvgPropertyBag *atts, const char * tag) /* tag for conicalGradient */
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgRadialGradient *grad = NULL;
	const char *id = NULL;
	double cx = 0., cy = 0., r = 0., fx = 0., fy = 0.;  
	const char * xlink_href = NULL, *value;
	ArtGradientSpread spread = ART_GRADIENT_PAD;
	gboolean obj_bbox = TRUE;
	gboolean got_cx, got_cy, got_r, got_fx, got_fy, got_spread, got_transform, got_bbox, cloned, shallow_cloned;
	guint32 color = 0;
	gboolean got_color = FALSE;
	double affine[6];
	int i;

	got_cx = got_cy = got_r = got_fx = got_fy = got_spread = got_transform = got_bbox = cloned = shallow_cloned = FALSE;
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "cx"))) {
				cx = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_cx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "cy"))) {
				cy = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_cy = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "r"))) {
				r = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, 
													  state->font_size);
				got_r = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fx"))) {
				fx = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
				got_fx = TRUE;
			}
			if ((value = rsvg_property_bag_lookup (atts, "fy"))) {
				fy = rsvg_css_parse_normalized_length (value, ctx->dpi, 1, state->font_size);
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
						spread = ART_GRADIENT_PAD;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "reflect")) {
						spread = ART_GRADIENT_REFLECT;
						got_spread = TRUE;
					}
					else if (!strcmp (value, "repeat")) {
						spread = ART_GRADIENT_REPEAT;
						got_spread = TRUE;
					}
				}
			if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
				if (!strcmp (value, "userSpaceOnUse"))
					obj_bbox = FALSE;
				got_bbox = TRUE;
			}
		}
	
	if (xlink_href != NULL)
		{
			RsvgRadialGradient * parent = (RsvgRadialGradient*)rsvg_defs_lookup (ctx->defs, xlink_href+1);
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
			grad->super.type = RSVG_DEF_RADGRAD;
			grad->super.free = rsvg_radial_gradient_free;
			ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops, tag);		   
		}

	/* setup defaults */
	if (!got_cx) {
		if (obj_bbox)
			cx = 0.5;
		else
			cx = rsvg_css_parse_normalized_length ("50%", ctx->dpi, (gdouble)ctx->width, state->font_size);
	}
	if (!got_cy) {
		if (obj_bbox)
			cy = 0.5;
		else
			cy = rsvg_css_parse_normalized_length ("50%", ctx->dpi, (gdouble)ctx->height, state->font_size);
	}
	if (!got_r) {
		if (obj_bbox)
			r = 0.5;
		else
			r  = rsvg_css_parse_normalized_length ("50%", ctx->dpi, rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), state->font_size);
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
		art_affine_identity(grad->affine);
	
	if (got_color)
		{
			grad->current_color = color;
			grad->has_current_color = TRUE;
		}

	/* gradient inherits parent/cloned information unless it's explicity gotten */
	grad->obj_bbox = (cloned && !got_bbox) ? grad->obj_bbox : obj_bbox;
	grad->cx = (cloned && !got_cx) ? grad->cx : cx;
	grad->cy = (cloned && !got_cy) ? grad->cy : cy;
	grad->r =  (cloned && !got_r)  ? grad->r  : r;
	grad->fx = (cloned && !got_fx) ? grad->fx : fx;
	grad->fy = (cloned && !got_fy) ? grad->fy : fy;
	grad->spread = (cloned && !got_spread) ? grad->spread : spread;
}

/* end gradients */

static void
rsvg_style_handler_free (RsvgSaxHandler *self)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	RsvgHandle *ctx = z->ctx;
	
	rsvg_parse_cssbuffer (ctx, z->style->str, z->style->len);
	
	g_string_free (z->style, TRUE);
	g_free (z);
}

static void
rsvg_style_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	g_string_append_len (z->style, (const char *)ch, len);
}

static void
rsvg_style_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						  RsvgPropertyBag *atts)
{
}

static void
rsvg_style_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	RsvgHandle *ctx = z->ctx;
	RsvgSaxHandler *prev = &z->parent->super;
	
	if (!strcmp ((char *)name, "style"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = prev;
				}
		}
}

static void
rsvg_start_style (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerStyle *handler = g_new0 (RsvgSaxHandlerStyle, 1);
	
	handler->super.free = rsvg_style_handler_free;
	handler->super.characters = rsvg_style_handler_characters;
	handler->super.start_element = rsvg_style_handler_start;
	handler->super.end_element   = rsvg_style_handler_end;
	handler->ctx = ctx;
	
	handler->style = g_string_new (NULL);
	
	handler->parent = (RsvgSaxHandlerDefs*)ctx->handler;
	ctx->handler = &handler->super;
}

/* start defs */

static void
rsvg_defs_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_defs_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
}

static void
rsvg_filter_handler_start (RsvgHandle *ctx, const xmlChar *name,
						   RsvgPropertyBag *atts)
{
	if (!strcmp ((char *)name, "filter"))
		rsvg_start_filter (ctx, atts);
	else if (!strcmp ((char *)name, "feBlend"))
		rsvg_start_filter_primitive_blend (ctx, atts);
	else if (!strcmp ((char *)name, "feColorMatrix"))
		rsvg_start_filter_primitive_colour_matrix(ctx, atts);
	else if (!strcmp ((char *)name, "feComponentTransfer"))
		rsvg_start_filter_primitive_component_transfer(ctx, atts);
	else if (!strcmp ((char *)name, "feComposite"))
		rsvg_start_filter_primitive_composite(ctx, atts);
	else if (!strcmp ((char *)name, "feConvolveMatrix"))
		rsvg_start_filter_primitive_convolve_matrix (ctx, atts);
	else if (!strcmp ((char *)name, "feDiffuseLighting"))
		rsvg_start_filter_primitive_diffuse_lighting(ctx, atts);
	else if (!strcmp ((char *)name, "feDisplacementMap"))
		rsvg_start_filter_primitive_displacement_map(ctx, atts);
	else if (!strcmp ((char *)name, "feFlood"))
		rsvg_start_filter_primitive_flood(ctx, atts);
	else if (!strcmp ((char *)name, "feGaussianBlur"))
		rsvg_start_filter_primitive_gaussian_blur (ctx, atts);
	else if (!strcmp ((char *)name, "feImage"))
		rsvg_start_filter_primitive_image (ctx, atts);
	else if (!strcmp ((char *)name, "feMerge"))
		rsvg_start_filter_primitive_merge(ctx, atts);
	else if (!strcmp ((char *)name, "feMorphology"))
		rsvg_start_filter_primitive_erode(ctx, atts);
	else if (!strcmp ((char *)name, "feOffset"))
		rsvg_start_filter_primitive_offset(ctx, atts);
	else if (!strcmp ((char *)name, "feSpecularLighting"))
		rsvg_start_filter_primitive_specular_lighting(ctx, atts);
	else if (!strcmp ((char *)name, "feTile"))
		rsvg_start_filter_primitive_tile(ctx, atts);
	else if (!strcmp ((char *)name, "feTurbulence"))
		rsvg_start_filter_primitive_turbulence(ctx, atts);
	else if (!strcmp ((char *)name, "feDistantLight"))
		rsvg_start_filter_primitive_light_source(ctx, atts, 'd');
	else if (!strcmp ((char *)name, "feSpotLight"))
		rsvg_start_filter_primitive_light_source(ctx, atts, 's');
	else if (!strcmp ((char *)name, "fePointLight"))
		rsvg_start_filter_primitive_light_source(ctx, atts, 'p');
	else if (!strcmp ((char *)name, "feMergeNode"))
		rsvg_start_filter_primitive_merge_node(ctx, atts);
	else if (!strcmp ((char *)name, "feFuncR"))
		rsvg_start_filter_primitive_component_transfer_function(ctx, atts, 'r');
	else if (!strcmp ((char *)name, "feFuncG"))
		rsvg_start_filter_primitive_component_transfer_function(ctx, atts, 'g');
	else if (!strcmp ((char *)name, "feFuncB"))
		rsvg_start_filter_primitive_component_transfer_function(ctx, atts, 'b');
	else if (!strcmp ((char *)name, "feFuncA"))
		rsvg_start_filter_primitive_component_transfer_function(ctx, atts, 'a');
}

static void
rsvg_defs_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
	RsvgSaxHandlerDefs *z = (RsvgSaxHandlerDefs *)self;
	RsvgHandle *ctx = z->ctx;
	
	/* push the state stack */
	if (ctx->n_state == ctx->n_state_max)
		ctx->state = g_renew (RsvgState, ctx->state, ctx->n_state_max <<= 1);
	if (ctx->n_state)
		{
			rsvg_state_inherit (&ctx->state[ctx->n_state],
								&ctx->state[ctx->n_state - 1]);
		}
	else
		rsvg_state_init (ctx->state);
	ctx->n_state++;

	/*
	 * conicalGradient isn't in the SVG spec and I'm not sure exactly what it does. libart definitely
	 * has no analogue. But it does seem similar enough to a radialGradient that i'd rather get the
	 * onscreen representation of the colour wrong than not have any colour displayed whatsoever
	 */

	if (!strcmp ((char *)name, "defs"))
		ctx->in_defs++;
	else if (!strcmp ((char *)name, "linearGradient"))
		rsvg_start_linear_gradient (ctx, atts);
	else if (!strcmp ((char *)name, "radialGradient"))
		rsvg_start_radial_gradient (ctx, atts, "radialGradient");
	else if (!strcmp((char *)name, "conicalGradient"))
		rsvg_start_radial_gradient (ctx, atts, "conicalGradient");
	else if (!strcmp ((char *)name, "style"))
		rsvg_start_style (ctx, atts);
	else if (!strcmp ((char *)name, "g"))
		rsvg_start_g (ctx, atts);
	else if (!strcmp ((char *)name, "symbol"))
		{
			ctx->in_defs++;
			rsvg_start_g (ctx, atts);
		}
	else if (!strcmp ((char *)name, "use"))
		rsvg_start_use (ctx, atts);
	else if (!strcmp ((char *)name, "path"))
		rsvg_start_path (ctx, atts);
	else if (!strcmp ((char *)name, "line"))
		rsvg_start_line (ctx, atts);
	else if (!strcmp ((char *)name, "rect"))
		rsvg_start_rect (ctx, atts);
	else if (!strcmp ((char *)name, "circle"))
		rsvg_start_circle (ctx, atts);
	else if (!strcmp ((char *)name, "ellipse"))
		rsvg_start_ellipse (ctx, atts);
	else if (!strcmp ((char *)name, "polygon"))
		rsvg_start_polygon (ctx, atts);
	else if (!strcmp ((char *)name, "polyline"))
		rsvg_start_polyline (ctx, atts);
	else if (!strcmp ((char *)name, "mask"))
		rsvg_start_mask(ctx, atts);	
	rsvg_filter_handler_start (ctx, name, atts);
}

static void
rsvg_defs_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerDefs *z = (RsvgSaxHandlerDefs *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "defs"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}
			ctx->in_defs--;
		}

	if (!strcmp ((char *)name, "g"))
		rsvg_end_g (ctx);
	else if (!strcmp ((char *)name, "symbol"))
		{
			ctx->in_defs--;
			rsvg_end_g (ctx);
		}
	else if (!strcmp ((char *)name, "filter"))
		rsvg_end_filter (ctx);
	else if (!strcmp ((char *)name, "mask"))
		rsvg_end_mask(ctx);
	
	/* pop the state stack */
	ctx->n_state--;
	rsvg_state_finalize (&ctx->state[ctx->n_state]);
}

static void
rsvg_start_defs (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerDefs *handler = g_new0 (RsvgSaxHandlerDefs, 1);
	
	handler->super.free = rsvg_defs_handler_free;
	handler->super.characters = rsvg_defs_handler_characters;
	handler->super.start_element = rsvg_defs_handler_start;
	handler->super.end_element   = rsvg_defs_handler_end;
	handler->ctx = ctx;

	ctx->in_defs++;	
	ctx->handler = &handler->super;
}

/* end defs */

/* start desc */

static void
rsvg_desc_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_desc_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;

	char * string = NULL;
	char * utf8 = NULL;

	/* This isn't quite the correct behavior - in theory, any graphics
	   element may contain a title or desc element */

	if (!ch || !len)
		return;

	string = g_strndup (ch, len);
	if (!g_utf8_validate (string, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = utf8;
		}

	g_string_append (ctx->desc, string);
	g_free (string);
}

static void
rsvg_desc_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
}

static void
rsvg_desc_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "desc"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}
		}
	
	/* pop the state stack */
	ctx->n_state--;
	rsvg_state_finalize (&ctx->state[ctx->n_state]);
}

static void
rsvg_start_desc (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerDesc *handler = g_new0 (RsvgSaxHandlerDesc, 1);
	
	handler->super.free = rsvg_desc_handler_free;
	handler->super.characters = rsvg_desc_handler_characters;
	handler->super.start_element = rsvg_desc_handler_start;
	handler->super.end_element   = rsvg_desc_handler_end;
	handler->ctx = ctx;

	ctx->handler = &handler->super;
}

/* end desc */

/* start title */

static void
rsvg_title_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_title_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;

	char * string = NULL;
	char * utf8 = NULL;

	/* This isn't quite the correct behavior - in theory, any graphics
	   element may contain a title or desc element */

	if (!ch || !len)
		return;

	string = g_strndup (ch, len);
	if (!g_utf8_validate (string, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = utf8;
		}

	g_string_append (ctx->title, string);
	g_free (string);
}

static void
rsvg_title_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
}

static void
rsvg_title_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerTitle *z = (RsvgSaxHandlerTitle *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "title"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}
		}
	
	/* pop the state stack */
	ctx->n_state--;
	rsvg_state_finalize (&ctx->state[ctx->n_state]);
}

static void
rsvg_start_title (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerTitle *handler = g_new0 (RsvgSaxHandlerTitle, 1);
	
	handler->super.free = rsvg_title_handler_free;
	handler->super.characters = rsvg_title_handler_characters;
	handler->super.start_element = rsvg_title_handler_start;
	handler->super.end_element   = rsvg_title_handler_end;
	handler->ctx = ctx;

	ctx->handler = &handler->super;
}

/* end title */

static void
rsvg_start_element (void *data, const xmlChar *name, const xmlChar **atts)
{
	RsvgHandle *ctx = (RsvgHandle *)data;

	RsvgPropertyBag * bag;

	bag = rsvg_property_bag_new(atts);

	if (ctx->handler)
		{
			ctx->handler_nest++;
			if (ctx->handler->start_element != NULL)
				ctx->handler->start_element (ctx->handler, name, bag);
		}
	else
		{
			/* push the state stack */
			if (ctx->n_state == ctx->n_state_max)
				ctx->state = g_renew (RsvgState, ctx->state, ctx->n_state_max <<= 1);
			if (ctx->n_state)
				{
					rsvg_state_inherit (&ctx->state[ctx->n_state],
										&ctx->state[ctx->n_state - 1]);
				}			
			else
				rsvg_state_init (ctx->state);
			ctx->n_state++;
			
			if (!strcmp ((char *)name, "svg"))
				rsvg_start_svg (ctx, bag);
			else if (!strcmp ((char *)name, "g"))
				rsvg_start_g (ctx, bag);
			else if (!strcmp ((char *)name, "symbol"))
				{
					ctx->in_defs++;
					rsvg_start_g (ctx, bag);
				}
			else if (!strcmp ((char *)name, "defs"))
				rsvg_start_defs (ctx, bag);
			else if (!strcmp ((char *)name, "path"))
				rsvg_start_path (ctx, bag);
			else if (!strcmp ((char *)name, "line"))
				rsvg_start_line (ctx, bag);
			else if (!strcmp ((char *)name, "rect"))
				rsvg_start_rect (ctx, bag);
			else if (!strcmp ((char *)name, "circle"))
				rsvg_start_circle (ctx, bag);
			else if (!strcmp ((char *)name, "ellipse"))
				rsvg_start_ellipse (ctx, bag);
			else if (!strcmp ((char *)name, "polygon"))
				rsvg_start_polygon (ctx, bag);
			else if (!strcmp ((char *)name, "polyline"))
				rsvg_start_polyline (ctx, bag);
			else if (!strcmp ((char *)name, "use"))
				rsvg_start_use (ctx, bag);
			else if (!strcmp ((char *)name, "text"))
				rsvg_start_text (ctx, bag);
			else if (!strcmp ((char *)name, "image"))
				rsvg_start_image (ctx, bag);
			else if (!strcmp ((char *)name, "style"))
				rsvg_start_style (ctx, bag);
			else if (!strcmp ((char *)name, "title"))
				rsvg_start_title (ctx, bag);
			else if (!strcmp ((char *)name, "desc"))
				rsvg_start_desc (ctx, bag);
			else if (!strcmp ((char *)name, "mask"))
				{
					ctx->in_defs++;
					rsvg_start_mask(ctx, bag);
				}
			
			/* see conicalGradient discussion above */
			else if (!strcmp ((char *)name, "linearGradient"))
				rsvg_start_linear_gradient (ctx, bag);
			else if (!strcmp ((char *)name, "radialGradient"))
				rsvg_start_radial_gradient (ctx, bag, "radialGradient");
			else if (!strcmp ((char *)name, "conicalGradient"))
				rsvg_start_radial_gradient (ctx, bag, "conicalGradient");

			rsvg_filter_handler_start (ctx, name, bag);
    }

	rsvg_property_bag_free(bag);
}

static void
rsvg_end_element (void *data, const xmlChar *name)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	if (ctx->handler_nest > 0 && ctx->handler != NULL)
		{
			if (ctx->handler->end_element != NULL)
				ctx->handler->end_element (ctx->handler, name);
			ctx->handler_nest--;
		}
	else
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}

			if (!strcmp ((char *)name, "g"))
				rsvg_end_g (ctx);
			else if (!strcmp ((char *)name, "symbol"))
				{
					ctx->in_defs--;
					rsvg_end_g (ctx);
				}
			else if (!strcmp ((char *)name, "filter"))
				rsvg_end_filter (ctx);
			else if (!strcmp ((char *)name, "defs")) {
				ctx->in_defs--;	
			}
			else if (!strcmp ((char *)name, "mask")){
				rsvg_end_mask(ctx);
				ctx->in_defs--;				
			}
			/* pop the state stack */
			ctx->n_state--;
			rsvg_state_finalize (&ctx->state[ctx->n_state]);
		}
}

static void
rsvg_characters (void *data, const xmlChar *ch, int len)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	if (ctx->handler && ctx->handler->characters != NULL)
		ctx->handler->characters (ctx->handler, ch, len);
}

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar *name)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	return (xmlEntityPtr)g_hash_table_lookup (ctx->entities, name);
}

static void
rsvg_entity_decl (void *data, const xmlChar *name, int type,
				  const xmlChar *publicId, const xmlChar *systemId, xmlChar *content)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	GHashTable *entities = ctx->entities;
	xmlEntityPtr entity;
	char *dupname;

	entity = g_new0 (xmlEntity, 1);
	entity->type = type;
	entity->length = strlen (name);
	dupname = g_strdup (name);
	entity->name = dupname;
	entity->ExternalID = g_strdup (publicId);
	entity->SystemID = g_strdup (systemId);
	if (content)
		{
			entity->content = xmlMemStrdup (content);
			entity->length = strlen (content);
		}
	g_hash_table_insert (entities, dupname, entity);
}

static void
rsvg_error_cb (void *data, const char *msg, ...)
{
	va_list args;
	
	va_start (args, msg);
	vfprintf (stderr, msg, args);
	va_end (args);
}

static xmlSAXHandler rsvgSAXHandlerStruct = {
    NULL, /* internalSubset */
    NULL, /* isStandalone */
    NULL, /* hasInternalSubset */
    NULL, /* hasExternalSubset */
    NULL, /* resolveEntity */
    rsvg_get_entity, /* getEntity */
    rsvg_entity_decl, /* entityDecl */
    NULL, /* notationDecl */
    NULL, /* attributeDecl */
    NULL, /* elementDecl */
    NULL, /* unparsedEntityDecl */
    NULL, /* setDocumentLocator */
    NULL, /* startDocument */
    NULL, /* endDocument */
    rsvg_start_element, /* startElement */
    rsvg_end_element, /* endElement */
    NULL, /* reference */
    rsvg_characters, /* characters */
    NULL, /* ignorableWhitespace */
    NULL, /* processingInstruction */
    NULL, /* comment */
    NULL, /* xmlParserWarning */
    rsvg_error_cb, /* xmlParserError */
    rsvg_error_cb, /* xmlParserFatalError */
    NULL, /* getParameterEntity */
    rsvg_characters, /* cdataCallback */
    NULL /* externalSubset */
	/* initialized */
};

/**
 * rsvg_error_quark
 *
 * The error domain for RSVG
 *
 * Returns: The error domain
 */
GQuark
rsvg_error_quark (void)
{
	static GQuark q = 0;
	if (q == 0)
		q = g_quark_from_static_string ("rsvg-error-quark");
	
	return q;
}

gboolean
rsvg_handle_write_impl (RsvgHandle    *handle,
						const guchar  *buf,
						gsize          count,
						GError       **error)
{
	GError *real_error;
	g_return_val_if_fail (handle != NULL, FALSE);
	
	handle->error = &real_error;
	if (handle->ctxt == NULL)
		{
			handle->ctxt = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0, NULL);
			handle->ctxt->replaceEntities = TRUE;
		}
	
	xmlParseChunk (handle->ctxt, buf, count, 0);
	
	handle->error = NULL;
	/* FIXME: Error handling not implemented. */
	/*  if (*real_error != NULL)
		{
		g_propagate_error (error, real_error);
		return FALSE;
		}*/
  return TRUE;
}

gboolean
rsvg_handle_close_impl (RsvgHandle  *handle,
						GError     **error)
{
	GError *real_error;
	
	handle->error = &real_error;
	
	if (handle->ctxt != NULL)
		{
			xmlDocPtr xmlDoc;

			xmlDoc = handle->ctxt->myDoc;

			xmlParseChunk (handle->ctxt, "", 0, TRUE);
			xmlFreeParserCtxt (handle->ctxt);
			xmlFreeDoc(xmlDoc);
		}
  
	/* FIXME: Error handling not implemented. */
	/*
	  if (real_error != NULL)
	  {
      g_propagate_error (error, real_error);
      return FALSE;
      }*/
	return TRUE;
}

void
rsvg_handle_free_impl (RsvgHandle *handle)
{
	int i;
	
	if (handle->pango_context != NULL)
		g_object_unref (handle->pango_context);
	rsvg_defs_free (handle->defs);
	
	for (i = 0; i < handle->n_state; i++)
		rsvg_state_finalize (&handle->state[i]);
	g_free (handle->state);
	
	g_hash_table_foreach (handle->entities, rsvg_ctx_free_helper, NULL);
	g_hash_table_destroy (handle->entities);
	
	g_hash_table_destroy (handle->css_props);
	
	if (handle->user_data_destroy)
		(* handle->user_data_destroy) (handle->user_data);
	if (handle->pixbuf)
		g_object_unref (handle->pixbuf);

	if (handle->title)
		g_string_free (handle->title, TRUE);
	if (handle->desc)
	    	g_string_free (handle->desc, TRUE);

	g_free (handle);
}

/**
 * rsvg_handle_get_title:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's title in UTF-8 or %NULL. You must make a copy
 * of this title if you wish to use it after #handle has been freed.
 *
 * Returns: The SVG's title
 *
 * Since: 2.4
 */
G_CONST_RETURN char *rsvg_handle_get_title (RsvgHandle *handle)
{
	return handle->title->str;
}

/**
 * rsvg_handle_get_desc:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's description in UTF-8 or %NULL. You must make a copy
 * of this description if you wish to use it after #handle has been freed.
 *
 * Returns: The SVG's description
 *
 * Since: 2.4
 */
G_CONST_RETURN char *rsvg_handle_get_desc (RsvgHandle *handle)
{
	return handle->desc->str;
}

/**
 * rsvg_handle_new:
 *
 * Returns a new rsvg handle.  Must be freed with @rsvg_handle_free.  This
 * handle can be used for dynamically loading an image.  You need to feed it
 * data using @rsvg_handle_write, then call @rsvg_handle_close when done.  No
 * more than one image can be loaded with one handle.
 *
 * Returns: A new #RsvgHandle
 **/
RsvgHandle *
rsvg_handle_new (void)
{
	RsvgHandle *handle;
	
	handle = g_new0 (RsvgHandle, 1);
	rsvg_handle_init (handle);

	handle->write = rsvg_handle_write_impl;
	handle->close = rsvg_handle_close_impl;
	handle->free  = rsvg_handle_free_impl;

	return handle;
}

void
rsvg_handle_init (RsvgHandle * handle)
{
	handle->n_state = 0;
	handle->n_state_max = 16;
	handle->state = g_new (RsvgState, handle->n_state_max);
	handle->defs = rsvg_defs_new ();
	handle->handler_nest = 0;
	handle->entities = g_hash_table_new (g_str_hash, g_str_equal);
	handle->dpi = internal_dpi;
	
	handle->css_props = g_hash_table_new_full (g_str_hash, g_str_equal,
											   g_free, g_free);
	
	handle->ctxt = NULL;
	handle->current_defs_group = NULL;
	handle->title = g_string_new (NULL);
	handle->desc = g_string_new (NULL);
}

/**
 * rsvg_set_default_dpi
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the all future outgoing pixbufs. Common values are
 * 72, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.2
 */
void
rsvg_set_default_dpi (double dpi)
{
	if (dpi <= 0.)
		internal_dpi = RSVG_DEFAULT_DPI;
	else
		internal_dpi = dpi;
}

/**
 * rsvg_handle_set_dpi
 * @handle: An #RsvgHandle
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 72, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.2
 */
void
rsvg_handle_set_dpi (RsvgHandle * handle, double dpi)
{
	g_return_if_fail (handle != NULL);
	
    if (dpi <= 0.)
        handle->dpi = internal_dpi;
    else
        handle->dpi = dpi;
}

/**
 * rsvg_handle_set_size_callback:
 * @handle: An #RsvgHandle
 * @size_func: A sizing function, or %NULL
 * @user_data: User data to pass to @size_func, or %NULL
 * @user_data_destroy: Destroy function for @user_data, or %NULL
 *
 * Sets the sizing function for the @handle.  This function is called right
 * after the size of the image has been loaded.  The size of the image is passed
 * in to the function, which may then modify these values to set the real size
 * of the generated pixbuf.  If the image has no associated size, then the size
 * arguments are set to -1.
 **/
void
rsvg_handle_set_size_callback (RsvgHandle     *handle,
							   RsvgSizeFunc    size_func,
							   gpointer        user_data,
							   GDestroyNotify  user_data_destroy)
{
	g_return_if_fail (handle != NULL);
	
	if (handle->user_data_destroy)
		(* handle->user_data_destroy) (handle->user_data);
	
	handle->size_func = size_func;
	handle->user_data = user_data;
	handle->user_data_destroy = user_data_destroy;
}

/**
 * rsvg_handle_write:
 * @handle: An #RsvgHandle
 * @buf: Pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: return location for errors
 *
 * Loads the next @count bytes of the image.  This will return #TRUE if the data
 * was loaded successful, and #FALSE if an error occurred.  In the latter case,
 * the loader will be closed, and will not accept further writes. If FALSE is
 * returned, @error will be set to an error from the #RSVG_ERROR domain.
 *
 * Returns: #TRUE if the write was successful, or #FALSE if there was an
 * error.
 **/
gboolean
rsvg_handle_write (RsvgHandle    *handle,
				   const guchar  *buf,
				   gsize          count,
				   GError       **error)
{
	if (handle->write)
		return (*handle->write) (handle, buf, count, error);

	return FALSE;
}

/**
 * rsvg_handle_close:
 * @handle: A #RsvgHandle
 * @error: A #GError
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return #TRUE if the loader closed successfully.  Note that @handle isn't
 * freed until @rsvg_handle_free is called.
 *
 * Returns: #TRUE if the loader closed successfully, or #FALSE if there was
 * an error.
 **/
gboolean
rsvg_handle_close (RsvgHandle  *handle,
				   GError     **error)
{
	if (handle->close)
		return (*handle->close) (handle, error);

	return FALSE;
}

/**
 * rsvg_handle_get_pixbuf:
 * @handle: An #RsvgHandle
 *
 * Returns the pixbuf loaded by #handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Returns: the pixbuf loaded by #handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle *handle)
{
	g_return_val_if_fail (handle != NULL, NULL);
	
	if (handle->pixbuf)
		return g_object_ref (handle->pixbuf);

	return NULL;
}

/**
 * rsvg_handle_free:
 * @handle: An #RsvgHandle
 *
 * Frees #handle.
 **/
void
rsvg_handle_free (RsvgHandle *handle)
{
	if (handle->free)
		(*handle->free) (handle);
}

