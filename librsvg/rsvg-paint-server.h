/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-paint-server.h : RSVG colors

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

#ifndef RSVG_PAINT_SERVER_H
#define RSVG_PAINT_SERVER_H

#include <glib.h>
#include <cairo.h>

G_BEGIN_DECLS 

typedef struct _RsvgPaintServer RsvgPaintServer;

/* Create a new paint server based on a specification string. */
/* Implemented in rust/src/paint_server.rs */
G_GNUC_INTERNAL
RsvgPaintServer	    *rsvg_paint_server_parse    (gboolean *inherit, const char *str);

/* Implemented in rust/src/paint_server.rs */
G_GNUC_INTERNAL
void                 rsvg_paint_server_ref      (RsvgPaintServer * ps);

/* Implemented in rust/src/paint_server.rs */
G_GNUC_INTERNAL
void                 rsvg_paint_server_unref    (RsvgPaintServer * ps);

G_END_DECLS

#endif
