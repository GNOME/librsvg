/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-cairo-render.c: The cairo backend plugin

   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Dom Lachowicz <cinamod@hotmail.com>
   Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>

#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-render.h"

static void
rsvg_cairo_render_free (RsvgRender * self)
{
	RsvgCairoRender * me = (RsvgCairoRender *)self;

	/* TODO */

	g_free (me);
}

RsvgCairoRender * 
rsvg_cairo_render_new(cairo_t * cr)
{
	RsvgCairoRender * cairo_render = g_new(RsvgCairoRender, 1);

	cairo_render->super.free                 = rsvg_cairo_render_free;
	cairo_render->super.render_image         = rsvg_cairo_render_image;
#if 0
	cairo_render->super.render_path          = rsvg_cairo_render_path;
	cairo_render->super.pop_discrete_layer   = rsvg_cairo_pop_discrete_layer;
	cairo_render->super.push_discrete_layer  = rsvg_cairo_push_discrete_layer;
	cairo_render->super.add_clipping_rect    = rsvg_cairo_add_clipping_rect;
	cairo_render->super.get_image_of_node    = rsvg_cairo_get_image_of_node;
#endif
	cairo_render->cr = cr;

	return cairo_render;
}
