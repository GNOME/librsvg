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
#include "rsvg-shapes.h"

#include <glib/gmem.h>
#include <glib/gmessages.h>
#include <glib/gstrfuncs.h>
#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_render_mask.h>
#include <string.h>
#include <math.h>

#include "rsvg-css.h"

typedef struct _RsvgPaintServerSolid RsvgPaintServerSolid;
typedef struct _RsvgPaintServerLinGrad RsvgPaintServerLinGrad;
typedef struct _RsvgPaintServerRadGrad RsvgPaintServerRadGrad;
typedef struct _RsvgPaintServerPattern RsvgPaintServerPattern;

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

struct _RsvgPaintServerPattern {
	RsvgPaintServer super;
	RsvgPattern *pattern;
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

	if (unitlen == 0 || pointlen == 0) {
		x2 = y2 = 0;
	} else {
		x2 = x1 + ychange / unitlen * pointlen;
		y2 = y1 - xchange / unitlen * pointlen;
	}

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

#include <libart_lgpl/art_rgb.h>
#include <libart_lgpl/art_render.h>

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
			py = y;// + (z->y * z->affine[3] + z->affine[5]);
			
			gx = px * z->invaffine[0] + py * z->invaffine[2] + z->invaffine[4] - z->x;
			gy = px * z->invaffine[1] + py * z->invaffine[3] + z->invaffine[5] - z->y;

			gnx = floor (gx / z->width);
			gny = floor (gy / z->height);
	
			sx = px - gnx * z->width * z->affine[0] - gny * z->height * z->affine[2] - z->affine[4]
				+ z->xoffset + tx;
			sy = py - gnx * z->width * z->affine[1] - gny * z->height * z->affine[3] - z->affine[5]
				+ z->yoffset + ty;

			/*printf("(%i, %i)->(%i, %i)\n", i, y, sx, sy);*/

			if (sx < 0 || sx >= z->realwidth || sy < 0 || sy >= z->realheight)
				{
					render->image_buf[i * 4 + 3] = 0;
					continue;
				}
			render->image_buf[i * 4] = z->pixels[sx * 4 + z->rowstride * sy];
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
render_image_pattern (ArtRender *render, gchar * pixels, gdouble x, gdouble y, 
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

	art_affine_invert(image_source->invaffine, affine);

	image_source->init = ART_FALSE;
	
	art_render_add_image_source (render, &image_source->super);
}

static void
rsvg_paint_server_pattern_free (RsvgPaintServer *self)
{
	RsvgPaintServerPattern *z = (RsvgPaintServerPattern *)self;
	g_free (z);
}


static void
rsvg_paint_server_pattern_render (RsvgPaintServer *self, ArtRender *ar,
								   const RsvgPSCtx *ctx)
{
	RsvgPaintServerPattern *z = (RsvgPaintServerPattern *)self;
	RsvgPattern *pattern = z->pattern;
	RsvgDefsDrawable *drawable = (RsvgDefsDrawable *)pattern->g;
	RsvgHandle *hctx = ctx->ctx;
	double affine[6];
	double caffine[6];
	int i, j;

	if (ctx->ctx == NULL)
		return;

	gdouble minx, miny, maxx, maxy, xcoord, ycoord, xoffset, yoffset;
	
	GdkPixbuf *save, *render;

	if (pattern->obj_bbox) {
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

	if (pattern->vbox) {
		caffine[0] = pattern->width / pattern->vbw;
		caffine[1] = 0.;		
		caffine[2] = 0.;
		caffine[3] = pattern->height / pattern->vbh;
		caffine[4] = - pattern->vbx * pattern->width / pattern->vbw;
		caffine[5] = - pattern->vby * pattern->height / pattern->vbh;
		art_affine_multiply(caffine, caffine, affine);		
	}
	else if (pattern->obj_cbbox) {
		caffine[0] = ctx->x1 - ctx->x0;
		caffine[1] = 0.;		
		caffine[2] = 0.;
		caffine[3] = ctx->y1 - ctx->y0;
		caffine[4] = ctx->x0;
		caffine[5] = ctx->y0;

	} else {
		for (i = 0; i < 6; i++)
			caffine[i] = ctx->affine[i];
	}

	art_affine_multiply(affine, affine, pattern->affine);
	art_affine_multiply(caffine, caffine, pattern->affine);

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

	if (minx < 0)
		xoffset = -minx;
	if (miny < 0)
		yoffset = -miny;
	if (maxx > gdk_pixbuf_get_width(hctx->pixbuf))
		xoffset = -minx;
	if (maxy > gdk_pixbuf_get_height(hctx->pixbuf))
		yoffset = -maxy;	

	render = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 gdk_pixbuf_get_width(hctx->pixbuf), 
							 gdk_pixbuf_get_height(hctx->pixbuf));

	gdk_pixbuf_fill(render, 0x00000000);	
	save = hctx->pixbuf;

	hctx->pixbuf = render;

	rsvg_state_push(ctx->ctx);
	
	caffine[4] += xoffset;
	caffine[5] += yoffset;

	for (i = 0; i < 6; i++)
		{
			rsvg_state_current(hctx)->personal_affine[i] = caffine[i];
			rsvg_state_current(hctx)->affine[i] = caffine[i];
		}

	if (((RsvgDefsDrawableGroup *)drawable)->children->len ||
		pattern->gfallback == NULL)
		rsvg_defs_drawable_draw (drawable, hctx, 2);
	else
		rsvg_defs_drawable_draw ((RsvgDefsDrawable *)pattern->gfallback, hctx, 2);		

	rsvg_state_pop(ctx->ctx);

  	hctx->pixbuf = save;

	render_image_pattern (ar, gdk_pixbuf_get_pixels (render),
						  pattern->x, pattern->y, 
						  pattern->width, pattern->height, 
						  gdk_pixbuf_get_width (render),
						  gdk_pixbuf_get_height (render),
						  gdk_pixbuf_get_rowstride (render), 
						  xoffset, yoffset, affine);
}

static RsvgPaintServer *
rsvg_paint_server_pattern (RsvgPattern *pattern)
{
	RsvgPaintServerPattern *result = g_new (RsvgPaintServerPattern, 1);
	
	result->super.refcnt = 1;
	result->super.free = rsvg_paint_server_pattern_free;
	result->super.render = rsvg_paint_server_pattern_render;
	
	result->pattern = pattern;
	
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
				case RSVG_DEF_PATTERN:
					return rsvg_paint_server_pattern ((RsvgPattern *)val);
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
