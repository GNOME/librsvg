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

#include "rsvg-cairo.h"
#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-render.h"
#include "rsvg-styles.h"
#include "rsvg-structure.h"

static void
rsvg_cairo_render_free (RsvgRender * self)
{
	RsvgCairoRender * me = (RsvgCairoRender *)self;

	/* TODO */

	g_free (me);
}

RsvgCairoRender * 
rsvg_cairo_render_new(cairo_t * cr, double width, double height)
{
	RsvgCairoRender * cairo_render = g_new0(RsvgCairoRender, 1);

	cairo_render->super.free                 = rsvg_cairo_render_free;
	cairo_render->super.render_image         = rsvg_cairo_render_image;
	cairo_render->super.render_path          = rsvg_cairo_render_path;
	cairo_render->super.pop_discrete_layer   = rsvg_cairo_pop_discrete_layer;
	cairo_render->super.push_discrete_layer  = rsvg_cairo_push_discrete_layer;
	cairo_render->super.add_clipping_rect    = rsvg_cairo_add_clipping_rect;
	cairo_render->super.get_image_of_node    = rsvg_cairo_get_image_of_node;
	cairo_render->width = width;
	cairo_render->height = height;
	cairo_render->cr = cr;

	return cairo_render;
}

static RsvgDrawingCtx * 
rsvg_cairo_new_drawing_ctx (cairo_t *cr, RsvgHandle *handle)
{
	RsvgDimensionData data;
	RsvgDrawingCtx * draw;
	RsvgState * state;
	double affine[6];

	rsvg_handle_get_dimensions(handle, &data);
	if(data.width == 0 || data.height == 0)
		return NULL;

	draw = g_new(RsvgDrawingCtx, 1);

	draw->render = (RsvgRender *) rsvg_cairo_render_new (cr, data.width, data.height);

	if(!draw->render)
		return NULL;	

	draw->state = NULL;

	/* should this be G_ALLOC_ONLY? */
	draw->state_allocator = g_mem_chunk_create (RsvgState, 256, G_ALLOC_AND_FREE);

	draw->defs = handle->defs;
	draw->base_uri = g_strdup(handle->base_uri);
	draw->dpi_x = handle->dpi_x;
	draw->dpi_y = handle->dpi_y;
	draw->pango_context = NULL;

	rsvg_state_push(draw);

	state = rsvg_state_current(draw);
	affine[0] = data.width / data.em;
	affine[1] = 0;
	affine[2] = 0;
	affine[3] = data.height / data.ex;
	affine[4] = 0;
	affine[5] = 0;

	_rsvg_affine_multiply(state->affine, affine, 
						  state->affine);
	
	return draw;
}

void
rsvg_cairo_render (cairo_t *cr, RsvgHandle *handle)
{
	RsvgDrawingCtx * draw;
	g_return_if_fail (handle != NULL);

	if (!handle->finished)
		return;

	draw = rsvg_cairo_new_drawing_ctx (cr, handle);
	if (!draw)
		return;

	rsvg_state_push(draw);
	rsvg_node_draw((RsvgNode *)handle->treebase, draw, 0);
	rsvg_state_pop(draw);
	rsvg_drawing_ctx_free(draw);
}
