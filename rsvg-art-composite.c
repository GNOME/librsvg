/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-art-composite.c: Composite different layers using gdk pixbuff for our 
   libart backend

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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
*/
#include <string.h>
#include <math.h>

#include "rsvg-art-composite.h"
#include "rsvg-art-render.h"
#include "rsvg-styles.h"
#include "rsvg-structure.h"
#include "rsvg-filter.h"
#include "rsvg-art-mask.h"

#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_rgb_svp.h>

static void
rsvg_pixmap_destroy (gchar *pixels, gpointer data)
{
  g_free (pixels);
}

typedef struct _RsvgArtDiscreteLayer RsvgArtDiscreteLayer;

struct _RsvgArtDiscreteLayer
{
	GdkPixbuf *save_pixbuf;
	ArtIRect underbbox;
	RsvgState * state;
	ArtSVP * clippath_save;
	gboolean clippath_loaded;
};

void
rsvg_art_push_discrete_layer (RsvgDrawingCtx *ctx)
{
	RsvgState *state;
	GdkPixbuf *pixbuf;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;
	art_u8 *pixels;
	int width, height, rowstride;
	RsvgArtDiscreteLayer * layer;

	state = rsvg_state_current(ctx);
	pixbuf = render->pixbuf;

	layer = g_new(RsvgArtDiscreteLayer, 1);
	render->layers = g_slist_prepend(render->layers, layer);
	layer->state = state;
	layer->save_pixbuf = NULL;

	if (state->filter != NULL || state->opacity != 0xFF || 
		state->backgroundnew || state->mask != NULL || state->adobe_blend)
		{
			layer->save_pixbuf = pixbuf;
			layer->underbbox = render->bbox;
			
			render->bbox.x0 = 0;
			render->bbox.x1 = 0;
			render->bbox.y0 = 0;
			render->bbox.y1 = 0;

			if (pixbuf == NULL)
				{
					/* FIXME: What warning/GError here? */
					return;
				}
			
			width = gdk_pixbuf_get_width (pixbuf);
			height = gdk_pixbuf_get_height (pixbuf);
			rowstride = gdk_pixbuf_get_rowstride (pixbuf);
			pixels = g_new (art_u8, rowstride * height);
			memset (pixels, 0, rowstride * height);
			
			pixbuf = gdk_pixbuf_new_from_data (pixels,
											   GDK_COLORSPACE_RGB,
											   TRUE,
											   gdk_pixbuf_get_bits_per_sample (pixbuf),
											   width,
											   height,
											   rowstride,
											   (GdkPixbufDestroyNotify)rsvg_pixmap_destroy,
											   NULL);
			render->pixbuf = pixbuf;
		}
	if (state->clip_path_ref)
		{
	 		ArtSVP * tmppath;
			
			rsvg_state_push(ctx);
			tmppath = rsvg_art_clip_path_render (state->clip_path_ref, ctx);
			rsvg_state_pop(ctx);

		 	layer->clippath_save = render->clippath;
			render->clippath = rsvg_art_clip_path_merge(render->clippath, tmppath, TRUE, 'i');
			if (tmppath)
			 	layer->clippath_loaded = TRUE;
			else
				layer->clippath_loaded = FALSE;
		}
		else
		{
			layer->clippath_save = render->clippath;
			layer->clippath_loaded = FALSE;
		}
}

static void
rsvg_use_opacity (RsvgDrawingCtx *ctx, int opacity, 
				  GdkPixbuf *tos, GdkPixbuf *nos)
{
	art_u8 *tos_pixels, *nos_pixels;
	int width;
	int height;
	int rowstride;
	int x, y;
	int tmp;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;
	
	
	if (tos == NULL || nos == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	if (!gdk_pixbuf_get_has_alpha (nos))
		{
			g_warning (_("push/pop transparency group on non-alpha buffer nyi\n"));
			return;
		}
	
	width = gdk_pixbuf_get_width (tos);
	height = gdk_pixbuf_get_height (tos);
	rowstride = gdk_pixbuf_get_rowstride (tos);
	
	tos_pixels = gdk_pixbuf_get_pixels (tos);
	nos_pixels = gdk_pixbuf_get_pixels (nos);

	tos_pixels += rowstride * MAX(render->bbox.y0, 0);
	nos_pixels += rowstride * MAX(render->bbox.y0, 0);
	
	for (y = MAX(render->bbox.y0, 0); y < MIN(render->bbox.y1 + 1, height); y++)
		{
			for (x = MAX(render->bbox.x0, 0); x < MIN(render->bbox.x1 + 1, width); x++)
				{
					art_u8 r, g, b, a;
					a = tos_pixels[4 * x + 3];
					if (a)
						{
							r = tos_pixels[4 * x];
							g = tos_pixels[4 * x + 1];
							b = tos_pixels[4 * x + 2];
							tmp = a * opacity + 0x80;
							a = (tmp + (tmp >> 8)) >> 8;
							art_rgba_run_alpha (nos_pixels + 4 * x, r, g, b, a, 1);
						}
				}
			tos_pixels += rowstride;
			nos_pixels += rowstride;
		}
}

static GdkPixbuf *
get_next_out(gint * operationsleft, GdkPixbuf * in, GdkPixbuf * tos, 
			 GdkPixbuf * nos, GdkPixbuf *intermediate)
{
	GdkPixbuf * out;

	if (*operationsleft == 1)
		out = nos;
	else
		{ 
			if (in == tos)	
				out = intermediate;
			else
				out = tos;
			gdk_pixbuf_fill(out, 0x00000000);
		}	
	(*operationsleft)--;
	
	return out;
}

static GdkPixbuf *
rsvg_compile_bg(RsvgDrawingCtx *ctx, RsvgState *topstate)
{
	int i, foundstate;
	GdkPixbuf *intermediate, *lastintermediate;
	RsvgArtDiscreteLayer *state, *lastvalid;
	ArtIRect save;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;

	foundstate = 0;	

	lastvalid = render->layers->data;
	lastintermediate = gdk_pixbuf_copy(lastvalid->save_pixbuf);

	lastvalid = NULL;
			
	save = render->bbox;

	render->bbox.x0 = 0;
	render->bbox.y0 = 0;
	render->bbox.x1 = gdk_pixbuf_get_width(render->pixbuf);
	render->bbox.y1 = gdk_pixbuf_get_height(render->pixbuf);

	for (i = 0; (state = g_slist_nth_data(render->layers, i)) != NULL; i++)
		{
			if (state->state == topstate)
				{
					foundstate = 1;
				}
			else if (!foundstate)
				continue;
			if (state->state->backgroundnew)
				break;
			if (state->save_pixbuf)
				{
					if (lastvalid)
						{
							intermediate = gdk_pixbuf_copy(state->save_pixbuf);
							rsvg_use_opacity(ctx, 0xFF, lastintermediate, intermediate);
							g_object_unref(lastintermediate);
							lastintermediate = intermediate;
						}
					lastvalid = state;
				}
		}

	render->bbox = save;
	return lastintermediate;
}

static void
rsvg_composite_layer(RsvgDrawingCtx *ctx, RsvgState *state, GdkPixbuf *tos, GdkPixbuf *nos)
{
	RsvgFilter *filter = state->filter;
	int opacity = state->opacity;
	RsvgDefsDrawable * mask = state->mask;
	GdkPixbuf *intermediate;
	GdkPixbuf *in, *out, *insidebg;
	int operationsleft;
	gint adobe_blend = state->adobe_blend;

	intermediate = NULL;

	operationsleft = 0;
	
	if (opacity != 0xFF)
		operationsleft++;
	if (filter != NULL)
		operationsleft++;
	if (mask != NULL)
		operationsleft++;
	if (adobe_blend)
		operationsleft++;		

	if (operationsleft > 1)
		intermediate = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
									   gdk_pixbuf_get_width (tos),
									   gdk_pixbuf_get_height (tos));

	in = tos;

	if (operationsleft == 0)
		{
			rsvg_use_opacity (ctx, 0xFF, tos, nos);			
		}

	if (filter != NULL || adobe_blend)
		{
			insidebg = rsvg_compile_bg(ctx, state);
		}
	else
		insidebg = NULL;

	if (filter != NULL)
		{
			out = get_next_out(&operationsleft, in, tos, nos, intermediate);
			rsvg_filter_render (filter, in, out, insidebg, ctx);
			in = out;
		}
	if (opacity != 0xFF)
		{
			out = get_next_out(&operationsleft, in, tos, nos, intermediate);
			rsvg_use_opacity (ctx, opacity, in, out);
			in = out;
		}
	if (mask != NULL)
		{
			out = get_next_out(&operationsleft, in, tos, nos, intermediate);
			rsvg_art_mask_render ((RsvgMask *)mask, in, out, ctx);
			in = out;
		}
	if (adobe_blend)
		{
			out = get_next_out(&operationsleft, in, tos, nos, intermediate);
			rsvg_filter_adobe_blend (adobe_blend, in, insidebg, out, ctx);
			in = out;
		}

	if (filter != NULL || adobe_blend)
		{
			g_object_unref (insidebg);
		}

	if (intermediate != NULL)
		g_object_unref (intermediate);

}

/**
 * rsvg_pop_discrete_layer: End a transparency group.
 * @ctx: Context in which to push.
 *
 * Pops a new transparency group from the stack, recompositing with the
 * next on stack using a filter, transperency value, or a mask to do so
 **/

void
rsvg_art_pop_discrete_layer(RsvgDrawingCtx *ctx)
{
	GdkPixbuf *tos, *nos;
	RsvgState *state;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;
	GSList * link;
	RsvgArtDiscreteLayer * layer;

	state = rsvg_state_current(ctx);

	link = g_slist_nth(render->layers, 0);
	layer = link->data;

	if (layer->save_pixbuf)
		{
			tos = render->pixbuf;
			nos = layer->save_pixbuf;
			
			if (nos != NULL)
				rsvg_composite_layer(ctx, state, tos, nos);
			
			g_object_unref (tos);
			render->pixbuf = nos;
			art_irect_union(&render->bbox, &render->bbox, &layer->underbbox);
		}
	if (layer->clippath_loaded)
		{
			art_svp_free(render->clippath);
		}
	render->clippath = layer->clippath_save;
	g_free (layer);
	render->layers = g_slist_delete_link(render->layers, link);
}

gboolean
rsvg_art_needs_discrete_layer(RsvgState *state)
{
	return state->filter || state->mask || state->adobe_blend || state->backgroundnew || state->clip_path_ref;
}

void
rsvg_art_alpha_blt (GdkPixbuf * src, gint srcx, gint srcy, gint srcwidth,
					gint srcheight, GdkPixbuf * dst, gint dstx, gint dsty)
{
	gint rightx;
	gint bottomy;
	gint dstwidth;
	gint dstheight;
	
	gint srcoffsetx;
	gint srcoffsety;
	gint dstoffsetx;
	gint dstoffsety;
	
	gint x, y, srcrowstride, dstrowstride, sx, sy, dx, dy;
	guchar *src_pixels, *dst_pixels;
	
	dstheight = srcheight;
	dstwidth = srcwidth;
	
	rightx = srcx + srcwidth;
	bottomy = srcy + srcheight;
	
	if (rightx > gdk_pixbuf_get_width (src))
		rightx = gdk_pixbuf_get_width (src);
	if (bottomy > gdk_pixbuf_get_height (src))
		bottomy = gdk_pixbuf_get_height (src);
	srcwidth = rightx - srcx;
	srcheight = bottomy - srcy;
	
	rightx = dstx + dstwidth;
	bottomy = dsty + dstheight;
	if (rightx > gdk_pixbuf_get_width (dst))
		rightx = gdk_pixbuf_get_width (dst);
	if (bottomy > gdk_pixbuf_get_height (dst))
		bottomy = gdk_pixbuf_get_height (dst);
	dstwidth = rightx - dstx;
	dstheight = bottomy - dsty;
	
	if (dstwidth < srcwidth)
		srcwidth = dstwidth;
	if (dstheight < srcheight)
		srcheight = dstheight;
	
	if (srcx < 0)
		srcoffsetx = 0 - srcx;
	else
		srcoffsetx = 0;

	if (srcy < 0)
		srcoffsety = 0 - srcy;
	else
		srcoffsety = 0;

	if (dstx < 0)
		dstoffsetx = 0 - dstx;
	else
		dstoffsetx = 0;

	if (dsty < 0)
		dstoffsety = 0 - dsty;
	else
		dstoffsety = 0;
	
	if (dstoffsetx > srcoffsetx)
		srcoffsetx = dstoffsetx;
	if (dstoffsety > srcoffsety)
		srcoffsety = dstoffsety;
	
	srcrowstride = gdk_pixbuf_get_rowstride (src);
	dstrowstride = gdk_pixbuf_get_rowstride (dst);
	
	src_pixels = gdk_pixbuf_get_pixels (src);
	dst_pixels = gdk_pixbuf_get_pixels (dst);
	
	for (y = srcoffsety; y < srcheight; y++)
		for (x = srcoffsetx; x < srcwidth; x++)
			{
				guchar r, g, b, a;

				sx = x + srcx;
				sy = y + srcy;
				dx = x + dstx;
				dy = y + dsty;
				a = src_pixels[4 * sx + sy * srcrowstride + 3];
				if (a)
					{
						r = src_pixels[4 * sx + sy * srcrowstride];
						g = src_pixels[4 * sx + 1 + sy * srcrowstride];
						b = src_pixels[4 * sx + 2 + sy * srcrowstride];
						art_rgba_run_alpha (dst_pixels + 4 * dx +
											dy * dstrowstride, r, g, b, a, 1);
					}
			}
}

void
rsvg_art_affine_image(const GdkPixbuf *img, GdkPixbuf *intermediate, 
					  double * affine, double w, double h)
{
	gdouble tmp_affine[6];
	gdouble inv_affine[6];
	gdouble raw_inv_affine[6];
	gint intstride;
	gint basestride;	
	gint basex, basey;
	gdouble fbasex, fbasey;
	gdouble rawx, rawy;
	guchar * intpix;
	guchar * basepix;
	gint i, j, k, basebpp, ii, jj;
	gboolean has_alpha;
	gdouble pixsum[4];
	gboolean xrunnoff, yrunnoff;
	gint iwidth, iheight;
	gint width, height;

	width = gdk_pixbuf_get_width (img);
	height = gdk_pixbuf_get_height (img);
	iwidth = gdk_pixbuf_get_width (intermediate);
	iheight = gdk_pixbuf_get_height (intermediate);

	has_alpha = gdk_pixbuf_get_has_alpha (img);

	basestride = gdk_pixbuf_get_rowstride (img);
	intstride = gdk_pixbuf_get_rowstride (intermediate);
	basepix = gdk_pixbuf_get_pixels (img);
	intpix = gdk_pixbuf_get_pixels (intermediate);
	basebpp = has_alpha ? 4 : 3;

	_rsvg_affine_invert(raw_inv_affine, affine);

	/*scale to w and h*/
	tmp_affine[0] = (double)w;
	tmp_affine[3] = (double)h;
	tmp_affine[1] = tmp_affine[2] = tmp_affine[4] = tmp_affine[5] = 0;
	_rsvg_affine_multiply(tmp_affine, tmp_affine, affine);

	_rsvg_affine_invert(inv_affine, tmp_affine);


	/*apply the transformation*/
	for (i = 0; i < iwidth; i++)
		for (j = 0; j < iheight; j++)		
			{
				fbasex = (inv_affine[0] * (double)i + inv_affine[2] * (double)j + 
						  inv_affine[4]) * (double)width;
				fbasey = (inv_affine[1] * (double)i + inv_affine[3] * (double)j + 
						  inv_affine[5]) * (double)height;
				basex = floor(fbasex);
				basey = floor(fbasey);
				rawx = raw_inv_affine[0] * i + raw_inv_affine[2] * j + 
					raw_inv_affine[4];
				rawy = raw_inv_affine[1] * i + raw_inv_affine[3] * j + 
					raw_inv_affine[5];
				if (rawx < 0 || rawy < 0 || rawx >= w || 
					rawy >= h || basex < 0 || basey < 0 
					|| basex >= width || basey >= height)
					{					
						for (k = 0; k < 4; k++)
							intpix[i * 4 + j * intstride + k] = 0;
					}
				else
					{
						if (basex < 0 || basex + 1 >= width)
							xrunnoff = TRUE;
						else
							xrunnoff = FALSE;
						if (basey < 0 || basey + 1 >= height)
							yrunnoff = TRUE;
						else
							yrunnoff = FALSE;
						for (k = 0; k < basebpp; k++)
							pixsum[k] = 0;
						for (ii = 0; ii < 2; ii++)
							for (jj = 0; jj < 2; jj++)
								{
									if (basex + ii < 0 || basey + jj< 0 
										|| basex + ii >= width || basey + jj >= height)
										;
									else
										{
											for (k = 0; k < basebpp; k++)
												{
													pixsum[k] += 
														(double)basepix[basebpp * (basex + ii) + (basey + jj) * basestride + k] 
														* (xrunnoff ? 1 : fabs(fbasex - (double)(basex + (1 - ii))))
														* (yrunnoff ? 1 : fabs(fbasey - (double)(basey + (1 - jj))));
												}
										}
								}
						for (k = 0; k < basebpp; k++)
							intpix[i * 4 + j * intstride + k] = pixsum[k];
						if (!has_alpha)
							intpix[i * 4 + j * intstride + 3] = 255;
					}	

			}
}

void
rsvg_art_clip_image(GdkPixbuf *intermediate, ArtSVP *path)
{
	gint intstride;
	gint basestride;	
	guchar * intpix;
	guchar * basepix;
	gint i, j;
	gint width, height;
	GdkPixbuf * base;

	width = gdk_pixbuf_get_width (intermediate);
	height = gdk_pixbuf_get_height (intermediate);

	intstride = gdk_pixbuf_get_rowstride (intermediate);
	intpix = gdk_pixbuf_get_pixels (intermediate);

	base = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 0, 8, 
						   width, height);
	basestride = gdk_pixbuf_get_rowstride (base);
	basepix = gdk_pixbuf_get_pixels (base);
	
	art_rgb_svp_aa(path, 0, 0, width, height, 0xFFFFFF, 0x000000, basepix, basestride, NULL);

	for (i = 0; i < width; i++)
		for (j = 0; j < height; j++)		
			{
				intpix[i * 4 + j * intstride + 3] = intpix[i * 4 + j * intstride + 3] * 
					basepix[i * 3 + j * basestride] / 255;
			}
}

void 
rsvg_art_add_clipping_rect(RsvgDrawingCtx *ctx, double x, double y, double w, double h)
{
	ArtSVP * temppath;
	RsvgArtRender * render = (RsvgArtRender *)ctx->render;
	RsvgArtDiscreteLayer * data = g_slist_nth(render->layers, 0)->data;	
	temppath = rsvg_art_rect_clip_path(x, y, w, h, ctx);
	render->clippath = rsvg_art_clip_path_merge(render->clippath, 
												temppath, TRUE, 'i');
	if (temppath)
		data->clippath_loaded = TRUE;
	else
		data->clippath_loaded = FALSE;	
}
