/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-styles.c: Handle SVG styles

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
#include "config.h"

#include <errno.h>
#include <string.h>
#include <math.h>

#include "rsvg-attributes.h"
#include "rsvg-private.h"
#include "rsvg-css.h"

#include <libcroco/libcroco.h>

/* This is defined like this so that we can export the Rust function... just for
 * the benefit of rsvg-convert.c
 */
RsvgCssColorSpec
rsvg_css_parse_color_ (const char *str)
{
    return rsvg_css_parse_color (str);
}
