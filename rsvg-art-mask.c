/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-filter.c: Provides filters
 
   Copyright (C) 2004 Caleb Moore
  
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
  
   Author: Caleb Moore <calebmm@tpg.com.au>
*/

#include "rsvg-private.h"
#include "rsvg-art-mask.h"
#include "rsvg-styles.h"
#include "rsvg-art-draw.h"
#include "rsvg-art-composite.h"
#include "rsvg-art-render.h"
#include "rsvg-css.h"
#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_svp_ops.h>
#include <string.h>

ArtSVP *
rsvg_art_rect_clip_path(double x, double y, double w, double h, RsvgDrawingCtx * ctx)
{	
	RsvgArtSVPRender * asvpr;
	RsvgRender * save;
	GString * d = NULL;
	ArtSVP * output = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];

	/* emulate a rect using a path */
	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + w));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y+h));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

	g_string_append (d, " Z");

	asvpr = rsvg_art_svp_render_new();

	save = ctx->render;
	ctx->render = (RsvgRender *)asvpr;

	rsvg_render_path (ctx, d->str);

	ctx->render = save;

	output = asvpr->outline;

	rsvg_render_free((RsvgRender *)asvpr);

	g_string_free (d, TRUE);
	return output;
}

/*in case anyone is wondering, if the save value is true, it means that we 
  don't want to be deleting the first SVP */
ArtSVP *
rsvg_art_clip_path_merge(ArtSVP * first, ArtSVP * second, int save, char operation)
{
	ArtSVP * tmppath;
	if (first != NULL && second != NULL)
		{
			if (operation == 'i')
				tmppath = art_svp_intersect(first, second);
			else
				tmppath = art_svp_union(first, second);
			art_svp_free(second);
			if (!save)
				art_svp_free(first);
			return tmppath;
		}
	else if (first != NULL)
		return first;
	else
		return second;
}

ArtSVP *
rsvg_art_clip_path_render (RsvgClipPath * self, RsvgDrawingCtx *ctx)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	ArtSVP *svp;
	RsvgArtSVPRender * asvpr;
	RsvgRender * save;	

	rsvg_state_reinherit_top(ctx, &self->super.super.state, 0);

	if (self->units == objectBoundingBox)
		{
			state->affine[0] = ((RsvgArtRender *)ctx->render)->bbox.x1 
				- ((RsvgArtRender *)ctx->render)->bbox.x0;
			state->affine[1] = 0;
			state->affine[2] = 0;
			state->affine[3] = ((RsvgArtRender *)ctx->render)->bbox.y1 - 
				((RsvgArtRender *)ctx->render)->bbox.y0;
			state->affine[4] = ((RsvgArtRender *)ctx->render)->bbox.x0;
			state->affine[5] = ((RsvgArtRender *)ctx->render)->bbox.y0;
		}

	asvpr = rsvg_art_svp_render_new();
	save = ctx->render;
	ctx->render = (RsvgRender *)asvpr;

	for (i = 0; i < group->children->len; i++)
		{
			rsvg_defs_drawable_draw (g_ptr_array_index(group->children, i), 
									 ctx, 0);
		}

	svp = asvpr->outline;
	rsvg_render_free(ctx->render);
	ctx->render = save;

	return svp;
}

void 
rsvg_art_mask_render (RsvgMask *self, GdkPixbuf *tos, GdkPixbuf *nos, RsvgDrawingCtx *ctx)
{
	art_u8 *tos_pixels, *nos_pixels, *mask_pixels;
	int width;
	int height;
	int rowstride;
	int x, y;
	
	GdkPixbuf *save, *mask;
	RsvgDefsDrawable *drawable;	

	drawable = (RsvgDefsDrawable*)self;
	
	mask = _rsvg_pixbuf_new_cleared(GDK_COLORSPACE_RGB, 1, 8, 
									gdk_pixbuf_get_width(tos), 
									gdk_pixbuf_get_height(tos));

	save = ((RsvgArtRender *)ctx->render)->pixbuf;
	((RsvgArtRender *)ctx->render)->pixbuf = mask;

	rsvg_state_push(ctx);
	rsvg_defs_drawable_draw (drawable, ctx, 0);
	rsvg_state_pop(ctx);

	((RsvgArtRender *)ctx->render)->pixbuf = save;

	if (tos == NULL || nos == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	if (!gdk_pixbuf_get_has_alpha (nos))
		{
			g_warning (_("push/pop transparency group on non-alpha buffer nyi"));
			return;
		}
	
	width = gdk_pixbuf_get_width (tos);
	height = gdk_pixbuf_get_height (tos);
	rowstride = gdk_pixbuf_get_rowstride (tos);
	
	tos_pixels = gdk_pixbuf_get_pixels (tos);
	nos_pixels = gdk_pixbuf_get_pixels (nos);
	mask_pixels = gdk_pixbuf_get_pixels (mask);
	
	for (y = 0; y < height; y++)
		{
			for (x = 0; x < width; x++)
				{
					guchar r, g, b, rm, gm, bm, am;
					guint a;
					guint luminance;
					a = tos_pixels[4 * x + 3];
					if (a)
						{
							r = tos_pixels[4 * x];
							g = tos_pixels[4 * x + 1];
							b = tos_pixels[4 * x + 2];

							rm = mask_pixels[4 * x];
							gm = mask_pixels[4 * x + 1];
							bm = mask_pixels[4 * x + 2];
							am = mask_pixels[4 * x + 3];

							luminance = (rm * 2125 + gm * 7154 + bm * 721) / 10000;

							a = a * luminance / 255 * am / 255;

							art_rgba_run_alpha (nos_pixels + 4 * x, r, g, b, a, 1);
						}
				}
			tos_pixels += rowstride;
			nos_pixels += rowstride;
			mask_pixels += rowstride;
		}
	g_object_unref (G_OBJECT (mask));
}

