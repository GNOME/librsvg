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

void
rsvg_node_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgState *state;
    GSList *stacksave;

    if (rsvg_drawing_ctx_limits_exceeded (ctx))
        return;

    state = self->state;

    stacksave = ctx->drawsub_stack;
    if (stacksave) {
        if (stacksave->data != self)
            return;
        ctx->drawsub_stack = stacksave->next;
    }
    if (!state->visible)
        return;

    self->draw (self, ctx, dominate);
    ctx->drawsub_stack = stacksave;
}

/* generic function for drawing all of the children of a particular node */
void
_rsvg_node_draw_children (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    guint i;
    if (dominate != -1) {
        rsvg_state_reinherit_top (ctx, self->state, dominate);

        rsvg_push_discrete_layer (ctx);
    }
    for (i = 0; i < self->children->len; i++) {
        rsvg_state_push (ctx);
        rsvg_node_draw (g_ptr_array_index (self->children, i), ctx, 0);
        rsvg_state_pop (ctx);
    }
    if (dominate != -1)
        rsvg_pop_discrete_layer (ctx);
}

/* generic function that doesn't draw anything at all */
static void
_rsvg_node_draw_nothing (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
}

static void
_rsvg_node_dont_set_atts (RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
}

void
_rsvg_node_init (RsvgNode * self,
                 RsvgNodeType type)
{
    self->type = type;
    self->parent = NULL;
    self->children = g_ptr_array_new ();
    self->state = g_new (RsvgState, 1);
    rsvg_state_init (self->state);
    self->typename = NULL;
    self->atts = NULL;
    self->free = _rsvg_node_free;
    self->draw = _rsvg_node_draw_nothing;
    self->set_atts = _rsvg_node_dont_set_atts;
}

void
_rsvg_node_finalize (RsvgNode * self)
{
    if (self->atts)
        rsvg_property_bag_free (self->atts);
    if (self->state != NULL) {
        rsvg_state_finalize (self->state);
        g_free (self->state);
    }
    if (self->children != NULL)
        g_ptr_array_free (self->children, TRUE);
}

void
_rsvg_node_free (RsvgNode * self)
{
    _rsvg_node_finalize (self);
    g_free (self);
}

static void
rsvg_node_group_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, self);
    }
}

RsvgNode *
rsvg_new_group (void)
{
    RsvgNodeGroup *group;
    group = g_new (RsvgNodeGroup, 1);
    _rsvg_node_init (&group->super, RSVG_NODE_TYPE_GROUP);
    group->super.typename = "g";
    group->super.draw = _rsvg_node_draw_children;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}

void
rsvg_pop_def_group (RsvgHandle * ctx)
{
    g_assert (ctx->priv->currentnode != NULL);
    ctx->priv->currentnode = ctx->priv->currentnode->parent;
}

void
rsvg_node_group_pack (RsvgNode * self, RsvgNode * child)
{
    g_ptr_array_add (self->children, child);
    child->parent = self;
}

static void
rsvg_node_use_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeUse *use = (RsvgNodeUse *) self;
    RsvgNode *self_acquired = NULL;
    RsvgNode *child;
    RsvgState *state;
    cairo_matrix_t affine;
    double x, y, w, h;
    x = _rsvg_css_normalize_length (&use->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&use->y, ctx, 'v');
    w = _rsvg_css_normalize_length (&use->w, ctx, 'h');
    h = _rsvg_css_normalize_length (&use->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    /* <use> is an element that is used directly, unlike
     * <pattern>, which is used through a fill="url(#...)"
     * reference.  However, <use> will always reference another
     * element, potentially itself or an ancestor of itself (or
     * another <use> which references the first one, etc.).  So,
     * we acquire the <use> element itself so that circular
     * references can be caught.
     */
    self_acquired = rsvg_drawing_ctx_acquire_node_ref (ctx, self);
    if (!self_acquired) {
        goto out;
    }

    if (use->link == NULL) {
        goto out;
    }

    child = rsvg_acquire_node (ctx, use->link);
    if (!child) {
        goto out;
    }

    state = rsvg_current_state (ctx);
    if (RSVG_NODE_TYPE (child) != RSVG_NODE_TYPE_SYMBOL) {
        cairo_matrix_init_translate (&affine, x, y);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);

        rsvg_push_discrete_layer (ctx);
        rsvg_state_push (ctx);
        rsvg_node_draw (child, ctx, 1);
        rsvg_state_pop (ctx);

        rsvg_release_node (ctx, child);
        rsvg_pop_discrete_layer (ctx);
    } else {
        RsvgNodeSymbol *symbol = (RsvgNodeSymbol *) child;

        if (symbol->vbox.active) {
            rsvg_preserve_aspect_ratio
                (symbol->preserve_aspect_ratio,
                 symbol->vbox.rect.width, symbol->vbox.rect.height,
                 &w, &h, &x, &y);

            cairo_matrix_init_translate (&affine, x, y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            cairo_matrix_init_scale (&affine, w / symbol->vbox.rect.width, h / symbol->vbox.rect.height);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            cairo_matrix_init_translate (&affine, -symbol->vbox.rect.x, -symbol->vbox.rect.y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);

            _rsvg_push_view_box (ctx, symbol->vbox.rect.width, symbol->vbox.rect.height);
            rsvg_push_discrete_layer (ctx);
            if (!state->overflow || (!state->has_overflow && child->state->overflow))
                rsvg_add_clipping_rect (ctx, symbol->vbox.rect.x, symbol->vbox.rect.y,
                                        symbol->vbox.rect.width, symbol->vbox.rect.height);
        } else {
            cairo_matrix_init_translate (&affine, x, y);
            cairo_matrix_multiply (&state->affine, &affine, &state->affine);
            rsvg_push_discrete_layer (ctx);
        }

        rsvg_state_push (ctx);
        _rsvg_node_draw_children (child, ctx, 1);
        rsvg_state_pop (ctx);
        rsvg_pop_discrete_layer (ctx);
        if (symbol->vbox.active)
            _rsvg_pop_view_box (ctx);

        rsvg_release_node (ctx, child);
    }

out:

    if (self_acquired) {
        rsvg_release_node (ctx, self_acquired);
    }
}

static void
rsvg_node_svg_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeSvg *sself;
    RsvgState *state;
    cairo_matrix_t affine, affine_old, affine_new;
    guint i;
    double nx, ny, nw, nh;
    sself = (RsvgNodeSvg *) self;

    nx = _rsvg_css_normalize_length (&sself->x, ctx, 'h');
    ny = _rsvg_css_normalize_length (&sself->y, ctx, 'v');
    nw = _rsvg_css_normalize_length (&sself->w, ctx, 'h');
    nh = _rsvg_css_normalize_length (&sself->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    state = rsvg_current_state (ctx);

    affine_old = state->affine;

    if (sself->vbox.active) {
        double x = nx, y = ny, w = nw, h = nh;
        rsvg_preserve_aspect_ratio (sself->preserve_aspect_ratio,
                                    sself->vbox.rect.width, sself->vbox.rect.height,
                                    &w, &h, &x, &y);
        cairo_matrix_init (&affine,
                           w / sself->vbox.rect.width,
                           0,
                           0,
                           h / sself->vbox.rect.height,
                           x - sself->vbox.rect.x * w / sself->vbox.rect.width,
                           y - sself->vbox.rect.y * h / sself->vbox.rect.height);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);
        _rsvg_push_view_box (ctx, sself->vbox.rect.width, sself->vbox.rect.height);
    } else {
        cairo_matrix_init_translate (&affine, nx, ny);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);
        _rsvg_push_view_box (ctx, nw, nh);
    }

    affine_new = state->affine;

    rsvg_push_discrete_layer (ctx);

    /* Bounding box addition must be AFTER the discrete layer push, 
       which must be AFTER the transformation happens. */
    if (!state->overflow && self->parent) {
        state->affine = affine_old;
        rsvg_add_clipping_rect (ctx, nx, ny, nw, nh);
        state->affine = affine_new;
    }

    for (i = 0; i < self->children->len; i++) {
        rsvg_state_push (ctx);
        rsvg_node_draw (g_ptr_array_index (self->children, i), ctx, 0);
        rsvg_state_pop (ctx);
    }

    rsvg_pop_discrete_layer (ctx);
    _rsvg_pop_view_box (ctx);
}

static void
rsvg_node_svg_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgNodeSvg *svg = (RsvgNodeSvg *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
            svg->vbox = rsvg_css_parse_vbox (value);

        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            svg->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            svg->w = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            svg->h = _rsvg_css_parse_length (value);
        /* 
         * x & y attributes have no effect on outermost svg
         * http://www.w3.org/TR/SVG/struct.html#SVGElement 
         */
        if (self->parent && (value = rsvg_property_bag_lookup (atts, "x")))
            svg->x = _rsvg_css_parse_length (value);
        if (self->parent && (value = rsvg_property_bag_lookup (atts, "y")))
            svg->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            rsvg_defs_register_name (ctx->priv->defs, value, &svg->super);
        }
    }
}

RsvgNode *
rsvg_new_svg (void)
{
    RsvgNodeSvg *svg;
    svg = g_new (RsvgNodeSvg, 1);
    _rsvg_node_init (&svg->super, RSVG_NODE_TYPE_SVG);
    svg->vbox.active = FALSE;
    svg->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    svg->x = _rsvg_css_parse_length ("0");
    svg->y = _rsvg_css_parse_length ("0");
    svg->w = _rsvg_css_parse_length ("100%");
    svg->h = _rsvg_css_parse_length ("100%");
    svg->super.typename = "svg";
    svg->super.draw = rsvg_node_svg_draw;
    svg->super.free = _rsvg_node_free;
    svg->super.set_atts = rsvg_node_svg_set_atts;
    return &svg->super;
}

static void
rsvg_node_use_free (RsvgNode * node)
{
    RsvgNodeUse *use = (RsvgNodeUse *) node;
    g_free (use->link);
    _rsvg_node_free (node);
}

static void
rsvg_node_use_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value = NULL;
    RsvgNodeUse *use;

    use = (RsvgNodeUse *) self;
    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            use->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            use->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            use->w = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            use->h = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &use->super);
        if ((value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
            g_free (use->link);
            use->link = g_strdup (value);
        }
    }

}

RsvgNode *
rsvg_new_use (void)
{
    RsvgNodeUse *use;
    use = g_new (RsvgNodeUse, 1);
    _rsvg_node_init (&use->super, RSVG_NODE_TYPE_USE);
    use->super.typename = "use";
    use->super.draw = rsvg_node_use_draw;
    use->super.free = rsvg_node_use_free;
    use->super.set_atts = rsvg_node_use_set_atts;
    use->x = _rsvg_css_parse_length ("0");
    use->y = _rsvg_css_parse_length ("0");
    use->w = _rsvg_css_parse_length ("0");
    use->h = _rsvg_css_parse_length ("0");
    use->link = NULL;
    return (RsvgNode *) use;
}

static void
rsvg_node_symbol_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgNodeSymbol *symbol = (RsvgNodeSymbol *) self;

    const char *value;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, &symbol->super);
        if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
            symbol->vbox = rsvg_css_parse_vbox (value);
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            symbol->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
    }

}


RsvgNode *
rsvg_new_symbol (void)
{
    RsvgNodeSymbol *symbol;
    symbol = g_new (RsvgNodeSymbol, 1);
    _rsvg_node_init (&symbol->super, RSVG_NODE_TYPE_SYMBOL);
    symbol->vbox.active = FALSE;
    symbol->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    symbol->super.typename = "symbol";
    symbol->super.draw = _rsvg_node_draw_nothing;
    symbol->super.set_atts = rsvg_node_symbol_set_atts;
    return &symbol->super;
}

RsvgNode *
rsvg_new_defs (void)
{
    RsvgNodeGroup *group;
    group = g_new (RsvgNodeGroup, 1);
    _rsvg_node_init (&group->super, RSVG_NODE_TYPE_DEFS);
    group->super.typename = "g";
    group->super.draw = _rsvg_node_draw_nothing;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}

static void
_rsvg_node_switch_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    guint i;

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    rsvg_push_discrete_layer (ctx);

    for (i = 0; i < self->children->len; i++) {
        RsvgNode *drawable = g_ptr_array_index (self->children, i);

        if (drawable->state->cond_true) {
            rsvg_state_push (ctx);
            rsvg_node_draw (g_ptr_array_index (self->children, i), ctx, 0);
            rsvg_state_pop (ctx);

            break;              /* only render the 1st one */
        }
    }

    rsvg_pop_discrete_layer (ctx);
}

RsvgNode *
rsvg_new_switch (void)
{
    RsvgNodeGroup *group;
    group = g_new (RsvgNodeGroup, 1);
    _rsvg_node_init (&group->super, RSVG_NODE_TYPE_SWITCH);
    group->super.typename = "g";
    group->super.draw = _rsvg_node_switch_draw;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}
