/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-shapes.c: Draw shapes with libart

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/
#ifndef RSVG_CAIRO_DRAW_H
#define RSVG_CAIRO_DRAW_H

#include "rsvg-private.h"

G_BEGIN_DECLS 

G_GNUC_INTERNAL
PangoContext    *rsvg_cairo_create_pango_context    (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void         rsvg_cairo_render_pango_layout	    (RsvgDrawingCtx *ctx, PangoLayout *layout, 
                                                 double x, double y);
G_GNUC_INTERNAL
void         rsvg_cairo_render_path             (RsvgDrawingCtx *ctx, 
                                                 const cairo_path_t *path);
G_GNUC_INTERNAL
void         rsvg_cairo_render_surface          (RsvgDrawingCtx *ctx, cairo_surface_t *surface,
                                                 double x, double y, double w, double h);
G_GNUC_INTERNAL
void         rsvg_cairo_push_discrete_layer	    (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void         rsvg_cairo_pop_discrete_layer      (RsvgDrawingCtx *ctx);
G_GNUC_INTERNAL
void         rsvg_cairo_add_clipping_rect       (RsvgDrawingCtx *ctx,
                                                 double x, double y, double width, double height);
G_GNUC_INTERNAL
cairo_surface_t*rsvg_cairo_get_surface_of_node  (RsvgDrawingCtx *ctx, RsvgNode *drawable, 
                                                 double width, double height);

G_END_DECLS

#endif /*RSVG_CAIRO_DRAW_H */
