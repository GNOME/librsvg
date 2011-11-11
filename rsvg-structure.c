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

    state = self->state;

    stacksave = ctx->drawsub_stack;
    if (stacksave) {
        if (stacksave->data != self)
            return;
        ctx->drawsub_stack = stacksave->next;
    }
    if (!state->visible)
        return;

    if (g_slist_find(ctx->ptrs, self) != NULL)
    {
        /*
         * 5.3.1 of the SVG 1.1 spec (http://www.w3.org/TR/SVG11/struct.html#HeadOverview)
         * seems to indicate ("URI references that directly or indirectly reference
         * themselves are treated as invalid circular references") that circular
         * references are invalid, and so we can drop them to avoid infinite recursion.
         * 
         * See also http://bugzilla.gnome.org/show_bug.cgi?id=518640
         */
        g_warning("Circular SVG reference noticed, dropping");
        return;
    }
    ctx->ptrs = g_slist_append(ctx->ptrs, self);

    self->draw (self, ctx, dominate);
    ctx->drawsub_stack = stacksave;

    ctx->ptrs = g_slist_remove(ctx->ptrs, self);
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
    self->free = _rsvg_node_free;
    self->draw = _rsvg_node_draw_nothing;
    self->set_atts = _rsvg_node_dont_set_atts;
}

void
_rsvg_node_finalize (RsvgNode * self)
{
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
    const char *klazz = NULL, *id = NULL, *value;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "g", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_group (void)
{
    RsvgNodeGroup *group;
    group = g_new (RsvgNodeGroup, 1);
    _rsvg_node_init (&group->super, RSVG_NODE_TYPE_GROUP);
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

static gboolean
rsvg_node_is_ancestor (RsvgNode * potential_ancestor, RsvgNode * potential_descendant)
{
    /* work our way up the family tree */
    while (TRUE) {
        if (potential_ancestor == potential_descendant)
            return TRUE;
        else if (potential_descendant->parent == NULL)
            return FALSE;
        else
            potential_descendant = potential_descendant->parent;
    }

    g_assert_not_reached ();
    return FALSE;
}

static void
rsvg_node_use_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeUse *use = (RsvgNodeUse *) self;
    RsvgNode *child;
    RsvgState *state;
    cairo_matrix_t affine;
    double x, y, w, h;
    x = _rsvg_css_normalize_length (&use->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&use->y, ctx, 'v');
    w = _rsvg_css_normalize_length (&use->w, ctx, 'h');
    h = _rsvg_css_normalize_length (&use->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    child = use->link;

    /* If it can find nothing to draw... draw nothing */
    if (!child)
        return;
    else if (rsvg_node_is_ancestor (child, self))       /* or, if we're <use>'ing ourself */
        return;

    state = rsvg_current_state (ctx);
    if (RSVG_NODE_TYPE (child) != RSVG_NODE_TYPE_SYMBOL) {
        cairo_matrix_init_translate (&affine, x, y);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);

        rsvg_push_discrete_layer (ctx);
        rsvg_state_push (ctx);
        rsvg_node_draw (child, ctx, 1);
        rsvg_state_pop (ctx);
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
        /*
         * style element is not loaded yet here, so we need to store those attribues
         * to be applied later.
         */
        svg->atts = rsvg_property_bag_dup(atts);
    }
}

void
_rsvg_node_svg_apply_atts (RsvgNodeSvg * self, RsvgHandle * ctx)
{
    const char *id = NULL, *klazz = NULL, *value;
    if (self->atts && rsvg_property_bag_size (self->atts)) {
        if ((value = rsvg_property_bag_lookup (self->atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (self->atts, "id")))
            id = value;
        rsvg_parse_style_attrs (ctx, ((RsvgNode *)self)->state, "svg", klazz, id, self->atts);
    }
}

static void
_rsvg_svg_free (RsvgNode * self)
{
    RsvgNodeSvg *svg = (RsvgNodeSvg *) self;

    if (svg->atts) {
        rsvg_property_bag_free (svg->atts);
        svg->atts = NULL;
    }

    _rsvg_node_free (self);
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
    svg->super.draw = rsvg_node_svg_draw;
    svg->super.free = _rsvg_svg_free;
    svg->super.set_atts = rsvg_node_svg_set_atts;
    svg->atts = NULL;
    return &svg->super;
}

static void
rsvg_node_use_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value = NULL, *klazz = NULL, *id = NULL;
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
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, &use->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
            rsvg_defs_add_resolver (ctx->priv->defs, &use->link, value);
        rsvg_parse_style_attrs (ctx, self->state, "use", klazz, id, atts);
    }

}

RsvgNode *
rsvg_new_use (void)
{
    RsvgNodeUse *use;
    use = g_new (RsvgNodeUse, 1);
    _rsvg_node_init (&use->super, RSVG_NODE_TYPE_USE);
    use->super.draw = rsvg_node_use_draw;
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

    const char *klazz = NULL, *value, *id = NULL;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, &symbol->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
            symbol->vbox = rsvg_css_parse_vbox (value);
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            symbol->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);

        rsvg_parse_style_attrs (ctx, self->state, "symbol", klazz, id, atts);
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
    group->super.draw = _rsvg_node_switch_draw;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}
