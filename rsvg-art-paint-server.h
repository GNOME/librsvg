/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-paint-server.h : RSVG colors

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

#ifndef RSVG_ART_PAINT_SERVER_H
#define RSVG_ART_PAINT_SERVER_H

#include <libart_lgpl/art_render_gradient.h>
#include "rsvg-paint-server.h"

G_BEGIN_DECLS


struct _RsvgPSCtx {
	double x0;
	double y0;
	double x1;
	double y1;

	guint32 color;
	double affine[6];
	RsvgDrawingCtx *ctx;
};

void
rsvg_art_render_paint_server (ArtRender *ar, RsvgPaintServer *ps,
							  const RsvgPSCtx *ctx);

G_END_DECLS

#endif
