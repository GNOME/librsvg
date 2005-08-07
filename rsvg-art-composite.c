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
	RsvgIRect underbbox;
	RsvgState * state;
	ArtSVP * clippath_save;
	gboolean clippath_loaded;
	gboolean backgroundnew;
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
			layer->backgroundnew = state->backgroundnew;

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
rsvg_compile_bg(RsvgDrawingCtx *ctx)
{
	int i;
	GdkPixbuf *intermediate, *lastintermediate;
	RsvgArtDiscreteLayer *state;
	RsvgIRect save;
	RsvgArtRender *render = (RsvgArtRender *)ctx->render;

	lastintermediate = gdk_pixbuf_copy(((RsvgArtDiscreteLayer *)render->layers->data)->save_pixbuf);
			
	save = render->bbox;

	render->bbox.x0 = 0;
	render->bbox.y0 = 0;
	render->bbox.x1 = gdk_pixbuf_get_width(render->pixbuf);
	render->bbox.y1 = gdk_pixbuf_get_height(render->pixbuf);

	for (i = 0; (state = g_slist_nth_data(render->layers, i)) != NULL; i++)
		{
			if (state->backgroundnew)
				break;
			if (state->save_pixbuf)
				{
					intermediate = gdk_pixbuf_copy(state->save_pixbuf);
					rsvg_use_opacity(ctx, 0xFF, lastintermediate, intermediate);
					g_object_unref(lastintermediate);
					lastintermediate = intermediate;
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
	RsvgNode * mask = state->mask;
	GdkPixbuf *intermediate;
	GdkPixbuf *in, *out, *insidebg;
	int operationsleft;
	gint adobe_blend = state->adobe_blend;
	RsvgArtRender * render = (RsvgArtRender *)ctx->render;

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
			insidebg = rsvg_compile_bg(ctx);
		}
	else
		insidebg = NULL;

	if (filter != NULL)
		{
			GdkPixbuf * temp;
			out = get_next_out(&operationsleft, in, tos, nos, intermediate);
			temp = rsvg_filter_render (filter, in, insidebg, ctx, 
									   (RsvgIRect *)&render->bbox);
			if (render->clippath)
				rsvg_art_clip_image(temp, render->clippath);
			rsvg_alpha_blt (temp, render->bbox.x0, render->bbox.y0, 
							render->bbox.x1 - render->bbox.x0,
							render->bbox.y1 - render->bbox.y0,
							out, render->bbox.x0, render->bbox.y0);
			g_object_unref (G_OBJECT (temp));
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
			rsvg_filter_adobe_blend (adobe_blend, in, insidebg, out, 
									 render->bbox, ctx);
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
			art_irect_union((ArtIRect*)&render->bbox, 
							(ArtIRect*)&render->bbox, 
							(ArtIRect*)&layer->underbbox);
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

GdkPixbuf * 
rsvg_art_get_image_of_node(RsvgDrawingCtx *ctx, RsvgNode * drawable,
						   double w, double h)
{
	GdkPixbuf *img, *save;


	img = _rsvg_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, w, h);
	
	save = ((RsvgArtRender *)ctx->render)->pixbuf;
	((RsvgArtRender *)ctx->render)->pixbuf = img;


	rsvg_state_push(ctx);
	
	rsvg_node_draw (drawable, ctx, 0);
	
	rsvg_state_pop(ctx);
		
	((RsvgArtRender *)ctx->render)->pixbuf = save;
	
	return img;
}
