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

#include "rsvg-art-composite.h"
#include "rsvg-art-draw.h"
#include "rsvg-art-render.h"

RsvgArtRender * rsvg_art_render_new(GdkPixbuf * pb)
{
	RsvgArtRender * output;
	output = g_new(RsvgArtRender, 1);

	output->super.render_path          = rsvg_art_render_path;
	output->super.render_image         = rsvg_art_render_image;
	output->super.pop_discrete_layer   = rsvg_art_pop_discrete_layer;
	output->super.push_discrete_layer  = rsvg_art_push_discrete_layer;

	output->pixbuf = pb;
	output->layers = NULL;
	return output;
}

static void 
bogus(RsvgDrawingCtx *ctx)
{
}
static void 
image_bogus(RsvgDrawingCtx *ctx, GdkPixbuf *pb, 
			double x, double y, double w, double h)
{
}


RsvgArtSVPRender * rsvg_art_svp_render_new()
{
	RsvgArtSVPRender * output;
	output = g_new(RsvgArtSVPRender, 1);

	output->super.render_path          = rsvg_art_svp_render_path;
	output->super.render_image         = image_bogus;
	output->super.pop_discrete_layer   = bogus;
	output->super.push_discrete_layer  = bogus;

	output->outline = NULL;
	return output;
}

