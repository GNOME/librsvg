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

#ifndef RSVG_PAINT_SERVER_H
#define RSVG_PAINT_SERVER_H

#include <glib/gtypes.h>
#include <libart_lgpl/art_render_gradient.h>
#include "rsvg-defs.h"

G_BEGIN_DECLS

typedef struct _RsvgGradientStop RsvgGradientStop;
typedef struct _RsvgGradientStops RsvgGradientStops;
typedef struct _RsvgLinearGradient RsvgLinearGradient;
typedef struct _RsvgRadialGradient RsvgRadialGradient;
typedef struct _RsvgPattern RsvgPattern;

typedef struct _RsvgPaintServer RsvgPaintServer;

typedef struct _RsvgPSCtx RsvgPSCtx;

struct _RsvgPSCtx {
	double x0;
	double y0;
	double x1;
	double y1;

	guint32 color;
	double affine[6];
	RsvgDrawingCtx *ctx;
};

struct _RsvgGradientStop {
	double offset;
	gboolean is_current_color;
	guint32 rgba;
};

struct _RsvgGradientStops {
	int n_stop;
	RsvgGradientStop *stop;
};

struct _RsvgLinearGradient {
	RsvgDefVal super;
	gboolean obj_bbox;
	double affine[6]; /* user space to actual at time of gradient def */
	RsvgGradientStops *stops;
	ArtGradientSpread spread;
	double x1, y1;
	double x2, y2;
	guint32 current_color;
	gboolean has_current_color;
};

struct _RsvgRadialGradient {
	RsvgDefVal super;
	gboolean obj_bbox;
	double affine[6]; /* user space to actual at time of gradient def */
	RsvgGradientStops *stops;
	ArtGradientSpread spread;
	double cx, cy;
	double r;
	double fx, fy;
	guint32 current_color;
	gboolean has_current_color;
};

struct _RsvgPattern {
	RsvgDefVal super;
	gboolean obj_cbbox;
	gboolean obj_bbox;
	gboolean vbox;
	double affine[6]; /* user space to actual at time of gradient def */
	double x, y, width, height;
	double vbx, vby, vbh, vbw;
	RsvgDefVal * g;
	RsvgDefVal * gfallback;
	unsigned int preserve_aspect_ratio;
};

/* Create a new paint server based on a specification string. */
RsvgPaintServer *
rsvg_paint_server_parse (gboolean * inherit, const RsvgDefs *defs, const char *str,
						 guint32 current_color);

void
rsvg_render_paint_server (ArtRender *ar, RsvgPaintServer *ps,
						  const RsvgPSCtx *ctx);

void
rsvg_paint_server_ref (RsvgPaintServer *ps);

void
rsvg_paint_server_unref (RsvgPaintServer *ps);

RsvgRadialGradient *
rsvg_clone_radial_gradient (const RsvgRadialGradient *grad, gboolean * shallow_cloned);

RsvgLinearGradient *
rsvg_clone_linear_gradient (const RsvgLinearGradient *grad, gboolean * shallow_cloned);

RsvgPattern *
rsvg_clone_pattern (const RsvgPattern *pattern);

G_END_DECLS

#endif
