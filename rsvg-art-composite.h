/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-art-composite.h: Composite different layers using gdk pixbuff for our 
   libart backend

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 - 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003 - 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

#ifndef RSVG_ART_COMPOSITE_H
#define RSVG_ART_COMPOSITE_H

#include "rsvg-private.h"
#include <libart_lgpl/art_svp.h>

G_BEGIN_DECLS

void rsvg_art_pop_discrete_layer(RsvgDrawingCtx *ctx);
void rsvg_art_push_discrete_layer (RsvgDrawingCtx *ctx);
gboolean rsvg_art_needs_discrete_layer(RsvgState *state);

void rsvg_art_clip_image (GdkPixbuf *intermediate, ArtSVP *path);
void rsvg_art_add_clipping_rect(RsvgDrawingCtx *ctx, double x, double y, double w, double h);
void * rsvg_art_get_image_of_node(RsvgDrawingCtx *ctx, RsvgNode * drawable, double w, double h);

G_END_DECLS

#endif
