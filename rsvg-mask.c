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
    const char *id = NULL, *klazz = NULL, *value;
    RsvgMask *mask;
    mask = (RsvgMask *) self;

    if (rsvg_property_bag_size (atts)) {
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
            mask->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            mask->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            mask->width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            mask->height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &mask->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
    }

    rsvg_parse_style_attrs (ctx, mask->super.state, "mask", klazz, id, atts);
}

RsvgNode *
rsvg_new_mask (void)
{
    RsvgMask *mask;

    mask = g_new (RsvgMask, 1);
    _rsvg_node_init (&mask->super, RSVG_NODE_TYPE_MASK);
    mask->maskunits = objectBoundingBox;
    mask->contentunits = userSpaceOnUse;
    mask->x = _rsvg_css_parse_length ("0");
    mask->y = _rsvg_css_parse_length ("0");
    mask->width = _rsvg_css_parse_length ("1");
    mask->height = _rsvg_css_parse_length ("1");
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

RsvgNode *
rsvg_mask_parse (const RsvgDefs * defs, const char *str)
{
    char *name;

    name = rsvg_get_url_string (str);
    if (name) {
        RsvgNode *val;
        val = rsvg_defs_lookup (defs, name);
        g_free (name);

        if (val && RSVG_NODE_TYPE (val) == RSVG_NODE_TYPE_MASK)
            return val;
    }
    return NULL;
}

RsvgNode *
rsvg_clip_path_parse (const RsvgDefs * defs, const char *str)
{
    char *name;

    name = rsvg_get_url_string (str);
    if (name) {
        RsvgNode *val;
        val = rsvg_defs_lookup (defs, name);
        g_free (name);

        if (val && RSVG_NODE_TYPE (val) == RSVG_NODE_TYPE_CLIP_PATH)
            return val;
    }
    return NULL;
}

static void
rsvg_clip_path_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *id = NULL, *klazz = NULL, *value = NULL;
    RsvgClipPath *clip_path;

    clip_path = (RsvgClipPath *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "clipPathUnits"))) {
            if (!strcmp (value, "objectBoundingBox"))
                clip_path->units = objectBoundingBox;
            else
                clip_path->units = userSpaceOnUse;
        }
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &clip_path->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
    }

    rsvg_parse_style_attrs (ctx, clip_path->super.state, "clipPath", klazz, id, atts);
}

RsvgNode *
rsvg_new_clip_path (void)
{
    RsvgClipPath *clip_path;

    clip_path = g_new (RsvgClipPath, 1);
    _rsvg_node_init (&clip_path->super, RSVG_NODE_TYPE_CLIP_PATH);
    clip_path->units = userSpaceOnUse;
    clip_path->super.set_atts = rsvg_clip_path_set_atts;
    clip_path->super.free = _rsvg_node_free;
    return &clip_path->super;
}
