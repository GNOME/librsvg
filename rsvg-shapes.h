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

typedef struct _RsvgDefsDrawable RsvgDefsDrawable;

void rsvg_handle_path (RsvgHandle *ctx, const char * d, const char * id);
void rsvg_render_path (RsvgHandle *ctx, const char *d);
ArtSVP * rsvg_render_path_as_svp(RsvgHandle *ctx, const char *d);
void rsvg_start_path (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_polygon (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_polyline (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_line (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_rect (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_circle (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_ellipse (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_image (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_use (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_symbol (RsvgHandle *ctx, RsvgPropertyBag *atts);
void rsvg_start_sub_svg (RsvgHandle *ctx, RsvgPropertyBag *atts);

RsvgDefsDrawable * rsvg_push_def_group (RsvgHandle *ctx, const char * id);
RsvgDefsDrawable * rsvg_push_part_def_group (RsvgHandle *ctx, const char * id);
void rsvg_pop_def_group (RsvgHandle *ctx);

typedef struct _RsvgDefsDrawablePath RsvgDefsDrawablePath;
typedef struct _RsvgDefsDrawableGroup RsvgDefsDrawableGroup;
typedef struct _RsvgDefsDrawableUse RsvgDefsDrawableUse;
typedef struct _RsvgDefsDrawableImage RsvgDefsDrawableImage;
typedef struct _RsvgDefsDrawableSymbol RsvgDefsDrawableSymbol;
typedef struct _RsvgDefsDrawableSvg RsvgDefsDrawableSvg;

struct _RsvgDefsDrawable {
 	RsvgDefVal super;
	RsvgState  state;
	RsvgDefsDrawable * parent;
	void (*draw) (RsvgDefsDrawable * self, RsvgHandle *ctx, int dominate);
	ArtSVP * (*draw_as_svp) (RsvgDefsDrawable * self, RsvgHandle *ctx, int dominate);
};

struct _RsvgDefsDrawablePath {
 	RsvgDefsDrawable super;
 	char       *d;
};

struct _RsvgDefsDrawableGroup {
 	RsvgDefsDrawable super;
 	GPtrArray *children;
};

struct _RsvgDefsDrawableSymbol {
 	RsvgDefsDrawableGroup super;
	gint preserve_aspect_ratio;
	int has_vbox;
 	double x, y, width, height;
};

struct _RsvgDefsDrawableUse {
 	RsvgDefsDrawable super;
 	RsvgDefsDrawable *child;
};

struct _RsvgDefsDrawableImage {
 	RsvgDefsDrawable super;
	gint preserve_aspect_ratio, x, y, w, h;
 	GdkPixbuf *img;
};

struct _RsvgDefsDrawableSvg {
 	RsvgDefsDrawableGroup super;
	gint preserve_aspect_ratio;
	gdouble x, y, w, h;
	gdouble vbx, vby, vbw, vbh;
	gboolean overflow, has_vbox;
 	GdkPixbuf *img;
};

typedef struct _RsvgMarker RsvgMarker;

struct _RsvgMarker {
	RsvgDefVal super;
 	RsvgDefsDrawable * contents;
	gboolean bbox;
	double refX, refY, orient;
	double vbx, vby, vbw, vbh, width, height;
	gint preserve_aspect_ratio;
	gboolean vbox;
	gboolean orientAuto;
};

void 
rsvg_start_marker (RsvgHandle *ctx, RsvgPropertyBag *atts);

void 
rsvg_marker_render (RsvgMarker *self, gdouble x, gdouble y, gdouble orient, gdouble linewidth, RsvgHandle *ctx);

RsvgDefVal *
rsvg_marker_parse (const RsvgDefs * defs, const char *str);

GdkPixbuf *
rsvg_pixbuf_new_from_href (const char *href,
						   const char *base_uri,
						   GError    **err);

void rsvg_defs_drawable_draw (RsvgDefsDrawable * self, RsvgHandle *ctx, 
							  int dominate);
ArtSVP * rsvg_defs_drawable_draw_as_svp (RsvgDefsDrawable * self, RsvgHandle *ctx, 
										 int dominate);

void rsvg_preserve_aspect_ratio(unsigned int aspect_ratio, double width, 
								double height, double * w, double * h,
								double * x, double * y);

void
rsvg_affine_image(GdkPixbuf *img, GdkPixbuf *intermediate, 
				  double * affine, double w, double h);

G_END_DECLS

#endif /* RSVG_SHAPES_H */
