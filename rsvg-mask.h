/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-mask.h : Provides Masks

   Copyright (C) 2004 Caleb Moore

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

   Author: Caleb Moore <calebmm@tpg.com.au>
*/

#ifndef RSVG_MASK_H
#define RSVG_MASK_H

#include "rsvg.h"
#include "rsvg-defs.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include <libxml/SAX.h>
#include <libart_lgpl/art_svp.h>

G_BEGIN_DECLS

typedef RsvgCoordUnits RsvgMaskUnits;

typedef struct _RsvgMask RsvgMask;

struct _RsvgMask {
	RsvgDefsDrawableGroup super;
	double x, y, width, height; 
	RsvgMaskUnits maskunits;
	RsvgMaskUnits contentunits;
};

void 
rsvg_mask_render (RsvgMask *self, GdkPixbuf *source, GdkPixbuf *output, RsvgHandle *ctx);

void 
rsvg_start_mask (RsvgHandle *ctx, RsvgPropertyBag *atts);

void 
rsvg_end_mask (RsvgHandle *ctx);

RsvgDefsDrawable * 
rsvg_mask_parse (const RsvgDefs * defs, const char *str);

typedef struct _RsvgClipPath RsvgClipPath;

struct _RsvgClipPath {
	RsvgDefsDrawableGroup super;
	RsvgCoordUnits units;
};

ArtSVP * 
rsvg_clip_path_render (RsvgClipPath *s, RsvgHandle *ctx);

void 
rsvg_start_clip_path (RsvgHandle *ctx, RsvgPropertyBag *atts);

void 
rsvg_end_clip_path (RsvgHandle *ctx);

RsvgDefsDrawable * 
rsvg_clip_path_parse (const RsvgDefs * defs, const char *str);

ArtSVP *
rsvg_rect_clip_path(double x, double y, double w, double h, RsvgHandle * ctx);

ArtSVP *
rsvg_clip_path_merge(ArtSVP * first, ArtSVP * second, char operation);

G_END_DECLS

#endif
