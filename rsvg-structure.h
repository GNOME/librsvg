/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

G_GNUC_INTERNAL
RsvgNode *rsvg_new_use (const char *element_name, RsvgNode *parent);
G_GNUC_INTERNAL
RsvgNode *rsvg_new_symbol (const char *element_name, RsvgNode *parent);
G_GNUC_INTERNAL
RsvgNode *rsvg_new_svg (const char *element_name, RsvgNode *parent);

/* Implemented in rust/src/structure.rs */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_group_new (const char *element_name, RsvgNode *parent);

/* Implemented in rust/src/structure.rs */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_defs_new (const char *element_name, RsvgNode *parent);

/* Implemented in rust/src/structure.rs */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_switch_new (const char *element_name, RsvgNode *parent);

typedef struct _RsvgNodeSvg RsvgNodeSvg;

G_GNUC_INTERNAL
void rsvg_node_svg_get_size (RsvgNode *node, RsvgLength *out_width, RsvgLength *out_height);

G_GNUC_INTERNAL
RsvgViewBox rsvg_node_svg_get_view_box (RsvgNode *node);

G_GNUC_INTERNAL
void _rsvg_node_svg_apply_atts  (RsvgNode *node, RsvgNodeSvg *svg, RsvgHandle * ctx);

G_END_DECLS

#endif                          /* RSVG_STRUCTURE_H */
