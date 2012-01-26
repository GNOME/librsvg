/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-defs.h : SVG defs utilities

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

#ifndef RSVG_DEFS_H
#define RSVG_DEFS_H

/* A module for handling SVG defs */

#include <glib.h>

#include "rsvg.h"

G_BEGIN_DECLS 

G_GNUC_INTERNAL
RsvgDefs    *rsvg_defs_new		(RsvgHandle *handle);
/* for some reason this one's public... */
RsvgNode    *rsvg_defs_lookup		(const RsvgDefs * defs, const char *name);
G_GNUC_INTERNAL
void	     rsvg_defs_set		(RsvgDefs * defs, const char *name, RsvgNode * val);
G_GNUC_INTERNAL
void	     rsvg_defs_free		(RsvgDefs * defs);
G_GNUC_INTERNAL
void	     rsvg_defs_add_resolver	(RsvgDefs * defs, RsvgNode ** tochange, const gchar * name);
G_GNUC_INTERNAL
void	     rsvg_defs_resolve_all	(RsvgDefs * defs);
G_GNUC_INTERNAL
void	     rsvg_defs_register_name	(RsvgDefs * defs, const char *name, RsvgNode * val);
G_GNUC_INTERNAL
void	     rsvg_defs_register_memory  (RsvgDefs * defs, RsvgNode * val);

G_END_DECLS
#endif
