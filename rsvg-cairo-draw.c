/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-shapes.c: Draw shapes with libart

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2005 Red Hat, Inc.

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
            Carl Worth <cworth@cworth.org>
*/

#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-render.h"
#include "rsvg-styles.h"
#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-filter.h"
#include "rsvg-structure.h"

#include <math.h>

static void
_pattern_add_rsvg_color_stops (cairo_pattern_t *pattern,
							   GPtrArray       *stops,
							   guint32          current_color_rgb,
							   guint8           opacity)
{
	gsize i;
	RsvgGradientStop *stop;
	RsvgNode *node;
	guint32 rgba;

	for (i=0; i < stops->len; i++) {
		node = (RsvgNode*) g_ptr_array_index (stops, i);
		if (node->type != RSVG_NODE_STOP)
			continue;
		stop = (RsvgGradientStop*) node;
		if (stop->is_current_color)
			rgba = current_color_rgb << 8;
		else
			rgba = stop->rgba;
		cairo_pattern_add_color_stop_rgba (pattern, stop->offset,
										   ((rgba >> 24) & 0xff) / 255.0,
										   ((rgba >> 16) & 0xff) / 255.0,
										   ((rgba >>  8) & 0xff) / 255.0,
										   (((rgba >>  0) & 0xff) * opacity)/255.0);
	}
}

static void
_set_source_rsvg_linear_gradient (cairo_t            *cr,
								  RsvgLinearGradient *linear,
								  guint32             current_color_rgb,
								  guint8              opacity)
{
	cairo_pattern_t *pattern;

	if (linear->has_current_color)
		current_color_rgb = linear->current_color;

	pattern = cairo_pattern_create_linear (linear->x1, linear->y1,
										   linear->x2, linear->y2);

	_pattern_add_rsvg_color_stops (pattern, linear->super.children,
								   current_color_rgb, opacity);


	cairo_set_source (cr, pattern);
	cairo_pattern_destroy (pattern);
}

static void
_set_source_rsvg_radial_gradient (cairo_t            *cr,
								  RsvgRadialGradient *radial,
								  guint32             current_color_rgb,
								  guint8              opacity)
{
	cairo_pattern_t *pattern;

	if (radial->has_current_color)
		current_color_rgb = radial->current_color;

	/* XXX: These are most likely quite bogus. */
	pattern = cairo_pattern_create_radial (radial->cx, radial->cy, radial->r,
										   radial->fx, radial->fy, radial->r);

	_pattern_add_rsvg_color_stops (pattern, radial->super.children,
								   current_color_rgb, opacity);

	cairo_set_source (cr, pattern);
	cairo_pattern_destroy (pattern);
}

static void
_set_source_rsvg_solid_colour (cairo_t         *cr,
							   RsvgSolidColour *colour,
							   guint8           opacity)
{
	guint32 rgb = colour->rgb;
	double r = ((rgb >> 16) & 0xff) / 255.0;
	double g = ((rgb >>  8) & 0xff) / 255.0;
	double b = ((rgb >>  0) & 0xff) / 255.0;

	if (opacity == 0xff)
		cairo_set_source_rgb (cr, r, g, b);
	else
		cairo_set_source_rgba (cr, r, g, b,
							   opacity / 255.0);
}

static void
_set_source_rsvg_pattern (cairo_t     *cr,
						  RsvgPattern *pattern,
						  guint8       opacity)
{
	/* XXX: NYI */
	cairo_set_source_rgb (cr, 0.0, 1.0, 1.0);
}

static void
_set_source_rsvg_paint_server (cairo_t         *cr,
							   guint32          current_color_rgb,
							   RsvgPaintServer *ps,
							   guint8           opacity)
{
	switch (ps->type) {
	case RSVG_PAINT_SERVER_LIN_GRAD:
		_set_source_rsvg_linear_gradient (cr, ps->core.lingrad,
										  current_color_rgb, opacity);
		break;
	case RSVG_PAINT_SERVER_RAD_GRAD:
		_set_source_rsvg_radial_gradient (cr, ps->core.radgrad,
										  current_color_rgb, opacity);
		break;
	case RSVG_PAINT_SERVER_SOLID:
		_set_source_rsvg_solid_colour (cr, ps->core.colour, opacity);
		break;
	case RSVG_PAINT_SERVER_PATTERN:
		_set_source_rsvg_pattern (cr, ps->core.pattern, opacity);
		break;
	}
}

static void
_set_rsvg_affine (cairo_t *cr, const double affine[6])
{
	cairo_matrix_t matrix;

	cairo_matrix_init (&matrix,
					   affine[0], affine[1],
					   affine[2], affine[3],
					   affine[4], affine[5]);
	cairo_set_matrix (cr, &matrix);
}

void
rsvg_cairo_render_path (RsvgDrawingCtx *ctx, const RsvgBpathDef *bpath_def)
{
	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	RsvgState *state = rsvg_state_current (ctx);
	cairo_t *cr = render->cr;
	RsvgBpath *bpath;
	int i;

	cairo_save (cr);

	_set_rsvg_affine (cr, state->affine);

	cairo_set_line_width (cr, state->stroke_width);
	cairo_set_miter_limit (cr, state->miter_limit);
	cairo_set_line_cap (cr, (cairo_line_cap_t)state->cap);
	cairo_set_line_join (cr, (cairo_line_join_t)state->join);
	cairo_set_dash (cr, state->dash.dash, state->dash.n_dash, state->dash.offset);

	for (i=0; i < bpath_def->n_bpath; i++) {
		bpath = &bpath_def->bpath[i];
		switch (bpath->code) {
		case RSVG_MOVETO:
			cairo_close_path (cr);
			/* fall-through */
		case RSVG_MOVETO_OPEN:
			cairo_move_to (cr, bpath->x3, bpath->y3);
			break;
		case RSVG_CURVETO:
			cairo_curve_to (cr,
							bpath->x1, bpath->y1,
							bpath->x2, bpath->y2,
							bpath->x3, bpath->y3);
			break;
		case RSVG_LINETO:
			cairo_line_to (cr, bpath->x3, bpath->y3);
			break;
		case RSVG_END:
			break;
		}
	}

	if (state->fill != NULL) {
		if (state->fill_rule == FILL_RULE_EVENODD)
			cairo_set_fill_rule (cr, CAIRO_FILL_RULE_EVEN_ODD);
		else /* state->fill_rule == FILL_RULE_NONZERO */                                       
			cairo_set_fill_rule (cr, CAIRO_FILL_RULE_WINDING);

		_set_source_rsvg_paint_server (cr,
									   state->current_color,
									   state->fill,
									   state->fill_opacity);

		if (state->stroke != NULL)
			cairo_fill_preserve (cr);
		else
			cairo_fill (cr);
	}

	if (state->stroke != NULL) {
		_set_source_rsvg_paint_server (cr,
									   state->current_color,
									   state->stroke,
									   state->stroke_opacity);

		cairo_stroke (cr);
	}
			
	cairo_restore (cr);
}

void rsvg_cairo_render_image (RsvgDrawingCtx *ctx, const GdkPixbuf * img, 
							  double x, double y, double w, double h)
{
	/* XXX: Untested */

	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	RsvgState *state = rsvg_state_current(ctx);
	cairo_surface_t * surface;
	unsigned char * data = gdk_pixbuf_get_pixels(img);

    cairo_save (render->cr);

    surface = cairo_image_surface_create_for_data (data, CAIRO_FORMAT_RGB24,
												   w, h, w * 4);
    cairo_translate (render->cr, x, y);

    cairo_set_source_surface (render->cr, surface, 0, 0);
    if (state->opacity != 1.0)
		cairo_paint_with_alpha (render->cr, state->opacity);
    else
		cairo_paint (render->cr);
    
    cairo_surface_destroy (surface);

    cairo_restore (render->cr);
}

void
rsvg_cairo_push_discrete_layer (RsvgDrawingCtx *ctx)
{
	/* XXX: NYI */

#if 0

	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	cairo_surface_t *surface;
	cairo_t *child_cr;
	
	surface = cairo_surface_create_similar (render->surface,
											CAIRO_CONTENT_COLOR_ALPHA,
											width, height);
	child_cr = cairo_create (surface);
	cairo_surface_destroy (surface);
	
	render->cr_stack = g_list_prepend(render->cr_stack, render->cr);	
	render->cr = child_cr;   

#endif
}

void
rsvg_cairo_pop_discrete_layer (RsvgDrawingCtx *ctx)
{
	/* XXX: NYI */

#if 0
	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	cairo_t *child_cr = render->cr;

	render->cr = (cairo_t *)render->cr_stack->data;
	render->cr_stack = g_list_remove_link (render->cr_stack, render->cr_stack);

	cairo_set_source_surface (render->cr,
							  cairo_get_target (child_cr),
							  0, 0);
	cairo_paint (render->cr);
	cairo_destroy (child_cr);
#endif
}

void 
rsvg_cairo_add_clipping_rect (RsvgDrawingCtx *ctx,
							  double x, double y,
							  double w, double h)
{
	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	cairo_t *cr = render->cr;

	cairo_rectangle (cr, x, y, w, h);
	cairo_clip (cr);
}

static cairo_status_t png_write_func(void * closure,
									 const unsigned char *data,
									 unsigned int	   length)
{
	g_byte_array_append ((GByteArray *)closure, data, length);
	return CAIRO_STATUS_SUCCESS;
}

void * 
rsvg_cairo_get_image_of_node (RsvgDrawingCtx *ctx,
							  RsvgNode       *drawable,
							  double          width,
							  double          height)
{
	/* XXX: Untested */

	GdkPixbuf *img = NULL;
	cairo_surface_t * surface;
	cairo_t * cr;
	GByteArray *png_bytes;

	RsvgCairoRender *save_render = (RsvgCairoRender *)ctx->render;
	RsvgCairoRender *render;

	surface = cairo_surface_create_similar(save_render->surface,
										   CAIRO_CONTENT_COLOR_ALPHA,
										   width, height);
	cr = cairo_create (surface);
	cairo_surface_destroy (surface);

	render = rsvg_cairo_render_new(cr);
	ctx->render = (RsvgRender *)render;

	rsvg_state_push(ctx);	
	rsvg_node_draw (drawable, ctx, 0);	
	rsvg_state_pop(ctx);
	
	png_bytes = g_byte_array_new();
	if(CAIRO_STATUS_SUCCESS == cairo_surface_write_to_png_stream (surface,
																  png_write_func,
																  png_bytes)) {
		GdkPixbufLoader* img_loader = gdk_pixbuf_loader_new ();
		gdk_pixbuf_loader_write (img_loader, png_bytes->data, png_bytes->len, NULL);
		gdk_pixbuf_loader_close (img_loader, NULL);
		img = gdk_pixbuf_loader_get_pixbuf (img_loader);
		g_object_unref (G_OBJECT (img_loader));
	}
	g_byte_array_free(png_bytes, TRUE);

	cairo_destroy (cr);
	ctx->render = (RsvgRender *)save_render;
	
	return img;
}
