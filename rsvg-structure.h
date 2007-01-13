/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 8; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

RsvgNode * rsvg_new_use (void);
RsvgNode *rsvg_new_symbol (void);
RsvgNode *rsvg_new_svg (void);
RsvgNode *rsvg_new_defs (void);
RsvgNode *rsvg_new_group (void);
RsvgNode *rsvg_new_switch (void);

typedef struct _RsvgNodeGroup RsvgNodeGroup;
typedef struct _RsvgNodeUse RsvgNodeUse;
typedef struct _RsvgNodeSymbol RsvgNodeSymbol;
typedef struct _RsvgNodeSvg RsvgNodeSvg;

struct _RsvgNodeGroup {
    RsvgNode super;
};

struct _RsvgNodeSymbol {
    RsvgNode super;
    gint preserve_aspect_ratio;
    RsvgViewBox vbox;
};

struct _RsvgNodeUse {
    RsvgNode super;
    RsvgNode *link;
    RsvgLength x, y, w, h;
};

struct _RsvgNodeSvg {
    RsvgNode super;
    gint preserve_aspect_ratio;
    RsvgLength x, y, w, h;
    RsvgViewBox vbox;
    GdkPixbuf *img;
};

void rsvg_pop_def_group		(RsvgHandle * ctx);
void rsvg_node_group_pack	(RsvgNode * self, RsvgNode * child);

void rsvg_node_draw		(RsvgNode * self, RsvgDrawingCtx * ctx, int dominate);
void _rsvg_node_draw_children	(RsvgNode * self, RsvgDrawingCtx * ctx, int dominate);
void _rsvg_node_finalize	(RsvgNode * self);
void _rsvg_node_free		(RsvgNode * self);
void _rsvg_node_init		(RsvgNode * self);

G_END_DECLS

#endif                          /* RSVG_STRUCTURE_H */
