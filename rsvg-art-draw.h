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
#ifndef RSVG_ART_DRAW_H
#define RSVG_ART_DRAW_H

#include "rsvg-private.h"

G_BEGIN_DECLS

void rsvg_art_render_path (RsvgDrawingCtx *ctx, const RsvgBpathDef * path);
void rsvg_art_svp_render_path (RsvgDrawingCtx *ctx, const RsvgBpathDef * path);
void rsvg_art_render_image (RsvgDrawingCtx *ctx, const GdkPixbuf * img, 
							double x, double y, double w, double h);

G_END_DECLS

#endif /*RSVG_ART_DRAW_H*/
