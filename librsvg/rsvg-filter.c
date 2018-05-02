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
#include "rsvg-cairo-draw.h"
#include "filters/common.h"

/**
 * rsvg_filter_render:
 * @node: a pointer to the filter node to use
 * @source: the a #cairo_surface_t of type %CAIRO_SURFACE_TYPE_IMAGE
 * @context: the context
 *
 * Create a new surface applied the filter. This function will create
 * a context for itself, set up the coordinate systems execute all its
 * little primatives and then clean up its own mess.
 *
 * Returns: (transfer full): a new #cairo_surface_t
 **/
cairo_surface_t *
rsvg_filter_render (RsvgNode *filter_node,
                    cairo_surface_t *source,
                    RsvgDrawingCtx *context,
                    char *channelmap)
{
    RsvgFilter *filter;
    RsvgFilterContext *ctx;
    RsvgNodeChildrenIter *iter;
    RsvgNode *child;
    guint i;
    cairo_surface_t *output;

    g_return_val_if_fail (source != NULL, NULL);
    g_return_val_if_fail (cairo_surface_get_type (source) == CAIRO_SURFACE_TYPE_IMAGE, NULL);

    g_assert (rsvg_node_get_type (filter_node) == RSVG_NODE_TYPE_FILTER);
    filter = rsvg_rust_cnode_get_impl (filter_node);

    ctx = g_new0 (RsvgFilterContext, 1);
    ctx->filter = filter;
    ctx->source_surface = source;
    ctx->bg_surface = NULL;
    ctx->results = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, rsvg_filter_free_pair);
    ctx->ctx = context;

    rsvg_filter_fix_coordinate_system (ctx, rsvg_drawing_ctx_get_current_state (context), &context->bbox);

    ctx->lastresult.surface = cairo_surface_reference (source);
    ctx->lastresult.bounds = rsvg_filter_primitive_get_bounds (NULL, ctx);

    for (i = 0; i < 4; i++)
        ctx->channelmap[i] = channelmap[i] - '0';

    iter = rsvg_node_children_iter_begin (filter_node);

    while (rsvg_node_children_iter_next (iter, &child)) {
        render_child_if_filter_primitive (child, ctx);
        child = rsvg_node_unref (child);
    }

    rsvg_node_children_iter_end (iter);

    output = ctx->lastresult.surface;

    g_hash_table_destroy (ctx->results);

    rsvg_filter_context_free (ctx);

    return output;
}

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
                                rsvg_filter_draw,
                                rsvg_filter_free);
}
