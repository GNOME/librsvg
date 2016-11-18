/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
  
   Author: Caleb Moore <calebmm@tpg.com.au>
*/

#include "rsvg-private.h"
#include "rsvg-mask.h"
#include "rsvg-styles.h"
#include "rsvg-css.h"
#include <string.h>

static void
rsvg_mask_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgMask *mask;
    const char *value;

    mask = (RsvgMask *) self;

    if ((value = rsvg_property_bag_lookup (atts, "maskUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            mask->maskunits = userSpaceOnUse;
        else
            mask->maskunits = objectBoundingBox;
    }
    if ((value = rsvg_property_bag_lookup (atts, "maskContentUnits"))) {
        if (!strcmp (value, "objectBoundingBox"))
            mask->contentunits = objectBoundingBox;
        else
            mask->contentunits = userSpaceOnUse;
    }
    if ((value = rsvg_property_bag_lookup (atts, "x")))
        mask->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        mask->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "width")))
        mask->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "height")))
        mask->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
}

RsvgNode *
rsvg_new_mask (const char *element_name)
{
    RsvgMask *mask;

    mask = g_new (RsvgMask, 1);
    _rsvg_node_init (&mask->super, RSVG_NODE_TYPE_MASK);
    mask->maskunits = objectBoundingBox;
    mask->contentunits = userSpaceOnUse;
    mask->x = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    mask->y = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    mask->width = rsvg_length_parse ("1", LENGTH_DIR_HORIZONTAL);
    mask->height = rsvg_length_parse ("1", LENGTH_DIR_VERTICAL);
    mask->super.set_atts = rsvg_mask_set_atts;
    return &mask->super;
}

char *
rsvg_get_url_string (const char *str)
{
    if (!strncmp (str, "url(", 4)) {
        const char *p = str + 4;
        int ix;

        while (g_ascii_isspace (*p))
            p++;

        for (ix = 0; p[ix]; ix++)
            if (p[ix] == ')')
                return g_strndup (p, ix);
    }
    return NULL;
}

static void
rsvg_clip_path_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgClipPath *clip_path;
    const char *value;

    clip_path = (RsvgClipPath *) self;

    if ((value = rsvg_property_bag_lookup (atts, "clipPathUnits"))) {
        if (!strcmp (value, "objectBoundingBox"))
            clip_path->units = objectBoundingBox;
        else
            clip_path->units = userSpaceOnUse;
    }
}

RsvgNode *
rsvg_new_clip_path (const char *element_name)
{
    RsvgClipPath *clip_path;

    clip_path = g_new (RsvgClipPath, 1);
    _rsvg_node_init (&clip_path->super, RSVG_NODE_TYPE_CLIP_PATH);
    clip_path->units = userSpaceOnUse;
    clip_path->super.set_atts = rsvg_clip_path_set_atts;
    clip_path->super.free = _rsvg_node_free;
    return &clip_path->super;
}
