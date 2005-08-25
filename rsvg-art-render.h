/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-art-render.h: The libart backend plugin

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

#ifndef RSVG_ART_RENDER_H
#define RSVG_ART_RENDER_H

#include "rsvg-private.h"
#include <libart_lgpl/art_rect.h>
#include <libart_lgpl/art_svp.h>

G_BEGIN_DECLS

typedef struct RsvgArtRender RsvgArtRender;
typedef struct RsvgArtSVPRender RsvgArtSVPRender;

struct RsvgArtRender {
	RsvgRender super;
	GdkPixbuf *pixbuf;
	GSList * layers;
	ArtIRect bbox;
	ArtSVP * clippath;
};

struct RsvgArtSVPRender {
	RsvgRender super;
	ArtSVP *outline;
};

RsvgArtSVPRender * rsvg_art_svp_render_new(void);
RsvgArtRender * rsvg_art_render_new(int new_width, int new_height);

G_END_DECLS

#endif
