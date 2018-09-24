/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-css.h : CSS utility functions

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
#ifndef RSVG_CSS_H
#define RSVG_CSS_H

#include <glib.h>

#ifdef RSVG_COMPILATION
#include <pango/pango.h>
#include "rsvg-private.h"
#endif

G_BEGIN_DECLS

/* Keep this in sync with rust/src/color.rs:ColorKind */
typedef enum {
    RSVG_CSS_COLOR_SPEC_INHERIT,
    RSVG_CSS_COLOR_SPEC_CURRENT_COLOR,
    RSVG_CSS_COLOR_SPEC_ARGB,
    RSVG_CSS_COLOR_PARSE_ERROR
} RsvgCssColorKind;

/* Keep this in sync with rust/src/color.rs:RsvgCssColor */
typedef struct {
    RsvgCssColorKind kind;
    guint32 argb; /* only valid if kind == RSVG_CSS_COLOR_SPEC_ARGB */
} RsvgCssColorSpec;

/* This one is semi-public for mis-use in rsvg-convert */
RsvgCssColorSpec rsvg_css_parse_color_ (const char *str);

#ifdef RSVG_COMPILATION

/* Implemented in rust/src/color.rs */
G_GNUC_INTERNAL
RsvgCssColorSpec rsvg_css_parse_color (const char *str);

#endif /* RSVG_COMPILATION */

G_END_DECLS

#endif
