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

#include "rsvg-art-composite.h"
#include "rsvg-art-render.h"
#include "rsvg-styles.h"
#include "rsvg-structure.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"

#include <libart_lgpl/art_rgba.h>

static void
rsvg_pixmap_destroy (gchar *pixels, gpointer data)
{
  g_free (pixels);
}

void
rsvg_art_push_discrete_layer (RsvgDrawingCtx *ctx)
{
	RsvgState *state;
	GdkPixbuf *pixbuf;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;
	art_u8 *pixels;
	int width, height, rowstride;

	state = rsvg_state_current(ctx);
	pixbuf = render->pixbuf;

	rsvg_state_clip_path_assure(ctx);

	if (state->filter == NULL && state->opacity == 0xFF && 
		!state->backgroundnew && state->mask == NULL && !state->adobe_blend)
		return;
	
	state->save_pixbuf = pixbuf;
	state->underbbox = ctx->bbox;	
	ctx->bbox.x0 = 0;
	ctx->bbox.x1 = 0;
	ctx->bbox.y0 = 0;
	ctx->bbox.y1 = 0;

	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}

	if (!gdk_pixbuf_get_has_alpha (pixbuf))
    {
		g_warning (_("push/pop transparency group on non-alpha buffer nyi\n"));
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

	tos_pixels += rowstride * MAX(ctx->bbox.y0, 0);
	nos_pixels += rowstride * MAX(ctx->bbox.y0, 0);
	
	for (y = MAX(ctx->bbox.y0, 0); y < MIN(ctx->bbox.y1 + 1, height); y++)
		{
			for (x = MAX(ctx->bbox.x0, 0); x < MIN(ctx->bbox.x1 + 1, width); x++)
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
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;
	int i, foundstate;
	GdkPixbuf *intermediate, *lastintermediate;
	RsvgState *state, *lastvalid;
	ArtIRect save;

	lastvalid = NULL;

	foundstate = 0;	

	lastintermediate = gdk_pixbuf_copy(topstate->save_pixbuf);
			
	save = ctx->bbox;

	ctx->bbox.x0 = 0;
	ctx->bbox.y0 = 0;
	ctx->bbox.x1 = gdk_pixbuf_get_width(render->pixbuf);
	ctx->bbox.y1 = gdk_pixbuf_get_height(render->pixbuf);

	for (i = 0; (state = g_slist_nth_data(ctx->state, i)) != NULL; i++)
		{
			if (state == topstate)
				{
					foundstate = 1;
				}
			else if (!foundstate)
				continue;
			if (state->backgroundnew)
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

	ctx->bbox = save;
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
			rsvg_mask_render ((RsvgMask *)mask, in, out, ctx);
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

	state = rsvg_state_current(ctx);

	if (state->filter == NULL && state->opacity == 0xFF && 
		!state->backgroundnew && state->mask == NULL && !state->adobe_blend)
		return;

	tos = render->pixbuf;
	nos = state->save_pixbuf;
	
	if (nos != NULL)
		rsvg_composite_layer(ctx, state, tos, nos);
	
	g_object_unref (tos);
	render->pixbuf = nos;
	art_irect_union(&ctx->bbox, &ctx->bbox, &state->underbbox);
}

gboolean
rsvg_art_needs_discrete_layer(RsvgState *state)
{
	return state->filter || state->mask || state->adobe_blend || state->backgroundnew;
}

void
rsvg_alpha_blt (GdkPixbuf * src, gint srcx, gint srcy, gint srcwidth,
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
