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

#include "../rsvg-private.h"
#include "../rsvg-styles.h"
#include "../rsvg-css.h"
#include "../rsvg-cairo-draw.h"
#include "common.h"

typedef struct _RsvgFilterPrimitiveMerge RsvgFilterPrimitiveMerge;

struct _RsvgFilterPrimitiveMerge {
    RsvgFilterPrimitive super;
};

static void
merge_render_child(RsvgNode          *node,
                   cairo_surface_t   *output,
                   RsvgIRect          boundarys,
                   RsvgFilterContext *ctx)
{
    RsvgFilterPrimitive *fp;
    cairo_surface_t *in;

    if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE)
        return;

    fp = rsvg_rust_cnode_get_impl (node);

    in = rsvg_filter_get_in (fp->in, ctx);
    if (in == NULL)
        return;

    rsvg_alpha_blt (in,
                    boundarys.x0,
                    boundarys.y0,
                    boundarys.x1 - boundarys.x0,
                    boundarys.y1 - boundarys.y0,
                    output,
                    boundarys.x0,
                    boundarys.y0);

    cairo_surface_destroy (in);
}

static void
rsvg_filter_primitive_merge_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgNodeChildrenIter *iter;
    RsvgNode *child;
    RsvgIRect boundarys;
    cairo_surface_t *output;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    output = _rsvg_image_surface_new (ctx->width, ctx->height);
    if (output == NULL) {
        return;
    }

    iter = rsvg_node_children_iter_begin (node);

    while (rsvg_node_children_iter_next (iter, &child)) {
        merge_render_child (child, output, boundarys, ctx);
        child = rsvg_node_unref (child);
    }

    rsvg_node_children_iter_end (iter);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_merge_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveMerge *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_merge (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveMerge *filter;

    filter = g_new0 (RsvgFilterPrimitiveMerge, 1);
    filter->super.result = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_merge_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_merge_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

static void
rsvg_filter_primitive_merge_node_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitive *primitive = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_IN:
            /* see bug 145149 - sodipodi generates bad SVG... */
            g_string_assign (primitive->in, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

static void
rsvg_filter_primitive_merge_node_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    /* todo */
}

RsvgNode *
rsvg_new_filter_primitive_merge_node (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitive *filter;

    filter = g_new0 (RsvgFilterPrimitive, 1);
    filter->in = g_string_new ("none");
    filter->render = rsvg_filter_primitive_merge_node_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_merge_node_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}
