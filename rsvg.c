/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

#ifdef HAVE_SVGZ
#include <gsf/gsf-input-gzip.h>
#include <gsf/gsf-input-memory.h>
#include <gsf/gsf-output-memory.h>
#include <gsf/gsf-utils.h>
#endif

#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-image.h"
#include "rsvg-text.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"

#include <math.h>
#include <string.h>
#include <stdarg.h>

#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-paint-server.h"

#include "rsvg-art-render.h"
#include "rsvg-art-draw.h"


static RsvgDrawingCtx * 
rsvg_new_drawing_ctx(RsvgHandle * handle)
{
	RsvgDimensionData data;
	RsvgDrawingCtx * draw;
	RsvgState * state;
	double affine[6];

	data = rsvg_get_dimensions(handle);
	if(data.width == 0 || data.height == 0)
		return NULL;

	draw = g_new(RsvgDrawingCtx, 1);

	draw->render = (RsvgRender *) rsvg_art_render_new (data.width, data.height);

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

/**
 * rsvg_handle_get_pixbuf:
 * @handle: An #RsvgHandle
 *
 * Returns the pixbuf loaded by #handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Returns: the pixbuf loaded by #handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle *handle)
{
	GdkPixbuf * output = NULL;
	RsvgDrawingCtx * draw;
	g_return_val_if_fail (handle != NULL, NULL);

	if (!handle->finished)
		return NULL;

	draw = rsvg_new_drawing_ctx(handle);
	if (!draw)
		return NULL;
	rsvg_state_push(draw);
	rsvg_node_draw((RsvgNode *)handle->treebase, draw, 0);
	rsvg_state_pop(draw);
	output = ((RsvgArtRender *)draw->render)->pixbuf;
	rsvg_drawing_ctx_free(draw);
	
	return output;
}

