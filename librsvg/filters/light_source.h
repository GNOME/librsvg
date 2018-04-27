/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.h : Provides filters

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

#ifndef RSVG_FILTERS_LIGHT_SOURCE_H
#define RSVG_FILTERS_LIGHT_SOURCE_H

#include "../rsvg-private.h"
#include "../rsvg-filter.h"

G_BEGIN_DECLS 

typedef enum {
    DISTANTLIGHT, POINTLIGHT, SPOTLIGHT
} lightType;

typedef struct _RsvgNodeLightSource RsvgNodeLightSource;

struct _RsvgNodeLightSource {
    lightType type;
    gdouble azimuth;
    gdouble elevation;
    RsvgLength x, y, z, pointsAtX, pointsAtY, pointsAtZ;
    gdouble specularExponent;
    gdouble limitingconeAngle;
};

typedef struct _vector3 vector3;

struct _vector3 {
    gdouble x;
    gdouble y;
    gdouble z;
};

G_GNUC_INTERNAL
gdouble dotproduct (vector3 A, vector3 B);

G_GNUC_INTERNAL
RsvgNodeLightSource *find_light_source_in_children (RsvgNode *node);

G_GNUC_INTERNAL
vector3 get_light_color (RsvgNodeLightSource * source, vector3 color,
                         gdouble x1, gdouble y1, gdouble z, cairo_matrix_t *affine, RsvgDrawingCtx * ctx);

G_GNUC_INTERNAL
vector3 get_light_direction (RsvgNodeLightSource * source, gdouble x1, gdouble y1, gdouble z,
                             cairo_matrix_t *affine, RsvgDrawingCtx * ctx);

G_GNUC_INTERNAL
vector3 get_surface_normal (guchar * I, RsvgIRect boundarys, gint x, gint y,
                            gdouble dx, gdouble dy, gdouble rawdx, gdouble rawdy, gdouble surfaceScale,
                            gint rowstride, int chan);

G_GNUC_INTERNAL
gdouble norm (vector3 A);

G_GNUC_INTERNAL
vector3 normalise (vector3 A);

G_GNUC_INTERNAL
void rsvg_node_light_source_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts);

G_END_DECLS

#endif
