/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-text.h: Text handling routines for RSVG

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

#ifndef RSVG_TEXT_H
#define RSVG_TEXT_H

#include "rsvg.h"
#include "rsvg-shapes.h"

G_BEGIN_DECLS 

G_GNUC_INTERNAL
RsvgNode    *rsvg_new_text	    (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_tspan	    (void);
G_GNUC_INTERNAL
RsvgNode    *rsvg_new_tref	    (void);
G_GNUC_INTERNAL
char	    *rsvg_make_valid_utf8   (const char *str, int len);

G_END_DECLS

#endif                          /* RSVG_TEXT_H */
