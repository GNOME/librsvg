/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
    RsvgNode super;
    RsvgLength x, y, dx, dy;
};

typedef struct _RsvgNodeTref RsvgNodeTref;

struct _RsvgNodeTref {
    RsvgNode super;
    RsvgNode *link;
};
char *
rsvg_make_valid_utf8 (const char *str, int len)
{
    GString *string;
    const char *remainder, *invalid;
    int remaining_bytes, valid_bytes;

    string = NULL;
    remainder = str;

    if (len < 0)
        remaining_bytes = strlen (str);
    else
        remaining_bytes = len;

    while (remaining_bytes != 0) {
        if (g_utf8_validate (remainder, remaining_bytes, &invalid))
            break;
        valid_bytes = invalid - remainder;

        if (string == NULL)
            string = g_string_sized_new (remaining_bytes);

        g_string_append_len (string, remainder, valid_bytes);
        g_string_append_c (string, '?');

        remaining_bytes -= valid_bytes + 1;
        remainder = invalid + 1;
    }

    if (string == NULL)
        return len < 0 ? g_strndup (str, len) : g_strdup (str);

    g_string_append (string, remainder);

    return g_string_free (string, FALSE);
}

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
_rsvg_node_text_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeText *text = (RsvgNodeText *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            text->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            text->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dx")))
            text->dx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dy")))
            text->dy = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "text", klazz, id, atts);
    }
}

static void
 rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y);


static void
 _rsvg_node_text_type_tspan (RsvgNodeText * self, RsvgDrawingCtx * ctx,
                             gdouble * x, gdouble * y, gboolean * lastwasspace,
                             gboolean usetextonly);

static void
 _rsvg_node_text_type_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx,
                            gdouble * x, gdouble * y, gboolean * lastwasspace,
                            gboolean usetextonly);

static void
_rsvg_node_text_type_children (RsvgNode * self, RsvgDrawingCtx * ctx,
                               gdouble * x, gdouble * y, gboolean * lastwasspace,
                               gboolean usetextonly)
{
    guint i;

    rsvg_push_discrete_layer (ctx);
    for (i = 0; i < self->children->len; i++) {
        RsvgNode *node = g_ptr_array_index (self->children, i);
        RsvgNodeType type = RSVG_NODE_TYPE (node);

        if (type == RSVG_NODE_TYPE_CHARS) {
            RsvgNodeChars *chars = (RsvgNodeChars *) node;
            GString *str = _rsvg_text_chomp (rsvg_current_state (ctx), chars->contents, lastwasspace);
            rsvg_text_render_text (ctx, str->str, x, y);
            g_string_free (str, TRUE);
        } else {
            if (usetextonly) {
                _rsvg_node_text_type_children (node, ctx, x, y, lastwasspace,
                                               usetextonly);
            } else {
                if (type == RSVG_NODE_TYPE_TSPAN) {
                    RsvgNodeText *tspan = (RsvgNodeText *) node;
                    rsvg_state_push (ctx);
                    _rsvg_node_text_type_tspan (tspan, ctx, x, y, lastwasspace,
                                                usetextonly);
                    rsvg_state_pop (ctx);
                } else if (type == RSVG_NODE_TYPE_TREF) {
                    RsvgNodeTref *tref = (RsvgNodeTref *) node;
                    _rsvg_node_text_type_tref (tref, ctx, x, y, lastwasspace,
                                               usetextonly);
                }
            }
        }
    }
    rsvg_pop_discrete_layer (ctx);
}

static int
 _rsvg_node_text_length_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx,
                              gdouble * x, gboolean * lastwasspace,
                              gboolean usetextonly);

static int
 _rsvg_node_text_length_tspan (RsvgNodeText * self, RsvgDrawingCtx * ctx,
                               gdouble * x, gboolean * lastwasspace,
                               gboolean usetextonly);

static gdouble rsvg_text_length_text_as_string (RsvgDrawingCtx * ctx, const char *text);

static int
_rsvg_node_text_length_children (RsvgNode * self, RsvgDrawingCtx * ctx,
                                 gdouble * length, gboolean * lastwasspace,
                                 gboolean usetextonly)
{
    guint i;
    int out = FALSE;
    for (i = 0; i < self->children->len; i++) {
        RsvgNode *node = g_ptr_array_index (self->children, i);
        RsvgNodeType type = RSVG_NODE_TYPE (node);

        rsvg_state_push (ctx);
        rsvg_state_reinherit_top (ctx, node->state, 0);
        if (type == RSVG_NODE_TYPE_CHARS) {
            RsvgNodeChars *chars = (RsvgNodeChars *) node;
            GString *str = _rsvg_text_chomp (rsvg_current_state (ctx), chars->contents, lastwasspace);
            *length += rsvg_text_length_text_as_string (ctx, str->str);
            g_string_free (str, TRUE);
        } else {
            if (usetextonly) {
                out = _rsvg_node_text_length_children(node, ctx, length,
                                                      lastwasspace,
                                                      usetextonly);
            } else {
                if (type == RSVG_NODE_TYPE_TSPAN) {
                    RsvgNodeText *tspan = (RsvgNodeText *) node;
                    out = _rsvg_node_text_length_tspan (tspan, ctx, length,
                                                        lastwasspace,
                                                        usetextonly);
                } else if (type == RSVG_NODE_TYPE_TREF) {
                    RsvgNodeTref *tref = (RsvgNodeTref *) node;
                    out = _rsvg_node_text_length_tref (tref, ctx, length,
                                                       lastwasspace,
                                                       usetextonly);
                }
            }
        }
        rsvg_state_pop (ctx);
        if (out)
            break;
    }
    return out;
}


static void
_rsvg_node_text_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    double x, y, dx, dy, length = 0;
    gboolean lastwasspace = TRUE;
    RsvgNodeText *text = (RsvgNodeText *) self;
    rsvg_state_reinherit_top (ctx, self->state, dominate);

    x = _rsvg_css_normalize_length (&text->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&text->y, ctx, 'v');
    dx = _rsvg_css_normalize_length (&text->dx, ctx, 'h');
    dy = _rsvg_css_normalize_length (&text->dy, ctx, 'v');

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        _rsvg_node_text_length_children (self, ctx, &length, &lastwasspace, FALSE);
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
    _rsvg_node_text_type_children (self, ctx, &x, &y, &lastwasspace, FALSE);
}

RsvgNode *
rsvg_new_text (void)
{
    RsvgNodeText *text;
    text = g_new (RsvgNodeText, 1);
    _rsvg_node_init (&text->super, RSVG_NODE_TYPE_TEXT);
    text->super.draw = _rsvg_node_text_draw;
    text->super.set_atts = _rsvg_node_text_set_atts;
    text->x = text->y = text->dx = text->dy = _rsvg_css_parse_length ("0");
    return &text->super;
}

static void
_rsvg_node_text_type_tspan (RsvgNodeText * self, RsvgDrawingCtx * ctx,
                            gdouble * x, gdouble * y, gboolean * lastwasspace,
                            gboolean usetextonly)
{
    double dx, dy, length = 0;
    rsvg_state_reinherit_top (ctx, self->super.state, 0);

    dx = _rsvg_css_normalize_length (&self->dx, ctx, 'h');
    dy = _rsvg_css_normalize_length (&self->dy, ctx, 'v');

    if (rsvg_current_state (ctx)->text_anchor != TEXT_ANCHOR_START) {
        gboolean lws = *lastwasspace;
        _rsvg_node_text_length_children (&self->super, ctx, &length, &lws,
                                         usetextonly);
        if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
            length /= 2;
    }

    if (self->x.factor != 'n') {
        *x = _rsvg_css_normalize_length (&self->x, ctx, 'h');
        if (!PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity)) {
            *x -= length;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
                dx /= 2;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
                dx = 0;
        }
    }
    *x += dx;

    if (self->y.factor != 'n') {
        *y = _rsvg_css_normalize_length (&self->y, ctx, 'v');
        if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity)) {
            *y -= length;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
                dy /= 2;
            if (rsvg_current_state (ctx)->text_anchor == TEXT_ANCHOR_END)
                dy = 0;
        }
    }
    *y += dy;
    _rsvg_node_text_type_children (&self->super, ctx, x, y, lastwasspace,
                                   usetextonly);
}

static int
_rsvg_node_text_length_tspan (RsvgNodeText * self,
                              RsvgDrawingCtx * ctx, gdouble * length,
                              gboolean * lastwasspace, gboolean usetextonly)
{
    if (self->x.factor != 'n' || self->y.factor != 'n')
        return TRUE;

    if (PANGO_GRAVITY_IS_VERTICAL (rsvg_current_state (ctx)->text_gravity))
        *length += _rsvg_css_normalize_length (&self->dy, ctx, 'v');
    else
        *length += _rsvg_css_normalize_length (&self->dx, ctx, 'h');

    return _rsvg_node_text_length_children (&self->super, ctx, length,
                                             lastwasspace, usetextonly);
}

static void
_rsvg_node_tspan_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeText *text = (RsvgNodeText *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            text->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            text->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dx")))
            text->dx = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "dy")))
            text->dy = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, value, self);
        }

        rsvg_parse_style_attrs (ctx, self->state, "tspan", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_tspan (void)
{
    RsvgNodeText *text;
    text = g_new (RsvgNodeText, 1);
    _rsvg_node_init (&text->super, RSVG_NODE_TYPE_TSPAN);
    text->super.set_atts = _rsvg_node_tspan_set_atts;
    text->x.factor = text->y.factor = 'n';
    text->dx = text->dy = _rsvg_css_parse_length ("0");
    return &text->super;
}

static void
_rsvg_node_text_type_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx,
                           gdouble * x, gdouble * y, gboolean * lastwasspace,
                           gboolean usetextonly)
{
    if (self->link)
        _rsvg_node_text_type_children (self->link, ctx, x, y, lastwasspace,
                                                              TRUE);
}

static int
_rsvg_node_text_length_tref (RsvgNodeTref * self, RsvgDrawingCtx * ctx, gdouble * x,
                             gboolean * lastwasspace, gboolean usetextonly)
{
    if (self->link)
        return _rsvg_node_text_length_children (self->link, ctx, x, lastwasspace, TRUE);
    return FALSE;
}

static void
_rsvg_node_tref_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *value;
    RsvgNodeTref *text = (RsvgNodeTref *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
            rsvg_defs_add_resolver (ctx->priv->defs, &text->link, value);
        if ((value = rsvg_property_bag_lookup (atts, "id")))
            rsvg_defs_register_name (ctx->priv->defs, value, self);
    }
}

RsvgNode *
rsvg_new_tref (void)
{
    RsvgNodeTref *text;
    text = g_new (RsvgNodeTref, 1);
    _rsvg_node_init (&text->super, RSVG_NODE_TYPE_TREF);
    text->super.set_atts = _rsvg_node_tref_set_atts;
    text->link = NULL;
    return &text->super;
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
rsvg_text_create_layout (RsvgDrawingCtx * ctx,
                         RsvgState * state, const char *text, PangoContext * context)
{
    PangoFontDescription *font_desc;
    PangoLayout *layout;
    PangoAttrList *attr_list;
    PangoAttribute *attribute;

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
    pango_font_description_set_size (font_desc,
                                     _rsvg_css_normalize_font_size (state, ctx) *
                                     PANGO_SCALE / ctx->dpi_y * 72);

    layout = pango_layout_new (context);
    pango_layout_set_font_description (layout, font_desc);
    pango_font_description_free (font_desc);

    attr_list = pango_attr_list_new ();
    attribute = pango_attr_letter_spacing_new (_rsvg_css_normalize_length (&state->letter_spacing,
                                                                           ctx, 'h') * PANGO_SCALE);
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
rsvg_text_layout_new (RsvgDrawingCtx * ctx, RsvgState * state, const char *text)
{
    RsvgTextLayout *layout;

    if (ctx->pango_context == NULL)
        ctx->pango_context = ctx->render->create_pango_context (ctx);

    layout = g_new0 (RsvgTextLayout, 1);

    layout->layout = rsvg_text_create_layout (ctx, state, text, ctx->pango_context);
    layout->ctx = ctx;

    layout->anchor = state->text_anchor;

    return layout;
}

void
rsvg_text_render_text (RsvgDrawingCtx * ctx, const char *text, gdouble * x, gdouble * y)
{
    PangoContext *context;
    PangoLayout *layout;
    PangoLayoutIter *iter;
    RsvgState *state;
    gint w, h, offsetX, offsetY;

    state = rsvg_current_state (ctx);

    /* Do not render the text if the font size is zero. See bug #581491. */
    if (state->font_size.length == 0)
        return;

    context = ctx->render->create_pango_context (ctx);
    layout = rsvg_text_create_layout (ctx, state, text, context);
    pango_layout_get_size (layout, &w, &h);
    iter = pango_layout_get_iter (layout);
    if (PANGO_GRAVITY_IS_VERTICAL (state->text_gravity)) {
        offsetX = -pango_layout_iter_get_baseline (iter) / (double)PANGO_SCALE;
        offsetY = 0;
    } else {
        offsetX = 0;
        offsetY = pango_layout_iter_get_baseline (iter) / (double)PANGO_SCALE;
    }
    pango_layout_iter_free (iter);
    ctx->render->render_pango_layout (ctx, layout, *x - offsetX, *y - offsetY);
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

    layout = rsvg_text_layout_new (ctx, rsvg_current_state (ctx), text);
    layout->x = layout->y = 0;

    x = rsvg_text_layout_width (layout);

    rsvg_text_layout_free (layout);
    return x;
}
