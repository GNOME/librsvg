/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 8; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
#include <libxml/SAX.h>

G_BEGIN_DECLS 

typedef RsvgCoordUnits RsvgFilterUnits;

struct _RsvgFilter {
    RsvgNode super;
    int refcnt;
    RsvgLength x, y, width, height;
    RsvgFilterUnits filterunits;
    RsvgFilterUnits primitiveunits;
};

GdkPixbuf   *rsvg_filter_render	    (RsvgFilter * self, GdkPixbuf * source, GdkPixbuf * bg,
				     RsvgDrawingCtx * context, RsvgBbox * dimentions, char *channelmap);

RsvgNode    *rsvg_new_filter	    (void);
RsvgFilter  *rsvg_filter_parse	    (const RsvgDefs * defs, const char *str);

RsvgNode    *rsvg_new_filter_primitive_blend		    (void);
RsvgNode    *rsvg_new_filter_primitive_convolve_matrix	    (void);
RsvgNode    *rsvg_new_filter_primitive_gaussian_blur	    (void);
RsvgNode    *rsvg_new_filter_primitive_offset		    (void);
RsvgNode    *rsvg_new_filter_primitive_merge		    (void);
RsvgNode    *rsvg_new_filter_primitive_merge_node	    (void);
RsvgNode    *rsvg_new_filter_primitive_colour_matrix	    (void);
RsvgNode    *rsvg_new_filter_primitive_component_transfer   (void);
RsvgNode    *rsvg_new_node_component_transfer_function	    (char channel);
RsvgNode    *rsvg_new_filter_primitive_erode		    (void);
RsvgNode    *rsvg_new_filter_primitive_composite	    (void);
RsvgNode    *rsvg_new_filter_primitive_flood		    (void);
RsvgNode    *rsvg_new_filter_primitive_displacement_map	    (void);
RsvgNode    *rsvg_new_filter_primitive_turbulence	    (void);
RsvgNode    *rsvg_new_filter_primitive_image		    (void);
RsvgNode    *rsvg_new_filter_primitive_diffuse_lighting	    (void);
RsvgNode    *rsvg_new_filter_primitive_light_source	    (char type);
RsvgNode    *rsvg_new_filter_primitive_specular_lighting    (void);
RsvgNode    *rsvg_new_filter_primitive_tile		    (void);

void	     rsvg_filter_adobe_blend	(gint modenum, GdkPixbuf * in, GdkPixbuf * bg,
					 GdkPixbuf * output, RsvgIRect boundarys, 
					 RsvgDrawingCtx * ctx);
void	     rsvg_alpha_blt		(GdkPixbuf * src, gint srcx, gint srcy,
					 gint srcwidth, gint srcheight, 
					 GdkPixbuf * dst, gint dstx, gint dsty);
void	     rsvg_art_affine_image	(const GdkPixbuf * img, GdkPixbuf * intermediate,
					 double *affine, double w, double h);

G_END_DECLS

#endif
