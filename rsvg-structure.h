/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-structure.h: Rsvg's structual elements

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

#ifndef RSVG_STRUCTURE_H
#define RSVG_STRUCTURE_H

#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "rsvg-styles.h"

G_BEGIN_DECLS

typedef struct _RsvgDefsDrawable RsvgDefsDrawable;

void rsvg_start_use (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_symbol (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_sub_svg (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_defs (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_g (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_end_g (RsvgHandle *ctx);
void rsvg_end_sub_svg(RsvgHandle *ctx);

typedef struct _RsvgDefsDrawableGroup RsvgDefsDrawableGroup;
typedef struct _RsvgDefsDrawableUse RsvgDefsDrawableUse;
typedef struct _RsvgDefsDrawableSymbol RsvgDefsDrawableSymbol;
typedef struct _RsvgDefsDrawableSvg RsvgDefsDrawableSvg;

struct _RsvgDefsDrawable {
 	RsvgDefVal super;
	RsvgState  state;
	RsvgDefsDrawable * parent;
	void (*draw) (RsvgDefsDrawable * self, RsvgDrawingCtx *ctx, int dominate);
};


struct _RsvgDefsDrawableGroup {
 	RsvgDefsDrawable super;
 	GPtrArray *children;
};

struct _RsvgDefsDrawableSymbol {
 	RsvgDefsDrawableGroup super;
	gint preserve_aspect_ratio;
	gboolean overflow, has_vbox;
 	double x, y, width, height;
};

struct _RsvgDefsDrawableUse {
 	RsvgDefsDrawable super;
 	GString * href;
	gint x, y, w, h;
};

struct _RsvgDefsDrawableSvg {
 	RsvgDefsDrawableGroup super;
	gint preserve_aspect_ratio;
	gdouble x, y, w, h;
	gdouble vbx, vby, vbw, vbh;
	gboolean overflow, has_vbox;
 	GdkPixbuf *img;
};

RsvgDefsDrawable * rsvg_push_def_group (RsvgHandle *ctx, const char * id, 
					RsvgState);
RsvgDefsDrawable * rsvg_push_part_def_group (RsvgHandle *ctx, const char * id, 
					     RsvgState);
void rsvg_pop_def_group (RsvgHandle *ctx);
void rsvg_defs_drawable_group_pack (RsvgDefsDrawableGroup *self, 
				    RsvgDefsDrawable *child);

void rsvg_defs_drawable_draw (RsvgDefsDrawable * self, RsvgDrawingCtx *ctx, 
			      int dominate);

G_END_DECLS

#endif /* RSVG_STRUCTURE_H */
