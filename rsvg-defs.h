/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

#include <glib/gtypes.h>

G_BEGIN_DECLS

typedef enum {
	/* todo: general question: should this be high level, ie a generic
	   paint server, coupled with a paint server interface; or low level,
	   ie specific definable things? For now, we're going low level,
	   but it's not clear that's the best way to go. */
	RSVG_DEF_LINGRAD,
	RSVG_DEF_RADGRAD,
	RSVG_DEF_PATTERN,
	RSVG_DEF_PATH,
	RSVG_DEF_FILTER
} RsvgDefType;

struct _RsvgDefVal {
	RsvgDefType type;
	void (*free) (RsvgDefVal *self);
};

RsvgDefs *
rsvg_defs_new (void);

RsvgDefVal *
rsvg_defs_lookup (const RsvgDefs *defs, const char *name);

void
rsvg_defs_set (RsvgDefs *defs, const char *name, RsvgDefVal *val);

void
rsvg_defs_free (RsvgDefs *defs);

G_END_DECLS

#endif
