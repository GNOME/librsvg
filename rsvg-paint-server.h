/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

#include <glib.h>
#include <cairo.h>
#include "rsvg-defs.h"

G_BEGIN_DECLS 

typedef struct _RsvgGradientStop RsvgGradientStop;
typedef struct _RsvgGradientStops RsvgGradientStops;
typedef struct _RsvgLinearGradient RsvgLinearGradient;
typedef struct _RsvgRadialGradient RsvgRadialGradient;
typedef struct _RsvgPattern RsvgPattern;
typedef struct _RsvgSolidColor RsvgSolidColor;

typedef struct _RsvgPaintServer RsvgPaintServer;

typedef struct _RsvgPSCtx RsvgPSCtx;

struct _RsvgGradientStop {
    RsvgNode super;
    double offset;
    guint32 rgba;
    gboolean is_valid;
};

struct _RsvgLinearGradient {
    RsvgNode super;
    gboolean obj_bbox;
    cairo_matrix_t affine; /* user space to actual at time of gradient def */
    cairo_extend_t spread;
    RsvgLength x1, y1, x2, y2;
    int hasx1:1;
    int hasy1:1;
    int hasx2:1;
    int hasy2:1;
    int hasbbox:1;
    int hastransform:1;
    int hasspread:1;
    char *fallback;
};

struct _RsvgRadialGradient {
    RsvgNode super;
    gboolean obj_bbox;
    cairo_matrix_t affine; /* user space to actual at time of gradient def */
    cairo_extend_t spread;
    RsvgLength cx, cy, r, fx, fy;
    int hascx:1;
    int hascy:1;
    int hasr:1;
    int hasfx:1;
    int hasfy:1;
    int hasbbox:1;
    int hastransform:1;
    int hasspread:1;
    char *fallback;
};

typedef struct _Gradient Gradient;

/* Implemented in rust/src/gradient.rs */
G_GNUC_INTERNAL
Gradient *gradient_linear_new (RsvgLength     *x1,
                               RsvgLength     *y1,
                               RsvgLength     *x2,
                               RsvgLength     *y2,
                               gboolean       *obj_bbox,
                               cairo_matrix_t *affine,
                               cairo_extend_t *extend,
                               const char     *fallback_name);

/* Implemented in rust/src/gradient.rs */
G_GNUC_INTERNAL
void gradient_destroy (Gradient *gradient);

G_GNUC_INTERNAL
void gradient_add_color_stop (Gradient *gradient,
                              double    offset,
                              guint32   rgba);

struct _RsvgPattern {
    RsvgNode super;
    gboolean obj_cbbox;
    gboolean obj_bbox;
    cairo_matrix_t affine; /* user space to actual at time of gradient def */
    RsvgLength x, y, width, height;
    RsvgViewBox vbox;
    unsigned int preserve_aspect_ratio;
    int hasx:1;
    int hasy:1;
    int hasvbox:1;
    int haswidth:1;
    int hasheight:1;
    int hasaspect:1;
    int hascbox:1;
    int hasbbox:1;
    int hastransform:1;
    char *fallback;
};

struct _RsvgSolidColor {
    gboolean currentcolor;
    guint32 argb;
};

typedef struct _RsvgSolidColor RsvgPaintServerColor;
typedef enum _RsvgPaintServerType RsvgPaintServerType;
typedef union _RsvgPaintServerCore RsvgPaintServerCore;

union _RsvgPaintServerCore {
    RsvgSolidColor *color;
    char *iri;
};

enum _RsvgPaintServerType {
    RSVG_PAINT_SERVER_SOLID,
    RSVG_PAINT_SERVER_IRI
};

struct _RsvgPaintServer {
    int refcnt;
    RsvgPaintServerType type;
    RsvgPaintServerCore core;
};

/* Create a new paint server based on a specification string. */
G_GNUC_INTERNAL
RsvgPaintServer	    *rsvg_paint_server_parse    (gboolean * inherit, const char *str);
G_GNUC_INTERNAL
void                 rsvg_paint_server_ref      (RsvgPaintServer * ps);
G_GNUC_INTERNAL
void                 rsvg_paint_server_unref    (RsvgPaintServer * ps);

G_GNUC_INTERNAL
RsvgNode *rsvg_new_linear_gradient  (const char *element_name);
G_GNUC_INTERNAL
RsvgNode *rsvg_new_radial_gradient  (const char *element_name);
G_GNUC_INTERNAL
RsvgNode *rsvg_new_stop	        (const char *element_name);
G_GNUC_INTERNAL
RsvgNode *rsvg_new_pattern      (const char *element_name);
G_GNUC_INTERNAL
void rsvg_pattern_fix_fallback          (RsvgDrawingCtx * ctx,
                                         RsvgPattern * pattern);
G_GNUC_INTERNAL
void rsvg_linear_gradient_fix_fallback	(RsvgDrawingCtx * ctx,
                                         RsvgLinearGradient * grad);
G_GNUC_INTERNAL
void rsvg_radial_gradient_fix_fallback	(RsvgDrawingCtx * ctx,
                                         RsvgRadialGradient * grad);

G_END_DECLS

#endif
