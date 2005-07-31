/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-shapes.c: Draw shapes with libart

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "rsvg-cairo-draw.h"
#include "rsvg-cairo-render.h"
#include "rsvg-styles.h"
#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-filter.h"

#include <math.h>

void
rsvg_cairo_render_path(RsvgDrawingCtx *ctx, const RsvgBpathDef *bpath_def)
{
}

void rsvg_cairo_render_image (RsvgDrawingCtx *ctx, const GdkPixbuf * img, 
							  double x, double y, double w, double h)
{
	RsvgCairoRender *render = (RsvgCairoRender *)ctx->render;
	RsvgState *state = rsvg_state_current(ctx);
	cairo_surface_t * surface;
	unsigned char * data = gdk_pixbuf_get_pixels(img);

    cairo_save (render->cr);

    surface = cairo_image_surface_create_for_data (data, CAIRO_FORMAT_RGB24,
												   w, h, w * 4);
    cairo_translate (render->cr, x, y);

    cairo_set_source_surface (render->cr, surface, 0, 0);
    if (state->opacity != 1.0)
		cairo_paint_with_alpha (render->cr, state->opacity);
    else
		cairo_paint (render->cr);
    
    cairo_surface_destroy (surface);

    cairo_restore (render->cr);
}
