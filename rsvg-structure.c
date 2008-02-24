/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
_rsvg_node_init (RsvgNode * self)
{
	self->parent = NULL;
    self->children = g_ptr_array_new ();
    self->state = g_new (RsvgState, 1);
    rsvg_state_init (self->state);
    self->free = _rsvg_node_free;
    self->draw = _rsvg_node_draw_nothing;
    self->set_atts = _rsvg_node_dont_set_atts;
    self->type = NULL;
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
    if (self->type != NULL)
        g_string_free (self->type, TRUE);

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
    _rsvg_node_init (&group->super);
    group->super.draw = _rsvg_node_draw_children;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}

void
rsvg_pop_def_group (RsvgHandle * ctx)
{
    if (ctx->priv->currentnode != NULL)
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
    double affine[6];
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

    state = rsvg_state_current (ctx);
    if (strcmp (child->type->str, "symbol")) {
        _rsvg_affine_translate (affine, x, y);
        _rsvg_affine_multiply (state->affine, affine, state->affine);

        rsvg_push_discrete_layer (ctx);
        rsvg_state_push (ctx);
        rsvg_node_draw (child, ctx, 1);
        rsvg_state_pop (ctx);
        rsvg_pop_discrete_layer (ctx);
    } else {
        RsvgNodeSymbol *symbol = (RsvgNodeSymbol *) child;

        if (symbol->vbox.active) {
            rsvg_preserve_aspect_ratio
                (symbol->preserve_aspect_ratio, symbol->vbox.w, symbol->vbox.h, &w, &h, &x, &y);

            _rsvg_affine_translate (affine, x, y);
            _rsvg_affine_multiply (state->affine, affine, state->affine);
            _rsvg_affine_scale (affine, w / symbol->vbox.w, h / symbol->vbox.h);
            _rsvg_affine_multiply (state->affine, affine, state->affine);
            _rsvg_affine_translate (affine, -symbol->vbox.x, -symbol->vbox.y);
            _rsvg_affine_multiply (state->affine, affine, state->affine);

            _rsvg_push_view_box (ctx, symbol->vbox.w, symbol->vbox.h);
            rsvg_push_discrete_layer (ctx);
            if (!state->overflow || (!state->has_overflow && child->state->overflow))
                rsvg_add_clipping_rect (ctx, symbol->vbox.x, symbol->vbox.y,
                                        symbol->vbox.w, symbol->vbox.h);
        } else {
            _rsvg_affine_translate (affine, x, y);
            _rsvg_affine_multiply (state->affine, affine, state->affine);
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
    gdouble affine[6], affine_old[6], affine_new[6];
    guint i;
    double nx, ny, nw, nh;
    sself = (RsvgNodeSvg *) self;

    nx = _rsvg_css_normalize_length (&sself->x, ctx, 'h');
    ny = _rsvg_css_normalize_length (&sself->y, ctx, 'v');
    nw = _rsvg_css_normalize_length (&sself->w, ctx, 'h');
    nh = _rsvg_css_normalize_length (&sself->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, self->state, dominate);

    state = rsvg_state_current (ctx);

    for (i = 0; i < 6; i++)
        affine_old[i] = state->affine[i];

    if (sself->vbox.active) {
        double x = nx, y = ny, w = nw, h = nh;
        rsvg_preserve_aspect_ratio (sself->preserve_aspect_ratio,
                                    sself->vbox.w, sself->vbox.h, &w, &h, &x, &y);
        affine[0] = w / sself->vbox.w;
        affine[1] = 0;
        affine[2] = 0;
        affine[3] = h / sself->vbox.h;
        affine[4] = x - sself->vbox.x * w / sself->vbox.w;
        affine[5] = y - sself->vbox.y * h / sself->vbox.h;
        _rsvg_affine_multiply (state->affine, affine, state->affine);
        _rsvg_push_view_box (ctx, sself->vbox.w, sself->vbox.h);
    } else {
        affine[0] = 1;
        affine[1] = 0;
        affine[2] = 0;
        affine[3] = 1;
        affine[4] = nx;
        affine[5] = ny;
        _rsvg_affine_multiply (state->affine, affine, state->affine);
        _rsvg_push_view_box (ctx, nw, nh);
    }
    for (i = 0; i < 6; i++)
        affine_new[i] = state->affine[i];

    rsvg_push_discrete_layer (ctx);

    /* Bounding box addition must be AFTER the discrete layer push, 
       which must be AFTER the transformation happens. */
    if (!state->overflow && self->parent) {
        for (i = 0; i < 6; i++)
            state->affine[i] = affine_old[i];
        rsvg_add_clipping_rect (ctx, nx, ny, nw, nh);
        for (i = 0; i < 6; i++)
            state->affine[i] = affine_new[i];
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
    const char *id = NULL, *klazz = NULL, *value;
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
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, &svg->super);
        }
        rsvg_parse_style_attrs (ctx, self->state, "svg", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_svg (void)
{
    RsvgNodeSvg *svg;
    svg = g_new (RsvgNodeSvg, 1);
    _rsvg_node_init (&svg->super);
    svg->vbox.active = FALSE;
    svg->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    svg->x = _rsvg_css_parse_length ("0");
    svg->y = _rsvg_css_parse_length ("0");
    svg->w = _rsvg_css_parse_length ("100%");
    svg->h = _rsvg_css_parse_length ("100%");
    svg->super.draw = rsvg_node_svg_draw;
    svg->super.set_atts = rsvg_node_svg_set_atts;
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
rsvg_new_use ()
{
    RsvgNodeUse *use;
    use = g_new (RsvgNodeUse, 1);
    _rsvg_node_init (&use->super);
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
    _rsvg_node_init (&symbol->super);
    symbol->vbox.active = FALSE;
    symbol->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    symbol->super.draw = _rsvg_node_draw_nothing;
    symbol->super.set_atts = rsvg_node_symbol_set_atts;
    return &symbol->super;
}

RsvgNode *
rsvg_new_defs ()
{
    RsvgNodeGroup *group;
    group = g_new (RsvgNodeGroup, 1);
    _rsvg_node_init (&group->super);
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
    _rsvg_node_init (&group->super);
    group->super.draw = _rsvg_node_switch_draw;
    group->super.set_atts = rsvg_node_group_set_atts;
    return &group->super;
}
