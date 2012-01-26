/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

#ifndef RSVG_FILTER_H
#define RSVG_FILTER_H

#include "rsvg.h"
#include "rsvg-defs.h"

G_BEGIN_DECLS 

typedef struct  {
    int x0, y0, x1, y1;
} RsvgIRect;

typedef RsvgCoordUnits RsvgFilterUnits;

struct _RsvgFilter {
    RsvgNode super;
    RsvgLength x, y, width, height;
    RsvgFilterUnits filterunits;
    RsvgFilterUnits primitiveunits;
};

G_GNUC_INTERNAL
cairo_surface_t *rsvg_filter_render (RsvgFilter *self,
                                     cairo_surface_t *source,
                                     RsvgDrawingCtx *context, 
                                     RsvgBbox *dimentions, 
                                     char *channelmap);

G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter	    (void);
G_GNUC_INTERNAL
RsvgFilter  *rsvg_filter_parse	    (const RsvgDefs * defs, const char *str);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_blend                (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_convolve_matrix      (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_gaussian_blur        (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_offset               (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_merge                (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_merge_node           (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_colour_matrix        (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_component_transfer   (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_node_component_transfer_function      (char channel);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_erode                (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_composite            (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_flood                (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_displacement_map     (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_turbulence           (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_image                (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_diffuse_lighting	    (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_node_light_source	                    (char type);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_specular_lighting    (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_tile                 (void);

G_END_DECLS

#endif
