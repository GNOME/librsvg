/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-bpath-util.h: Path utility functions

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

#ifndef RSVG_BPATH_UTIL_H
#define RSVG_BPATH_UTIL_H

#include <glib.h>

G_BEGIN_DECLS 
typedef enum {
    RSVG_MOVETO,
    RSVG_MOVETO_OPEN,
    RSVG_CURVETO,
    RSVG_LINETO,
    RSVG_END
} RsvgPathcode;

typedef struct _RsvgBpath RsvgBpath;
struct _RsvgBpath {
    /*< public > */
    RsvgPathcode code;
    double x1;
    double y1;
    double x2;
    double y2;
    double x3;
    double y3;
};

typedef struct _RsvgBpathDef RsvgBpathDef;

struct _RsvgBpathDef {
    RsvgBpath *bpath;
    int n_bpath;
    int n_bpath_max;
    int moveto_idx;
};

RsvgBpathDef *rsvg_bpath_def_new        (void);
RsvgBpathDef *rsvg_bpath_def_new_from   (RsvgBpath * bpath);

void rsvg_bpath_def_free        (RsvgBpathDef * bpd);

void rsvg_bpath_def_moveto      (RsvgBpathDef * bpd, double x, double y);
void rsvg_bpath_def_lineto      (RsvgBpathDef * bpd, double x, double y);
void rsvg_bpath_def_curveto     (RsvgBpathDef * bpd,
                                 double x1, double y1, double x2, double y2, double x3, double y3);
void rsvg_bpath_def_closepath   (RsvgBpathDef * bpd);

void rsvg_bpath_def_art_finish  (RsvgBpathDef * bpd);

G_END_DECLS

#endif
