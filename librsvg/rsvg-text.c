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

/* Keep in sync with rust/src/space.rs:XmlSpace */
typedef enum {
    XML_SPACE_DEFAULT,
    XML_SPACE_PRESERVE
} XmlSpace;

/* Implemented in rust/src/space.rs */
extern char *rsvg_xml_space_normalize (XmlSpace mode, const char *s);

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

static void
set_text_common_atts (RsvgNodeText *text, RsvgPropertyBag *atts)
{
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_X:
            text->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            text->x_specified = TRUE;
            break;

        case RSVG_ATTRIBUTE_Y:
            text->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            text->y_specified = TRUE;
            break;

        case RSVG_ATTRIBUTE_DX:
            text->dx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
            break;

        case RSVG_ATTRIBUTE_DY:
            text->dy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}


static void
rsvg_node_text_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgNodeText *text = impl;

    set_text_common_atts (text, atts);
}

static void rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y);

static void
draw_from_children (RsvgNode       *self,
                    RsvgDrawingCtx *ctx,
                    gdouble        *x,
                    gdouble        *y,
                    gboolean        usetextonly);

static void
draw_tspan (RsvgNode       *node,
            RsvgNodeText   *self,
            RsvgDrawingCtx *ctx,
            gdouble        *x,
            gdouble        *y,
            gboolean        usetextonly);

static void
draw_tref (RsvgNodeTref   *self,
           RsvgDrawingCtx *ctx,
           gdouble        *x,
           gdouble        *y,
           gboolean        usetextonly);

typedef struct {
    RsvgDrawingCtx *ctx;
    gdouble *x;
    gdouble *y;
    gboolean usetextonly;
} DrawTextClosure;

static XmlSpace
xml_space_from_current_state (RsvgDrawingCtx *ctx)
{
    RsvgState *state = rsvg_current_state (ctx);

    if (state->space_preserve) {
        return XML_SPACE_PRESERVE;
    } else {
        return XML_SPACE_DEFAULT;
    }
}

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
        char *chomped;

        rsvg_node_chars_get_string (node, &chars_str, &chars_len);
        string = g_string_new_len (chars_str, chars_len);

        chomped = rsvg_xml_space_normalize (xml_space_from_current_state (closure->ctx), string->str);
        g_string_free (string, TRUE);

        rsvg_text_render_text (closure->ctx, chomped, closure->x, closure->y);
        g_free (chomped);
    } else {
        if (closure->usetextonly) {
            draw_from_children (node,
                                closure->ctx,
                                closure->x,
                                closure->y,
                                closure->usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                RsvgNodeText *tspan = rsvg_rust_cnode_get_impl (node);
                draw_tspan (node,
                            tspan,
                            closure->ctx,
                            closure->x,
                            closure->y,
                            closure->usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                RsvgNodeTref *tref = rsvg_rust_cnode_get_impl (node);
                draw_tref (tref,
                           closure->ctx,
                           closure->x,
                           closure->y,
                           closure->usetextonly);
            }
        }
    }

    return TRUE;
}

/* This function is responsible of selecting render for a text element including its children and giving it the drawing context */
static void
draw_from_children (RsvgNode       *self,
                    RsvgDrawingCtx *ctx,
                    gdouble        *x,
                    gdouble        *y,
                    gboolean        usetextonly)
{
    DrawTextClosure closure;

    rsvg_push_discrete_layer (ctx);

    closure.ctx = ctx;
    closure.x = x;
    closure.y = y;
    closure.usetextonly = usetextonly;

    rsvg_node_foreach_child (self, draw_text_child, &closure);

    rsvg_pop_discrete_layer (ctx);
}

static gboolean
compute_length_from_children (RsvgNode       *self,
                              RsvgDrawingCtx *ctx,
                              gdouble        *length,
                              gboolean        usetextonly);

static gboolean
length_from_tref (RsvgNodeTref   *self,
                  RsvgDrawingCtx *ctx,
                  gdouble        *x,
                  gboolean        usetextonly);

static gboolean
length_from_tspan (RsvgNode       *node,
                   RsvgNodeText   *self,
                   RsvgDrawingCtx *ctx,
                   gdouble        *x,
                   gboolean        usetextonly);

static gdouble measure_text (RsvgDrawingCtx * ctx, const char *text);

typedef struct {
    RsvgDrawingCtx *ctx;
    gdouble *length;
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
        char *chomped;

        rsvg_node_chars_get_string (node, &chars_str, &chars_len);
        string = g_string_new_len (chars_str, chars_len);

        chomped = rsvg_xml_space_normalize (xml_space_from_current_state (closure->ctx), string->str);
        g_string_free (string, TRUE);

        *closure->length += measure_text (closure->ctx, chomped);
        g_free (chomped);
    } else {
        if (closure->usetextonly) {
            done = compute_length_from_children (node,
                                                 closure->ctx,
                                                 closure->length,
                                                 closure->usetextonly);
        } else {
            if (type == RSVG_NODE_TYPE_TSPAN) {
                RsvgNodeText *tspan = rsvg_rust_cnode_get_impl (node);
                done = length_from_tspan (node,
                                          tspan,
                                          closure->ctx,
                                          closure->length,
                                          closure->usetextonly);
            } else if (type == RSVG_NODE_TYPE_TREF) {
                RsvgNodeTref *tref = rsvg_rust_cnode_get_impl (node);
                done = length_from_tref (tref,
                                         closure->ctx,
                                         closure->length,
                                         closure->usetextonly);
            }
        }
    }

    rsvg_state_pop (closure->ctx);

    closure->done = done;
    return !done;
}

static gboolean
compute_length_from_children (RsvgNode       *self,
                              RsvgDrawingCtx *ctx,
                              gdouble        *length,
                              gboolean        usetextonly)
{
    ChildrenLengthClosure closure;

    closure.ctx = ctx;
    closure.length = length;
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

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), dominate);

    x = rsvg_length_normalize (&text->x, ctx);
    y = rsvg_length_normalize (&text->y, ctx);
    dx = rsvg_length_normalize (&text->dx, ctx);
    dy = rsvg_length_normalize (&text->dy, ctx);

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        compute_length_from_children (node, ctx, &length, FALSE);
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

    draw_from_children (node, ctx, &x, &y, FALSE);
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
draw_tspan (RsvgNode       *node,
            RsvgNodeText   *self,
            RsvgDrawingCtx *ctx,
            gdouble        *x,
            gdouble        *y,
            gboolean        usetextonly)
{
    double dx, dy, length = 0;

    rsvg_state_push (ctx);

    rsvg_state_reinherit_top (ctx, rsvg_node_get_state (node), 0);

    dx = rsvg_length_normalize (&self->dx, ctx);
    dy = rsvg_length_normalize (&self->dy, ctx);

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        compute_length_from_children (node, ctx, &length, usetextonly);
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
    draw_from_children (node, ctx, x, y, usetextonly);

    rsvg_state_pop (ctx);
}

static gboolean
length_from_tspan (RsvgNode       *node,
                   RsvgNodeText   *self,
                   RsvgDrawingCtx *ctx,
                   gdouble        *length,
                   gboolean        usetextonly)
{
    if (self->x_specified || self->y_specified)
        return TRUE;

    if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity))
        *length += rsvg_length_normalize (&self->dy, ctx);
    else
        *length += rsvg_length_normalize (&self->dx, ctx);

    return compute_length_from_children (node, ctx, length, usetextonly);
}

static void
rsvg_node_tspan_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
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
draw_tref (RsvgNodeTref   *self,
           RsvgDrawingCtx *ctx,
           gdouble        *x,
           gdouble        *y,
           gboolean        usetextonly)
{
    RsvgNode *link;

    if (self->link == NULL)
      return;
    link = rsvg_drawing_ctx_acquire_node (ctx, self->link);
    if (link == NULL)
      return;

    draw_from_children (link, ctx, x, y, TRUE);

    rsvg_drawing_ctx_release_node (ctx, link);
}

static gboolean
length_from_tref (RsvgNodeTref   *self,
                  RsvgDrawingCtx *ctx,
                  gdouble        *x,
                  gboolean        usetextonly)
{
    gboolean result;
    RsvgNode *link;

    if (self->link == NULL)
      return FALSE;
    link = rsvg_drawing_ctx_acquire_node (ctx, self->link);
    if (link == NULL)
      return FALSE;

    result = compute_length_from_children (link, ctx, x, TRUE);

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
rsvg_node_tref_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgNodeTref *text = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        if (attr == RSVG_ATTRIBUTE_XLINK_HREF) {
            g_free (text->link);
            text->link = g_strdup (value);
        }
    }

    rsvg_property_bag_iter_end (iter);
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

/* Defined in rust/src/text.rs */
extern PangoLayout *rsvg_text_create_layout (RsvgDrawingCtx *ctx, const char *text);

static void
rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y)
{
    PangoLayout *layout;
    PangoLayoutIter *iter;
    RsvgState *state;
    gint w, h;
    double offset_x, offset_y, offset;
    PangoGravity gravity;

    state = rsvg_current_state (ctx);

    layout = rsvg_text_create_layout (ctx, text);
    pango_layout_get_size (layout, &w, &h);
    iter = pango_layout_get_iter (layout);
    offset = pango_layout_iter_get_baseline (iter) / (double) PANGO_SCALE;
    offset += _rsvg_css_accumulate_baseline_shift (state, ctx);

    gravity = rsvg_state_get_text_gravity (state);

    if (PANGO_GRAVITY_IS_VERTICAL (gravity)) {
        offset_x = -offset;
        offset_y = 0;
    } else {
        offset_x = 0;
        offset_y = offset;
    }
    pango_layout_iter_free (iter);
    rsvg_drawing_ctx_render_pango_layout (ctx, layout, *x - offset_x, *y - offset_y);
    if (PANGO_GRAVITY_IS_VERTICAL (gravity))
        *y += w / (double)PANGO_SCALE;
    else
        *x += w / (double)PANGO_SCALE;

    g_object_unref (layout);
}

static gdouble
measure_text (RsvgDrawingCtx * ctx, const char *text)
{
    PangoLayout *layout;
    gint width;
    gdouble scaled_width;

    layout = rsvg_text_create_layout (ctx, text);

    pango_layout_get_size (layout, &width, NULL);
    scaled_width = width / (double)PANGO_SCALE;

    g_object_unref (layout);

    return scaled_width;
}
