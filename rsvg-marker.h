/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 8; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-marker.h: Marker loading and rendering

   Copyright (C) 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#ifndef RSVG_MARKER_H
#define RSVG_MARKER_H

#include "rsvg-structure.h"

G_BEGIN_DECLS 

typedef struct _RsvgMarker RsvgMarker;

struct _RsvgMarker {
    RsvgNode super;
    gboolean bbox;
    RsvgLength refX, refY, width, height;
    double orient;
    gint preserve_aspect_ratio;
    gboolean orientAuto;
    RsvgViewBox vbox;
};

RsvgNode    *rsvg_new_marker	    (void);
void	     rsvg_marker_render	    (RsvgMarker * self, gdouble x, gdouble y, 
				     gdouble orient, gdouble linewidth, RsvgDrawingCtx * ctx);
RsvgNode    *rsvg_marker_parse	    (const RsvgDefs * defs, const char *str);
void	     rsvg_render_markers    (const RsvgBpathDef * bpath_def, RsvgDrawingCtx * ctx);

G_END_DECLS

#endif                          /* RSVG_MARKER_H */
