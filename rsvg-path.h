/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-path.h: Draw SVG paths

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

#ifndef RSVG_PATH_H
#define RSVG_PATH_H

#include <glib.h>
#include <cairo.h>

G_BEGIN_DECLS 

typedef struct {
    GArray *path_data;
    int     last_move_to_index;
} RsvgPathBuilder;

G_GNUC_INTERNAL
void rsvg_path_builder_init (RsvgPathBuilder *builder,
                             int n_elements);
G_GNUC_INTERNAL
void rsvg_path_builder_move_to (RsvgPathBuilder *builder,
                                double x,
                                double y);
G_GNUC_INTERNAL
void rsvg_path_builder_line_to (RsvgPathBuilder *builder,
                                double x,
                                double y);
G_GNUC_INTERNAL
void rsvg_path_builder_curve_to (RsvgPathBuilder *builder,
                                 double x1,
                                 double y1,
                                 double x2,
                                 double y2,
                                 double x3,
                                 double y3);
G_GNUC_INTERNAL
void rsvg_path_builder_close_path (RsvgPathBuilder *builder);
G_GNUC_INTERNAL
cairo_path_t *rsvg_path_builder_finish (RsvgPathBuilder *builder);
G_GNUC_INTERNAL
cairo_path_t *rsvg_parse_path (const char *path_str);
G_GNUC_INTERNAL
void rsvg_cairo_path_destroy (cairo_path_t *path);

G_END_DECLS

#endif /* RSVG_PATH_H */
