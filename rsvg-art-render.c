/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-art-render.c: The libart backend plugin

   Copyright (C) 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>

#include "rsvg-art-composite.h"
#include "rsvg-art-draw.h"
#include "rsvg-art-render.h"

static void
rsvg_art_pixels_destroy (guchar *pixels, gpointer data)
{
	g_free (pixels);
}

static void
rsvg_art_render_free (RsvgRender * self)
{
	RsvgArtRender * me = (RsvgArtRender *)self;

	/* TODO */

	g_free (me);
}

RsvgArtRender * 
rsvg_art_render_new(int new_width, int new_height)
{
	RsvgArtRender * output;
	guint8 *pixels;
	int rowstride;

	rowstride = (new_width * 4 + 3) & ~3;
	if (new_height <= 0 || rowstride > INT_MAX / new_height)
		{
			g_warning (_("rsvg_art_render_new: width too large"));
			return NULL;
		}

	pixels = g_try_malloc (rowstride * new_height);
	if (pixels == NULL)
		{
			g_warning (_("rsvg_art_render_new: dimensions too large"));
			return NULL;
		}
	memset (pixels, 0, rowstride * new_height);

	output = g_new(RsvgArtRender, 1);

	output->super.free                 = rsvg_art_render_free;
	output->super.render_path          = rsvg_art_render_path;
	output->super.render_image         = rsvg_art_render_image;
	output->super.pop_discrete_layer   = rsvg_art_pop_discrete_layer;
	output->super.push_discrete_layer  = rsvg_art_push_discrete_layer;
	output->super.add_clipping_rect    = rsvg_art_add_clipping_rect;

	output->pixbuf = gdk_pixbuf_new_from_data (pixels,
											   GDK_COLORSPACE_RGB,
											   TRUE, 8,
											   new_width, new_height,
											   rowstride,
											   rsvg_art_pixels_destroy,
											   NULL);

	output->bbox.x0 = output->bbox.y0 = output->bbox.x1 = output->bbox.y1 = 0;
	output->layers = NULL;
	output->clippath = NULL;

	return output;
}

static void 
bogus(RsvgDrawingCtx *ctx)
{
}

static void 
image_bogus(RsvgDrawingCtx *ctx, const GdkPixbuf *pb, 
			double x, double y, double w, double h)
{
}

static void 
cr_bogus(RsvgDrawingCtx *ctx, double x, double y, double w, double h)
{
}

static void
rsvg_art_svp_render_free (RsvgRender * self)
{
	RsvgArtSVPRender * me = (RsvgArtSVPRender *)self;

	if (me->outline)
		art_svp_free (me->outline);

	g_free (me);
}

RsvgArtSVPRender * 
rsvg_art_svp_render_new(void)
{
	RsvgArtSVPRender * output;
	output = g_new(RsvgArtSVPRender, 1);

	output->super.free                 = rsvg_art_svp_render_free;
	output->super.render_path          = rsvg_art_svp_render_path;
	output->super.render_image         = image_bogus;
	output->super.pop_discrete_layer   = bogus;
	output->super.push_discrete_layer  = bogus;
	output->super.add_clipping_rect    = cr_bogus;

	output->outline = NULL;
	return output;
}

