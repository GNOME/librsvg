/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.c: Provides filters

   Copyright (C) 2004 Caleb Moore

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

   Author: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "rsvg-private.h"
#include "rsvg-filter.h"
#include "rsvg-styles.h"
#include "rsvg-css.h"
#include "rsvg-drawing-ctx.h"
#include "filters/common.h"

/**
 * rsvg_new_filter:
 *
 * Creates a blank filter and assigns default values to everything
 **/
RsvgNode *
rsvg_new_filter (const char *element_name, RsvgNode *parent)
{
    RsvgFilter *filter;

    filter = g_new0 (RsvgFilter, 1);
    filter->filterunits = objectBoundingBox;
    filter->primitiveunits = userSpaceOnUse;
    filter->x = rsvg_length_parse ("-10%", LENGTH_DIR_HORIZONTAL);
    filter->y = rsvg_length_parse ("-10%", LENGTH_DIR_VERTICAL);
    filter->width = rsvg_length_parse ("120%", LENGTH_DIR_HORIZONTAL);
    filter->height = rsvg_length_parse ("120%", LENGTH_DIR_VERTICAL);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER,
                                parent,
                                filter,
                                rsvg_filter_set_atts,
                                rsvg_filter_free);
}
