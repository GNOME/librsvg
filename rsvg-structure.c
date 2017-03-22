/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-structure.c: Rsvg's structual elements

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 - 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003 - 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Raph Levien <raph@artofcode.com>,
            Dom Lachowicz <cinamod@hotmail.com>,
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "rsvg-structure.h"
#include "rsvg-image.h"
#include "rsvg-css.h"
#include "string.h"

#include <stdio.h>

typedef struct _RsvgNodeUse RsvgNodeUse;
typedef struct _RsvgNodeSymbol RsvgNodeSymbol;

struct _RsvgNodeSymbol {
    guint32 preserve_aspect_ratio;
    RsvgViewBox vbox;
};

struct _RsvgNodeUse {
    char *link;
    RsvgLength x, y, w, h;
};

static gboolean
rsvg_node_is_ancestor (RsvgNode *potential_ancestor, RsvgNode *descendant)
{
    descendant = rsvg_node_ref (descendant);

    while (descendant != NULL) {
        RsvgNode *parent;

        if (rsvg_node_is_same (potential_ancestor, descendant)) {
            descendant = rsvg_node_unref (descendant);
            return TRUE;
        }

        parent = rsvg_node_get_parent (descendant);

        descendant = rsvg_node_unref (descendant);
        descendant = parent;
    }

    return FALSE;
}

static void
rsvg_node_use_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeUse *use = impl;
    RsvgNode *child;
    RsvgState *state;
    cairo_matrix_t affine;
    double x, y, w, h;

    x = rsvg_length_normalize (&use->x, ctx);
    y = rsvg_length_normalize (&use->y, ctx);
    w = rsvg_length_normalize (&use->w, ctx);
    h = rsvg_length_normalize (&use->h, ctx);

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    if (use->link == NULL)
      return;
    child = rsvg_drawing_ctx_acquire_node (ctx, use->link);
    if (!child)
        return;
    else if (rsvg_node_is_ancestor (child, node)) {     /* or, if we're <use>'ing ourself */
        rsvg_drawing_ctx_release_node (ctx, child);
        return;
    }

    state = rsvg_current_state (ctx);
    if (rsvg_node_get_type (child) != RSVG_NODE_TYPE_SYMBOL) {
        cairo_matrix_init_translate (&affine, x, y);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);

        rsvg_push_discrete_layer (ctx);
        rsvg_drawing_ctx_draw_node_from_stack (ctx, child, 1);
        rsvg_pop_discrete_layer (ctx);
    } else {
        RsvgNodeSymbol *symbol = rsvg_rust_cnode_get_impl (child);

        if (symbol->vbox.active) {
            rsvg_aspect_ratio_compute (symbol->preserve_aspect_ratio,
                                       symbol->vbox.rect.width,
                                       symbol->vbox.rect.height,
                                       &x, &y, &w, &h);

            cairo_matrix_init_translate (&affine, x, y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            cairo_matrix_init_scale (&affine, w / symbol->vbox.rect.width, h / symbol->vbox.rect.height);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            cairo_matrix_init_translate (&affine, -symbol->vbox.rect.x, -symbol->vbox.rect.y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            rsvg_drawing_ctx_push_view_box (ctx, symbol->vbox.rect.width, symbol->vbox.rect.height);
            rsvg_push_discrete_layer (ctx);
            if (!state->overflow || (!state->has_overflow && rsvg_node_get_state (child)->overflow))
                rsvg_drawing_ctx_add_clipping_rect (ctx, symbol->vbox.rect.x, symbol->vbox.rect.y,
                                                    symbol->vbox.rect.width, symbol->vbox.rect.height);
        } else {
            cairo_matrix_init_translate (&affine, x, y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);
            rsvg_push_discrete_layer (ctx);
        }

        rsvg_state_push (ctx);
        rsvg_node_draw_children (child, ctx, 1);
        rsvg_state_pop (ctx);
        rsvg_pop_discrete_layer (ctx);
        if (symbol->vbox.active)
            rsvg_drawing_ctx_pop_view_box (ctx);
    }

    rsvg_drawing_ctx_release_node (ctx, child);
}

static void
rsvg_node_use_free (gpointer impl)
{
    RsvgNodeUse *use = impl;

    g_free (use->link);
    g_free (use);
}

static void
rsvg_node_use_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeUse *use = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "x")))
        use->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "y")))
        use->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
    if ((value = rsvg_property_bag_lookup (atts, "width")))
        use->w = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "height")))
        use->h = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);

    if ((value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
        g_free (use->link);
        use->link = g_strdup (value);
    }
}

RsvgNode *
rsvg_new_use (const char *element_name, RsvgNode *parent)
{
    RsvgNodeUse *use;

    use = g_new0 (RsvgNodeUse, 1);
    use->x = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    use->y = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    use->w = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    use->h = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    use->link = NULL;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_USE,
                                parent,
                                rsvg_state_new (),
                                use,
                                rsvg_node_use_set_atts,
                                rsvg_node_use_draw,
                                rsvg_node_use_free);
}

static void
rsvg_node_symbol_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeSymbol *symbol = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
        symbol->vbox = rsvg_css_parse_vbox (value);
    if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
        symbol->preserve_aspect_ratio = rsvg_aspect_ratio_parse (value);
}

static void
rsvg_node_symbol_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing; this gets drawn specially in rsvg_node_use_draw() */
}

static void
rsvg_node_symbol_free (gpointer impl)
{
    RsvgNodeSymbol *symbol = impl;

    g_free (symbol);
}


RsvgNode *
rsvg_new_symbol (const char *element_name, RsvgNode *parent)
{
    RsvgNodeSymbol *symbol;

    symbol = g_new0 (RsvgNodeSymbol, 1);
    symbol->vbox.active = FALSE;
    symbol->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_SYMBOL,
                                parent,
                                rsvg_state_new (),
                                symbol,
                                rsvg_node_symbol_set_atts,
                                rsvg_node_symbol_draw,
                                rsvg_node_symbol_free);
}
