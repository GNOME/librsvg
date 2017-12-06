/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-text.c: Text handling routines for RSVG

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

#include <string.h>

#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-text.h"
#include "rsvg-css.h"

#include "rsvg-shapes.h"

/* what we use for text rendering depends on what cairo has to offer */
#include <pango/pangocairo.h>

typedef struct _RsvgNodeText RsvgNodeText;

struct _RsvgNodeText {
    RsvgLength x, y;
    gboolean x_specified;
    gboolean y_specified;
    RsvgLength dx, dy;
};

typedef struct _RsvgNodeTref RsvgNodeTref;

struct _RsvgNodeTref {
    char *link;
};

static GString *
_rsvg_text_chomp (RsvgState *state, GString * in, gboolean * lastwasspace)
{
    GString *out;
    guint i;
    out = g_string_new (in->str);

    if (!state->space_preserve) {
        for (i = 0; i < out->len;) {
            if (out->str[i] == '\n')
                g_string_erase (out, i, 1);
            else
                i++;
        }

        for (i = 0; i < out->len; i++)
            if (out->str[i] == '\t')
                out->str[i] = ' ';

        for (i = 0; i < out->len;) {
            if (out->str[i] == ' ' && *lastwasspace)
                g_string_erase (out, i, 1);
            else {
                if (out->str[i] == ' ')
                    *lastwasspace = TRUE;
                else
                    *lastwasspace = FALSE;
                i++;
            }
        }
    }

    return out;
}

static void
set_text_common_atts (RsvgNodeText *text, RsvgPropertyBag *atts)
{
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "x"))) {
        text->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        text->x_specified = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "y"))) {
        text->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        text->y_specified = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "dx")))
        text->dx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
    if ((value = rsvg_property_bag_lookup (atts, "dy")))
        text->dy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
}


static void
rsvg_node_text_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeText *text = impl;

    set_text_common_atts (text, atts);
}

static void rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y);

static void
draw_from_children (RsvgNode * self, RsvgDrawingCtx * ctx,
                    gdouble * x, gdouble * y, gboolean * lastwasspace,
                    gboolean usetextonly);

static void
_rsvg_node_text_type_tspan (RsvgNode *node, RsvgNodeText *self, RsvgDrawingCtx *ctx,
                            gdouble *x, gdouble *y, gboolean *lastwasspace,
                            gboolean usetextonly);

static void
_rsvg_node_text_type_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx,
                           gdouble * x, gdouble * y, gboolean * lastwasspace,
                           gboolean usetextonly);

typedef struct {
    RsvgDrawingCtx *ctx;
    gdouble *x;
    gdouble *y;
    gboolean *lastwasspace;
    gboolean usetextonly;
} DrawTextClosure;

static gboolean
draw_text_child (RsvgNode *node, gpointer data)
{
    DrawTextClosure *closure;
    RsvgNodeType type = rsvg_node_get_type (node);

    closure = data;

    if (type == RSVG_NODE_TYPE_CHARS) {
        const char *chars_str;
        gsize chars_len;
        GString *string;
        GString *chomped;

        rsvg_node_chars_get_string (node, &chars_str, &chars_len);
        string = g_string_new_len (chars_str, chars_len);

        chomped = _rsvg_text_chomp (rsvg_current_state (closure->ctx), string, closure->lastwasspace);
        g_string_free (string, TRUE);

        rsvg_text_render_text (closure->ctx, chomped->str, closure->x, closure->y);
        g_string_free (chomped, TRUE);
    } else {
        if (closure->usetextonly) {
            draw_from_children (node,
                                closure->ctx,
                                closure->x,
                                closure->y,
                                closure->lastwasspace,
                                closure->usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                RsvgNodeText *tspan = rsvg_rust_cnode_get_impl (node);
                _rsvg_node_text_type_tspan (node,
                                            tspan,
                                            closure->ctx,
                                            closure->x,
                                            closure->y,
                                            closure->lastwasspace,
                                            closure->usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                RsvgNodeTref *tref = rsvg_rust_cnode_get_impl (node);
                _rsvg_node_text_type_tref (tref,
                                           closure->ctx,
                                           closure->x,
                                           closure->y,
                                           closure->lastwasspace,
                                           closure->usetextonly);
            }
        }
    }

    return TRUE;
}

/* This function is responsible of selecting render for a text element including its children and giving it the drawing context */
static void
draw_from_children (RsvgNode * self, RsvgDrawingCtx * ctx,
                    gdouble * x, gdouble * y, gboolean * lastwasspace,
                    gboolean usetextonly)
{
    DrawTextClosure closure;

    rsvg_push_discrete_layer (ctx);

    closure.ctx = ctx;
    closure.x = x;
    closure.y = y;
    closure.lastwasspace = lastwasspace;
    closure.usetextonly = usetextonly;

    rsvg_node_foreach_child (self, draw_text_child, &closure);

    rsvg_pop_discrete_layer (ctx);
}

static gboolean
compute_length_from_children (RsvgNode       *self,
                              RsvgDrawingCtx *ctx,
                              gdouble        *length,
                              gboolean       *lastwasspace,
                              gboolean        usetextonly);

static gboolean
length_from_tref (RsvgNodeTref   *self,
                  RsvgDrawingCtx *ctx,
                  gdouble        *x,
                  gboolean       *lastwasspace,
                  gboolean        usetextonly);

static gboolean
length_from_tspan (RsvgNode       *node,
                   RsvgNodeText   *self,
                   RsvgDrawingCtx *ctx,
                   gdouble        *x,
                   gboolean       *lastwasspace,
                   gboolean        usetextonly);

static gdouble rsvg_text_length_text_as_string (RsvgDrawingCtx * ctx, const char *text);

typedef struct {
    RsvgDrawingCtx *ctx;
    gdouble *length;
    gboolean *lastwasspace;
    gboolean usetextonly;
    gboolean done;
} ChildrenLengthClosure;

static gboolean
compute_child_length (RsvgNode *node, gpointer data)
{
    ChildrenLengthClosure *closure;
    RsvgNodeType type = rsvg_node_get_type (node);
    gboolean done;

    closure = data;
    done = FALSE;

    rsvg_state_push (closure->ctx);
    rsvg_state_reinherit_top (closure->ctx, rsvg_node_get_state (node), 0);

    if (type == RSVG_NODE_TYPE_CHARS) {
        const char *chars_str;
        gsize chars_len;
        GString *string;
        GString *chomped;

        rsvg_node_chars_get_string (node, &chars_str, &chars_len);
        string = g_string_new_len (chars_str, chars_len);

        chomped = _rsvg_text_chomp (rsvg_current_state (closure->ctx), string, closure->lastwasspace);
        g_string_free (string, TRUE);

        *closure->length += rsvg_text_length_text_as_string (closure->ctx, chomped->str);
        g_string_free (chomped, TRUE);
    } else {
        if (closure->usetextonly) {
            done = compute_length_from_children (node,
                                                 closure->ctx,
                                                 closure->length,
                                                 closure->lastwasspace,
                                                 closure->usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                RsvgNodeText *tspan = rsvg_rust_cnode_get_impl (node);
                done = length_from_tspan (node,
                                          tspan,
                                          closure->ctx,
                                          closure->length,
                                          closure->lastwasspace,
                                          closure->usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                RsvgNodeTref *tref = rsvg_rust_cnode_get_impl (node);
                done = length_from_tref (tref,
                                         closure->ctx,
                                         closure->length,
                                         closure->lastwasspace,
                                         closure->usetextonly);
            }
        }
    }

    rsvg_state_pop (closure->ctx);

    closure->done = done;
    return !done;
}

static gboolean
compute_length_from_children (RsvgNode * self, RsvgDrawingCtx * ctx,
                              gdouble * length, gboolean * lastwasspace,
                              gboolean usetextonly)
{
    ChildrenLengthClosure closure;

    closure.ctx = ctx;
    closure.length = length;
    closure.lastwasspace = lastwasspace;
    closure.usetextonly = usetextonly;
    closure.done = FALSE;

    rsvg_node_foreach_child (self, compute_child_length, &closure);

    return closure.done;
}


static void
rsvg_node_text_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    RsvgNodeText *text = impl;
    double x, y, dx, dy, length = 0;
    gboolean lastwasspace = TRUE;

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    x = rsvg_length_normalize (&text->x, ctx);
    y = rsvg_length_normalize (&text->y, ctx);
    dx = rsvg_length_normalize (&text->dx, ctx);
    dy = rsvg_length_normalize (&text->dy, ctx);

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        compute_length_from_children (node, ctx, &length, &lastwasspace, FALSE);
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
            length /= 2;
    }
    if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity)) {
        y -= length;
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
            dy /= 2;
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
            dy = 0;
    } else {
        x -= length;
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
            dx /= 2;
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
            dx = 0;
    }
    x += dx;
    y += dy;

    lastwasspace = TRUE;
    draw_from_children (node, ctx, &x, &y, &lastwasspace, FALSE);
}

RsvgNode *
rsvg_new_text (const char *element_name, RsvgNode *parent)
{
    RsvgNodeText *text;

    text = g_new0 (RsvgNodeText, 1);
    text->x = text->y = text->dx = text->dy = rsvg_length_parse ("0", LENGTH_DIR_BOTH);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_TEXT,
                                parent,
                                rsvg_state_new (),
                                text,
                                rsvg_node_text_set_atts,
                                rsvg_node_text_draw,
                                g_free);                                
}

static void
_rsvg_node_text_type_tspan (RsvgNode *node, RsvgNodeText *self, RsvgDrawingCtx *ctx,
                            gdouble *x, gdouble *y, gboolean *lastwasspace,
                            gboolean usetextonly)
{
    double dx, dy, length = 0;

    rsvg_state_push (ctx);

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), 0);

    dx = rsvg_length_normalize (&self->dx, ctx);
    dy = rsvg_length_normalize (&self->dy, ctx);

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        gboolean lws = *lastwasspace;
        compute_length_from_children (node, ctx, &length, &lws, usetextonly);
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
            length /= 2;
    }

    if (self->x_specified) {
        *x = rsvg_length_normalize (&self->x, ctx);
        if (!PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity)) {
            *x -= length;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
                dx /= 2;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
                dx = 0;
        }
    }
    *x += dx;

    if (self->y_specified) {
        *y = rsvg_length_normalize (&self->y, ctx);
        if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity)) {
            *y -= length;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
                dy /= 2;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
                dy = 0;
        }
    }
    *y += dy;
    draw_from_children (node, ctx, x, y, lastwasspace, usetextonly);

    rsvg_state_pop (ctx);
}

static gboolean
length_from_tspan (RsvgNode       *node,
                   RsvgNodeText   *self,
                   RsvgDrawingCtx *ctx,
                   gdouble        *length,
                   gboolean       *lastwasspace,
                   gboolean        usetextonly)
{
    if (self->x_specified || self->y_specified)
        return TRUE;

    if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity))
        *length += rsvg_length_normalize (&self->dy, ctx);
    else
        *length += rsvg_length_normalize (&self->dx, ctx);

    return compute_length_from_children (node, ctx, length,
                                         lastwasspace, usetextonly);
}

static void
rsvg_node_tspan_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeText *text = impl;

    set_text_common_atts (text, atts);
}

static void
rsvg_node_tspan_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing */
}

RsvgNode *
rsvg_new_tspan (const char *element_name, RsvgNode *parent)
{
    RsvgNodeText *text;

    text = g_new0 (RsvgNodeText, 1);
    text->dx = text->dy = rsvg_length_parse ("0", LENGTH_DIR_BOTH);

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_TSPAN,
                                parent,
                                rsvg_state_new (),
                                text,
                                rsvg_node_tspan_set_atts,
                                rsvg_node_tspan_draw,
                                g_free);
}

static void
_rsvg_node_text_type_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx,
                           gdouble * x, gdouble * y, gboolean * lastwasspace,
                           gboolean usetextonly)
{
    RsvgNode *link;

    if (self->link == NULL)
      return;
    link = rsvg_drawing_ctx_acquire_node (ctx, self->link);
    if (link == NULL)
      return;

    draw_from_children (link, ctx, x, y, lastwasspace, TRUE);

    rsvg_drawing_ctx_release_node (ctx, link);
}

static gboolean
length_from_tref (RsvgNodeTref   *self,
                  RsvgDrawingCtx *ctx,
                  gdouble        *x,
                  gboolean       *lastwasspace,
                  gboolean        usetextonly)
{
    gboolean result;
    RsvgNode *link;

    if (self->link == NULL)
      return FALSE;
    link = rsvg_drawing_ctx_acquire_node (ctx, self->link);
    if (link == NULL)
      return FALSE;

    result = compute_length_from_children (link, ctx, x, lastwasspace, TRUE);

    rsvg_drawing_ctx_release_node (ctx, link);

    return result;
}

static void
rsvg_node_tref_free (gpointer impl)
{
    RsvgNodeTref *self = impl;

    g_free (self->link);
    g_free (self);
}

static void
rsvg_node_tref_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgNodeTref *text = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
        g_free (text->link);
        text->link = g_strdup (value);
    }
}

static void
rsvg_node_tref_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing */
}

RsvgNode *
rsvg_new_tref (const char *element_name, RsvgNode *parent)
{
    RsvgNodeTref *text;

    text = g_new0 (RsvgNodeTref, 1);
    text->link = NULL;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_TREF,
                                parent,
                                rsvg_state_new (),
                                text,
                                rsvg_node_tref_set_atts,
                                rsvg_node_tref_draw,
                                rsvg_node_tref_free);
}

typedef struct _RsvgTextLayout RsvgTextLayout;

struct _RsvgTextLayout {
    PangoLayout *layout;
    RsvgDrawingCtx *ctx;
    TextAnchor anchor;
    gdouble x, y;
};

static void
rsvg_text_layout_free (RsvgTextLayout * layout)
{
    g_object_unref (layout->layout);
    g_free (layout);
}

static PangoLayout *
rsvg_text_create_layout (RsvgDrawingCtx * ctx, const char *text, PangoContext * context)
{
    RsvgState *state;
    PangoFontDescription *font_desc;
    PangoLayout *layout;
    PangoAttrList *attr_list;
    PangoAttribute *attribute;
    double dpi_y;

    state = rsvg_current_state (ctx);

    if (state->lang)
        pango_context_set_language (context, pango_language_from_string (state->lang));

    if (state->unicode_bidi == UNICODE_BIDI_OVERRIDE || state->unicode_bidi == UNICODE_BIDI_EMBED)
        pango_context_set_base_dir (context, state->text_dir);

    if (PANGO_GRAVITY_IS_VERTICAL (state->text_gravity))
        pango_context_set_base_gravity (context, state->text_gravity);

    font_desc = pango_font_description_copy (pango_context_get_font_description (context));

    if (state->font_family)
        pango_font_description_set_family_static (font_desc, state->font_family);

    pango_font_description_set_style (font_desc, state->font_style);
    pango_font_description_set_variant (font_desc, state->font_variant);
    pango_font_description_set_weight (font_desc, state->font_weight);
    pango_font_description_set_stretch (font_desc, state->font_stretch);

    rsvg_drawing_ctx_get_dpi (ctx, NULL, &dpi_y);
    pango_font_description_set_size (font_desc,
                                     rsvg_drawing_ctx_get_normalized_font_size (ctx) * PANGO_SCALE / dpi_y * 72);

    layout = pango_layout_new (context);
    pango_layout_set_font_description (layout, font_desc);
    pango_font_description_free (font_desc);

    attr_list = pango_attr_list_new ();
    attribute = pango_attr_letter_spacing_new (rsvg_length_normalize (&state->letter_spacing, ctx) * PANGO_SCALE);
    attribute->start_index = 0;
    attribute->end_index = G_MAXINT;
    pango_attr_list_insert (attr_list, attribute);

    if (state->has_font_decor && text) {
        if (state->font_decor & TEXT_UNDERLINE) {
            attribute = pango_attr_underline_new (PANGO_UNDERLINE_SINGLE);
            attribute->start_index = 0;
            attribute->end_index = -1;
            pango_attr_list_insert (attr_list, attribute);
        }
	if (state->font_decor & TEXT_STRIKE) {
            attribute = pango_attr_strikethrough_new (TRUE);
            attribute->start_index = 0;
            attribute->end_index = -1;
            pango_attr_list_insert (attr_list, attribute);
	}
    }

    pango_layout_set_attributes (layout, attr_list);
    pango_attr_list_unref (attr_list);

    if (text)
        pango_layout_set_text (layout, text, -1);
    else
        pango_layout_set_text (layout, NULL, 0);

    pango_layout_set_alignment (layout, (state->text_dir == PANGO_DIRECTION_LTR) ?
                                PANGO_ALIGN_LEFT : PANGO_ALIGN_RIGHT);

    return layout;
}


static RsvgTextLayout *
rsvg_text_layout_new (RsvgDrawingCtx * ctx, const char *text)
{
    RsvgState *state;
    RsvgTextLayout *layout;

    state = rsvg_current_state (ctx);

    if (ctx->pango_context == NULL)
        ctx->pango_context = ctx->render->create_pango_context (ctx);

    layout = g_new0 (RsvgTextLayout, 1);

    layout->layout = rsvg_text_create_layout (ctx, text, ctx->pango_context);
    layout->ctx = ctx;

    layout->anchor = state->text_anchor;

    return layout;
}

static void
rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y)
{
    PangoContext *context;
    PangoLayout *layout;
    PangoLayoutIter *iter;
    RsvgState *state;
    gint w, h;
    double offset_x, offset_y, offset;

    state = rsvg_current_state (ctx);

    /* Do not render the text if the font size is zero. See bug #581491. */
    if (state->font_size.length == 0)
        return;

    context = ctx->render->create_pango_context (ctx);
    layout = rsvg_text_create_layout (ctx, text, context);
    pango_layout_get_size (layout, &w, &h);
    iter = pango_layout_get_iter (layout);
    offset = pango_layout_iter_get_baseline (iter) / (double) PANGO_SCALE;
    offset += _rsvg_css_accumulate_baseline_shift (state, ctx);
    if (PANGO_GRAVITY_IS_VERTICAL (state->text_gravity)) {
        offset_x = -offset;
        offset_y = 0;
    } else {
        offset_x = 0;
        offset_y = offset;
    }
    pango_layout_iter_free (iter);
    ctx->render->render_pango_layout (ctx, layout, *x - offset_x, *y - offset_y);
    if (PANGO_GRAVITY_IS_VERTICAL (state->text_gravity))
        *y += w / (double)PANGO_SCALE;
    else
        *x += w / (double)PANGO_SCALE;

    g_object_unref (layout);
    g_object_unref (context);
}

static gdouble
rsvg_text_layout_width (RsvgTextLayout * layout)
{
    gint width;

    pango_layout_get_size (layout->layout, &width, NULL);

    return width / (double)PANGO_SCALE;
}

static gdouble
rsvg_text_length_text_as_string (RsvgDrawingCtx * ctx, const char *text)
{
    RsvgTextLayout *layout;
    gdouble x;

    layout = rsvg_text_layout_new (ctx, text);
    layout->x = layout->y = 0;

    x = rsvg_text_layout_width (layout);

    rsvg_text_layout_free (layout);
    return x;
}
