/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-art-paint-server.c: Implement the SVG paint servers using libart
 
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
#include "rsvg-art-paint-server.h"
#include "rsvg-styles.h"
#include "rsvg-image.h"
#include "rsvg-art-render.h"

#include <glib/gmem.h>
#include <glib/gmessages.h>
#include <glib/gstrfuncs.h>
#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_render_mask.h>
#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_render.h>
#include <string.h>
#include <math.h>

static ArtGradientStop *
rsvg_paint_art_stops_from_rsvg (GPtrArray *rstops, guint32 * nstops,
								guint32 current_color)
{
	ArtGradientStop *stops;
	unsigned int len = rstops->len;
	unsigned int i, j;
	
	j = 0;
	for (i = 0; i < len; i++)
		if (((RsvgNode *)g_ptr_array_index(rstops, i))->type == RSVG_NODE_STOP)
			j++;

	*nstops = j;
	stops = g_new (ArtGradientStop, j);

	j = 0;

	for (i = 0; i < len; i++)
		{
			RsvgGradientStop * stop;
			RsvgNode * temp;
			guint32 rgba;
			guint32 r, g, b, a;

			temp = (RsvgNode *)g_ptr_array_index(rstops, i);
			if (temp->type != RSVG_NODE_STOP)
				continue;
			stop = (RsvgGradientStop *)temp;
			stops[j].offset = stop->offset;
			if (!stop->is_current_color)
				rgba = stop->rgba;
			else
				rgba = current_color << 8;
			/* convert from separated to premultiplied alpha */
			a = stop->rgba & 0xff;
			r = (rgba >> 24) * a + 0x80;
			r = (r + (r >> 8)) >> 8;
			g = ((rgba >> 16) & 0xff) * a + 0x80;
			g = (g + (g >> 8)) >> 8;
			b = ((rgba >> 8) & 0xff) * a + 0x80;
			b = (b + (b >> 8)) >> 8;
			stops[j].color[0] = ART_PIX_MAX_FROM_8(r);
			stops[j].color[1] = ART_PIX_MAX_FROM_8(g);
			stops[j].color[2] = ART_PIX_MAX_FROM_8(b);
			stops[j].color[3] = ART_PIX_MAX_FROM_8(a);
			j++;
		}
	return stops;
}


static void
rsvg_art_paint_server_solid_render (RsvgSolidColour *z, ArtRender *ar,
									const RsvgPSCtx *ctx)
{
	ArtPixMaxDepth color[3];
	guint32 rgb = z->rgb;
	if (z->currentcolour)
		rgb = rsvg_state_current(ctx->ctx)->current_color;
	
	color[0] = ART_PIX_MAX_FROM_8 (rgb >> 16);
	color[1] = ART_PIX_MAX_FROM_8 ((rgb >> 8) & 0xff);
	color[2] = ART_PIX_MAX_FROM_8 (rgb  & 0xff);
	
	art_render_image_solid (ar, color);
}

/* This is a fudge factor, we add this to the linear gradient going 
   to libart because libart is retarded */

#define FUDGE 0.00000001

static void
rsvg_art_paint_server_lin_grad_render (RsvgLinearGradient *rlg, ArtRender *ar,
									   const RsvgPSCtx *ctx)
{
	ArtGradientLinear *agl;
	RsvgLinearGradient statgrad = *rlg;
	double x1, y1, x2, y2;
	double dx, dy, scale;
	double affine[6];
	guint32 current_color;
	int i;
	double xchange, ychange, pointlen,unitlen;
	double cx, cy, cxt, cyt;
	double px, py, pxt, pyt;
	double x2t, y2t;

	rlg = &statgrad;
	rsvg_linear_gradient_fix_fallback(rlg);

	if (rlg->has_current_color)
		current_color = rlg->current_color;
	else
		current_color = ctx->color;

	agl = g_new (ArtGradientLinear, 1);
	agl->stops = rsvg_paint_art_stops_from_rsvg (rlg->super.children, 
												 &agl->n_stops, current_color);

	if (agl->n_stops == 0)
		{
			g_free (agl->stops);
			g_free (agl);
			return;
		}

	if (rlg->obj_bbox) {
		affine[0] = ctx->x1 - ctx->x0;
		affine[1] = 0.;		
		affine[2] = 0.;
		affine[3] = ctx->y1 - ctx->y0;
		affine[4] = ctx->x0;
		affine[5] = ctx->y0;
		_rsvg_affine_multiply(affine, affine, ctx->affine);
	} else
		for (i = 0; i < 6; i++)
			affine[i] = ctx->affine[i];

	_rsvg_affine_multiply(affine, rlg->affine, affine);

	/*
	in case I am hit by a bus, here is how the following code works:

	in the spec, the various transformations are not applied to the coordinates
	that determine the gradient, they are applied to the gradient itself, which
	is logical. However after transformation, these things become different.
	which is where this code comes in. The effective gradient is two things:
	the slope of the lines of the same colour (perpendicular to the gradient 
	without transform), and the distance between the first and the last of 
	these strips. What this code figures out, is the slope of the lines of
	equal colour, and the distance between them. It transforms them both and 
	finally spits out a new two point gradient which is basically the original
	(x1, y1) point, and the second point which is the point on the line where
	(x2, y2) lay that is closest to (x1, y1)

	I'm not sure if this is the right solution to the problem, but it works
	for now.

	***Start explained section***/

	/*calculate (nx2, ny2), the point perpendicular to the gradient*/
	cx = (rlg->x2 + rlg->x1) / 2;
	cy = (rlg->y2 + rlg->y1) / 2;
	xchange = cx - rlg->x1;
	ychange = cy - rlg->y1;
	px = cx - ychange;
	py = cy + xchange;

	/* compute [xy][12] in pixel space */
	cxt = cx * affine[0] + cy * affine[2] + affine[4];
	cyt = cx * affine[1] + cy * affine[3] + affine[5];
	x2t = rlg->x2 * affine[0] + rlg->y2 * affine[2] + affine[4];
	y2t = rlg->x2 * affine[1] + rlg->y2 * affine[3] + affine[5];
	pxt = px * affine[0] + py * affine[2] + affine[4];
	pyt = px * affine[1] + py * affine[3] + affine[5];

	pointlen = ((pxt - cxt)*(cyt - y2t)  - (cxt - x2t)*(pyt - cyt)) / 
		sqrt((pxt - cxt) * (pxt - cxt) + (pyt - cyt) * (pyt - cyt));

	xchange = pxt - cxt;
	ychange = pyt - cyt;
	unitlen = sqrt(xchange*xchange + ychange*ychange);

	if (unitlen == 0) {
		x2 = x1 = cxt; 
		y2 = y1 = cyt;
	} else {
		x1 = cxt - ychange / unitlen * pointlen;
		y1 = cyt + xchange / unitlen * pointlen;
		x2 = cxt + ychange / unitlen * pointlen;
		y2 = cyt - xchange / unitlen * pointlen;
	}

	/***end explained section***/

	/* solve a, b, c so ax1 + by1 + c = 0 and ax2 + by2 + c = 1, maximum
	   gradient is in x1,y1 to x2,y2 dir */
	dx = x2 - x1;
	dy = y2 - y1;

	/* workaround for an evil devide by 0 bug - not sure if this is sufficient */

	if (fabs(dx) + fabs(dy) <= 0.0000001)
		scale = 100000000.;
	else
		scale = 1.0 / (dx * dx + dy * dy);
	agl->a = dx * scale + FUDGE;
	agl->b = dy * scale + FUDGE;
	agl->c = -(x1 * agl->a + y1 * agl->b) + FUDGE;

	agl->spread = rlg->spread;

	art_render_gradient_linear (ar, agl, ART_FILTER_NEAREST);

	g_free (agl->stops);
	g_free (agl);
}

static void
rsvg_art_paint_server_rad_grad_render (RsvgRadialGradient *rrg, ArtRender *ar,
									   const RsvgPSCtx *ctx)
{
	RsvgRadialGradient statgrad = *rrg;
	ArtGradientRadial *agr;
	double aff1[6], aff2[6], affine[6];
	guint32 current_color;
	int i;
	rrg = &statgrad;
	rsvg_radial_gradient_fix_fallback(rrg);

	if (rrg->obj_bbox) {
		affine[0] = ctx->x1 - ctx->x0;
		affine[1] = 0.;		
		affine[2] = 0.;
		affine[3] = ctx->y1 - ctx->y0;
		affine[4] = ctx->x0;
		affine[5] = ctx->y0;
		_rsvg_affine_multiply(affine, affine, ctx->affine);
	} else {
		for (i = 0; i < 6; i++)
			affine[i] = ctx->affine[i];
	}

	_rsvg_affine_multiply(affine, rrg->affine, affine);

	if (rrg->has_current_color)
		current_color = rrg->current_color;
	else
		current_color = ctx->color;
	
	agr = g_new (ArtGradientRadial, 1);
	agr->stops = rsvg_paint_art_stops_from_rsvg (rrg->super.children, 
												 &agr->n_stops, current_color);
	
	if (agr->n_stops == 0)
		{
			g_free (agr->stops);
			g_free (agr);
			return;
		}

	_rsvg_affine_scale (aff1, rrg->r, rrg->r);
	_rsvg_affine_translate (aff2, rrg->cx, rrg->cy);
	_rsvg_affine_multiply (aff1, aff1, aff2);
	_rsvg_affine_multiply (aff1, aff1, affine);
	_rsvg_affine_invert (agr->affine, aff1);
	
	/* todo: libart doesn't support spreads on radial gradients */

	agr->fx = (rrg->fx - rrg->cx) / rrg->r;
	agr->fy = (rrg->fy - rrg->cy) / rrg->r;
	
	art_render_gradient_radial (ar, agr, ART_FILTER_NEAREST);

	g_free (agr->stops);
	g_free (agr);
}


typedef struct {
	ArtImageSource super;
	gchar * pixels;
	gdouble x, y, width, height, xoffset, yoffset;
	gint realwidth, realheight; 
	gint rowstride;
	art_boolean init;
	gdouble affine[6];
	gdouble invaffine[6];
} RsvgImageSourcePattern;

static void
render_image_pattern_done (ArtRenderCallback *self, ArtRender *render)
{
	RsvgImageSourcePattern *z;
	z = (RsvgImageSourcePattern *) self;
	g_free(z->pixels);
	g_free(self);
}

/*the commented out regions in the next bit allow overflow*/

static void
render_image_pattern_render(ArtRenderCallback *self, ArtRender *render,
							art_u8 *dest, int y)
{
	RsvgImageSourcePattern *z = (RsvgImageSourcePattern *)self;
	int i;
	int x0 = render->x0;
	int x1 = render->x1;

	int sx, sy;
	double px, py, gx, gy, gnx, gny, tx, ty;

	tx = -z->x * z->affine[0] + -z->y * z->affine[2] + z->affine[4];
	ty = -z->x * z->affine[1] + -z->y * z->affine[3] + z->affine[5];

	for (i = 0; i < x1 - x0; i++)
		{
			px = i;
			py = y;
			
			gx = px * z->invaffine[0] + py * z->invaffine[2] + z->invaffine[4] - z->x;
			gy = px * z->invaffine[1] + py * z->invaffine[3] + z->invaffine[5] - z->y;
			
			gnx = floor (gx / z->width);
			gny = floor (gy / z->height);
	
			sx = px - gnx * z->width * z->affine[0] - gny * z->height * z->affine[2] - z->affine[4]
				+ z->xoffset + tx;
			sy = py - gnx * z->width * z->affine[1] - gny * z->height * z->affine[3] - z->affine[5]
				+ z->yoffset + ty;
			
			if (sx < 0 || sx >= z->realwidth || sy < 0 || sy >= z->realheight){
				render->image_buf[i * 4 + 3] = 0;
				continue;
			}
			render->image_buf[i * 4 + 0] = z->pixels[sx * 4 + z->rowstride * sy];
			render->image_buf[i * 4 + 1] = z->pixels[sx * 4 + z->rowstride * sy + 1];
			render->image_buf[i * 4 + 2] = z->pixels[sx * 4 + z->rowstride * sy + 2];
			render->image_buf[i * 4 + 3] = z->pixels[sx * 4 + z->rowstride * sy + 3];
			
		}
}

static void
render_image_pattern_negotiate (ArtImageSource *self, ArtRender *render,
				  ArtImageSourceFlags *p_flags,
				  int *p_buf_depth, ArtAlphaType *p_alpha)
{
	self->super.render = render_image_pattern_render;
	*p_flags = 0;
	*p_buf_depth = 8;
	*p_alpha = ART_ALPHA_SEPARATE;
}

static void
render_image_pattern (ArtRender *render, guchar * pixels, gdouble x, gdouble y, 
					  gdouble width, gdouble height, gint realwidth, gint realheight, gint rowstride,
					  gdouble xoffset, gdouble yoffset, double * affine)
{	
	RsvgImageSourcePattern *image_source;
	int i;
	
	image_source = art_new (RsvgImageSourcePattern, 1);
	image_source->super.super.render = NULL;
	image_source->super.super.done = render_image_pattern_done;
	image_source->super.negotiate = render_image_pattern_negotiate;
	
	image_source->pixels = g_new(gchar, rowstride * realheight);
	
	image_source->rowstride = rowstride;
	image_source->width = width;
	image_source->height = height;
	image_source->realwidth = realwidth;
	image_source->realheight = realheight;
	image_source->x = x;
	image_source->y = y;
	image_source->xoffset = xoffset;
	image_source->yoffset = yoffset;
		
	for (i = 0; i < rowstride * realheight; i++)
		image_source->pixels[i] = pixels[i];  
	
	for (i = 0; i < 6; i++)
		image_source->affine[i] = affine[i];  

	_rsvg_affine_invert(image_source->invaffine, affine);

	image_source->init = ART_FALSE;
	
	art_render_add_image_source (render, &image_source->super);
}

static void
rsvg_art_paint_server_pattern_render (RsvgPattern *pattern, ArtRender *ar,
									  const RsvgPSCtx *ctx)
{
	RsvgPattern statpat = *pattern;
	RsvgNode *drawable = (RsvgNode *)&statpat;
	RsvgDrawingCtx *hctx = ctx->ctx;
	GdkPixbuf *pixbuf = ((RsvgArtRender *)hctx->render)->pixbuf; 
	double affine[6];
	double caffine[6];
	int i, j;
	gdouble minx, miny, maxx, maxy, xcoord, ycoord, xoffset, yoffset;
	GdkPixbuf *save, *render;

	pattern = &statpat;
	rsvg_pattern_fix_fallback(pattern);

	if (ctx->ctx == NULL)
		return;

	if (pattern->obj_bbox) {
		affine[0] = ctx->x1 - ctx->x0;
		affine[1] = 0.;		
		affine[2] = 0.;
		affine[3] = ctx->y1 - ctx->y0;
		affine[4] = ctx->x0;
		affine[5] = ctx->y0;
		_rsvg_affine_multiply(affine, affine, ctx->affine);
	} else {
		for (i = 0; i < 6; i++)
			affine[i] = ctx->affine[i];
	}

	if (pattern->vbox) {
		double w, h, x, y;
		w = pattern->width;
		h = pattern->height;
		x = 0;
		y = 0;

		rsvg_preserve_aspect_ratio(pattern->preserve_aspect_ratio,
								   pattern->vbw, pattern->vbh, 
								   &w, &h, &x, &y);

		x -= pattern->vbx * w / pattern->vbw;
		y -= pattern->vby * h / pattern->vbh;

		caffine[0] = w / pattern->vbw;
		caffine[1] = 0.;		
		caffine[2] = 0.;
		caffine[3] = h / pattern->vbh;
		caffine[4] = x;
		caffine[5] = y;
		_rsvg_affine_multiply(caffine, caffine, affine);
	}
	else if (pattern->obj_cbbox) {
		caffine[0] = ctx->x1 - ctx->x0;
		caffine[1] = 0.;		
		caffine[2] = 0.;
		caffine[3] = ctx->y1 - ctx->y0;
		caffine[4] = ctx->x0;
		caffine[5] = ctx->y0;
		_rsvg_affine_multiply(caffine, caffine, ctx->affine);
	} else {
		for (i = 0; i < 6; i++)
			caffine[i] = ctx->affine[i];
	}

	_rsvg_affine_multiply(affine, affine, pattern->affine);
	_rsvg_affine_multiply(caffine, caffine, pattern->affine);

	/*check if everything is going to be within the boundaries of the rendering surface*/
	maxx = maxy = minx = miny = xoffset = yoffset = 0;

	for (i = 0; i < 2; i++)
		for (j = 0; j < 2; j++)
			{
				xcoord = affine[0] * pattern->width * i + affine[2] * pattern->height * j + affine[4];
				ycoord = affine[1] * pattern->width * i + affine[3] * pattern->height * j + affine[5];
				if (xcoord < minx)
					minx = xcoord;
				if (xcoord > maxx)
					maxx = xcoord;
				if (ycoord < miny)
					miny = ycoord;
				if (ycoord > maxy)
					maxy = ycoord;
			}

	xoffset = -minx;
	yoffset = -miny;

	render = _rsvg_pixbuf_new_cleared(GDK_COLORSPACE_RGB, 1, 8, 
									  maxx - minx, maxy - miny);

	save = pixbuf;

	((RsvgArtRender *)hctx->render)->pixbuf = render;

	rsvg_state_push(ctx->ctx);
	
	caffine[4] += xoffset;
	caffine[5] += yoffset;

	for (i = 0; i < 6; i++)
		{
			rsvg_state_current(hctx)->personal_affine[i] = caffine[i];
			rsvg_state_current(hctx)->affine[i] = caffine[i];
		}

	_rsvg_node_draw_children (drawable, hctx, 2);

	rsvg_state_pop(ctx->ctx);

  	((RsvgArtRender *)hctx->render)->pixbuf = save;

	render_image_pattern (ar, gdk_pixbuf_get_pixels (render),
						  pattern->x, pattern->y, 
						  pattern->width, pattern->height, 
						  gdk_pixbuf_get_width (render),
						  gdk_pixbuf_get_height (render),
						  gdk_pixbuf_get_rowstride (render), 
						  xoffset, yoffset, affine);

	g_object_unref(G_OBJECT(render));
}

void
rsvg_art_render_paint_server (ArtRender *ar, RsvgPaintServer *ps,
							  const RsvgPSCtx *ctx)
{
	switch(ps->type)
		{
		case RSVG_PAINT_SERVER_LIN_GRAD:
			rsvg_art_paint_server_lin_grad_render(ps->core.lingrad, ar, ctx);
			break;
		case RSVG_PAINT_SERVER_RAD_GRAD:
			rsvg_art_paint_server_rad_grad_render(ps->core.radgrad, ar, ctx);
			break;
		case RSVG_PAINT_SERVER_SOLID:
			rsvg_art_paint_server_solid_render(ps->core.colour, ar, ctx);
			break;
		case RSVG_PAINT_SERVER_PATTERN:
			rsvg_art_paint_server_pattern_render(ps->core.pattern, ar, ctx);
			break;
		}
}
