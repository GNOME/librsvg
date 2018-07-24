/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-styles.h: Handle SVG styles

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

#ifndef RSVG_STYLES_H
#define RSVG_STYLES_H

#include <cairo.h>
#include "rsvg.h"
#include "rsvg-css.h"

#include <libxml/SAX.h>

G_BEGIN_DECLS 

/* Defined in rsvg_internals/src/state.rs */
G_GNUC_INTERNAL
void rsvg_state_free (RsvgState *state);

G_GNUC_INTERNAL
void rsvg_parse_cssbuffer (RsvgHandle *handle, const char *buff, size_t buflen);

/* Defined in rsvg_internals/src/state.rs */
G_GNUC_INTERNAL
void rsvg_parse_style_attrs (RsvgHandle *handle, RsvgNode *node, const char *tag, RsvgPropertyBag * atts);

G_GNUC_INTERNAL
gboolean rsvg_lookup_apply_css_style (RsvgHandle *handle, const char *target, RsvgState * state);

/* Defined in rsvg_internals/src/state.rs */
G_GNUC_INTERNAL
RsvgState *rsvg_state_parent (RsvgState *state);

G_END_DECLS

#endif                          /* RSVG_STYLES_H */
