/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-shapes.h: Draw SVG shapes

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

#ifndef RSVG_SHAPES_H
#define RSVG_SHAPES_H

#include "rsvg.h"
#include <libxml/SAX.h>

G_BEGIN_DECLS

void rsvg_handle_path (RsvgHandle *ctx, const char * d, const char * id);
void rsvg_render_path (RsvgHandle *ctx, const char *d);
void rsvg_start_path (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_polygon (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_polyline (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_line (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_rect (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_circle (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_ellipse (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_image (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_use (RsvgHandle *ctx, RsvgPropertyBag *atts);

void rsvg_push_def_group (RsvgHandle *ctx, const char * id);
void rsvg_pop_def_group (RsvgHandle *ctx);

typedef struct _RsvgDefsDrawable RsvgDefsDrawable;
typedef struct _RsvgDefsDrawablePath RsvgDefsDrawablePath;
typedef struct _RsvgDefsDrawableGroup RsvgDefsDrawableGroup;
typedef struct _RsvgDefsDrawableUse RsvgDefsDrawableUse;

struct _RsvgDefsDrawable {
 	RsvgDefVal super;
	RsvgState  state;
	RsvgDefsDrawable * parent;
	void (*draw) (RsvgDefsDrawable * self, RsvgHandle *ctx, int dominate);
};

struct _RsvgDefsDrawablePath {
 	RsvgDefsDrawable super;
 	char       *d;
};

struct _RsvgDefsDrawableGroup {
 	RsvgDefsDrawable super;
 	GPtrArray *children;
};

struct _RsvgDefsDrawableUse {
 	RsvgDefsDrawable super;
 	RsvgDefsDrawable *child;
};

GdkPixbuf *
rsvg_pixbuf_new_from_file_at_size (const char *filename,
								   int         width, 
								   int         height,
								   gboolean    keep_aspect_ratio,
								   GError    **error);
GdkPixbuf *
rsvg_pixbuf_new_from_data_at_size (const char *data,
								   int         width, 
								   int         height,
								   gboolean    keep_aspect_ratio,
								   GError    **error);

void rsvg_defs_drawable_draw (RsvgDefsDrawable * self, RsvgHandle *ctx, 
							  int dominate);

G_END_DECLS

#endif /* RSVG_SHAPES_H */
