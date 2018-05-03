/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-cairo-render.h: The cairo backend plugin

   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Dom Lachowicz <cinamod@hotmail.com>
   Caleb Moore <c.moore@student.unsw.edu.au>
*/

#ifndef RSVG_CAIRO_RENDER_H
#define RSVG_CAIRO_RENDER_H

#include "rsvg-private.h"
#include <cairo.h>

#ifdef HAVE_PANGOFT2
#include <pango/pangofc-fontmap.h>
#endif

G_BEGIN_DECLS

struct _RsvgCairoRender {
    cairo_t *cr;
    double width;
    double height;

    cairo_t *initial_cr;
    double offset_x;
    double offset_y;

    GList *cr_stack;

    /* Stack for bounding boxes with path extents */
    GList *bb_stack;

    /* Stack for bounding boxes with ink extents */
    GList *ink_bb_stack;

    GList *surfaces_stack;

#ifdef HAVE_PANGOFT2
    FcConfig *font_config_for_testing;
    PangoFontMap *font_map_for_testing;
#endif
};

G_GNUC_INTERNAL
RsvgCairoRender *rsvg_cairo_render_new (cairo_t *cr, double width, double height);

G_GNUC_INTERNAL
void rsvg_cairo_render_free (RsvgCairoRender *render);

G_END_DECLS

#endif
