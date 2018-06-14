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

#ifndef RSVG_FILTER_H
#define RSVG_FILTER_H

#include "rsvg-private.h"

G_BEGIN_DECLS 

typedef struct  {
    int x0, y0, x1, y1;
} RsvgIRect;

// typedef RsvgCoordUnits RsvgFilterUnits;
// 
// struct _RsvgFilter {
//     RsvgLength x, y, width, height;
//     RsvgFilterUnits filterunits;
//     RsvgFilterUnits primitiveunits;
// };

/**
 * rsvg_filter_render:
 * @node: a pointer to the filter node to use
 * @source: the a #cairo_surface_t of type %CAIRO_SURFACE_TYPE_IMAGE
 * @context: the context
 *
 * Create a new surface applied the filter. This function will create
 * a context for itself, set up the coordinate systems execute all its
 * little primatives and then clean up its own mess.
 *
 * Returns: (transfer full): a new #cairo_surface_t
 **/
/* Implemented in rust/src/filters/ffi.rs */
G_GNUC_INTERNAL
cairo_surface_t *rsvg_filter_render (RsvgNode *filter_node,
                                     cairo_surface_t *source,
                                     RsvgDrawingCtx *context,
                                     char *channelmap);

G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_blend                (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_convolve_matrix      (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_gaussian_blur        (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_merge                (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_merge_node           (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_color_matrix         (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_component_transfer   (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_node_component_transfer_function      (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_erode                (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_displacement_map     (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_turbulence           (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_diffuse_lighting	    (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_node_light_source	                    (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_specular_lighting    (const char *element_name, RsvgNode *parent, const char *id, const char *klass);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_filter_primitive_tile                 (const char *element_name, RsvgNode *parent, const char *id, const char *klass);

G_END_DECLS

#endif
