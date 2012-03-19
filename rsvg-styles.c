/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-styles.c: Handle SVG styles

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
#include "config.h"

#include <string.h>
#include <math.h>

#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-filter.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"

#include <libcroco/libcroco.h>

#define RSVG_DEFAULT_FONT "Times New Roman"

enum {
  SHAPE_RENDERING_AUTO = CAIRO_ANTIALIAS_DEFAULT,
  SHAPE_RENDERING_OPTIMIZE_SPEED = CAIRO_ANTIALIAS_NONE,
  SHAPE_RENDERING_CRISP_EDGES = CAIRO_ANTIALIAS_NONE,
  SHAPE_RENDERING_GEOMETRIC_PRECISION = CAIRO_ANTIALIAS_DEFAULT
};

enum {
  TEXT_RENDERING_AUTO = CAIRO_ANTIALIAS_DEFAULT,
  TEXT_RENDERING_OPTIMIZE_SPEED = CAIRO_ANTIALIAS_NONE,
  TEXT_RENDERING_OPTIMIZE_LEGIBILITY = CAIRO_ANTIALIAS_DEFAULT,
  TEXT_RENDERING_GEOMETRIC_PRECISION = CAIRO_ANTIALIAS_DEFAULT
};

typedef struct _StyleValueData {
    gchar *value;
    gboolean important;
} StyleValueData;

/*
 * _rsvg_cairo_matrix_init_shear: Set up a shearing matrix.
 * @dst: Where to store the resulting affine transform.
 * @theta: Shear angle in degrees.
 *
 * Sets up a shearing matrix. In the standard libart coordinate system
 * and a small value for theta, || becomes \\. Horizontal lines remain
 * unchanged.
 **/
static void
_rsvg_cairo_matrix_init_shear (cairo_matrix_t *dst, double theta)
{
  cairo_matrix_init (dst, 1., 0., tan (theta * M_PI / 180.0), 1., 0., 0);
}

static StyleValueData *
style_value_data_new (const gchar *value, gboolean important)
{
    StyleValueData *ret;

    ret = g_new (StyleValueData, 1);
    ret->value = g_strdup (value);
    ret->important = important;

    return ret;
}

static void
style_value_data_free (StyleValueData *value)
{
    if (!value)
        return;
    g_free (value->value);
    g_free (value);
}

gdouble
rsvg_viewport_percentage (gdouble width, gdouble height)
{
    return sqrt (width * height);
}

gdouble
rsvg_dpi_percentage (RsvgHandle * ctx)
{
    return sqrt (ctx->priv->dpi_x * ctx->priv->dpi_y);
}

void
rsvg_state_init (RsvgState * state)
{
    memset (state, 0, sizeof (RsvgState));

    state->parent = NULL;
    cairo_matrix_init_identity (&state->affine);
    cairo_matrix_init_identity (&state->personal_affine);
    state->mask = NULL;
    state->opacity = 0xff;
    state->adobe_blend = 0;
    state->fill = rsvg_paint_server_parse (NULL, NULL, "#000", 0);
    state->fill_opacity = 0xff;
    state->stroke_opacity = 0xff;
    state->stroke_width = _rsvg_css_parse_length ("1");
    state->miter_limit = 4;
    state->cap = CAIRO_LINE_CAP_BUTT;
    state->join = CAIRO_LINE_JOIN_MITER;
    state->stop_opacity = 0xff;
    state->fill_rule = CAIRO_FILL_RULE_WINDING;
    state->clip_rule = CAIRO_FILL_RULE_WINDING;
    state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    state->comp_op = CAIRO_OPERATOR_OVER;
    state->overflow = FALSE;
    state->flood_color = 0;
    state->flood_opacity = 255;

    state->font_family = g_strdup (RSVG_DEFAULT_FONT);
    state->font_size = _rsvg_css_parse_length ("12.0");
    state->font_style = PANGO_STYLE_NORMAL;
    state->font_variant = PANGO_VARIANT_NORMAL;
    state->font_weight = PANGO_WEIGHT_NORMAL;
    state->font_stretch = PANGO_STRETCH_NORMAL;
    state->text_dir = PANGO_DIRECTION_LTR;
    state->text_gravity = PANGO_GRAVITY_SOUTH;
    state->unicode_bidi = UNICODE_BIDI_NORMAL;
    state->text_anchor = TEXT_ANCHOR_START;
    state->letter_spacing = _rsvg_css_parse_length ("0.0");
    state->visible = TRUE;
    state->cond_true = TRUE;
    state->filter = NULL;
    state->clip_path_ref = NULL;
    state->startMarker = NULL;
    state->middleMarker = NULL;
    state->endMarker = NULL;

    state->has_current_color = FALSE;
    state->has_flood_color = FALSE;
    state->has_flood_opacity = FALSE;
    state->has_fill_server = FALSE;
    state->has_fill_opacity = FALSE;
    state->has_fill_rule = FALSE;
    state->has_clip_rule = FALSE;
    state->has_stroke_server = FALSE;
    state->has_stroke_opacity = FALSE;
    state->has_stroke_width = FALSE;
    state->has_miter_limit = FALSE;
    state->has_cap = FALSE;
    state->has_join = FALSE;
    state->has_dash = FALSE;
    state->has_dashoffset = FALSE;
    state->has_visible = FALSE;
    state->has_cond = FALSE;
    state->has_stop_color = FALSE;
    state->has_stop_opacity = FALSE;
    state->has_font_size = FALSE;
    state->has_font_family = FALSE;
    state->has_lang = FALSE;
    state->has_font_style = FALSE;
    state->has_font_variant = FALSE;
    state->has_font_weight = FALSE;
    state->has_font_stretch = FALSE;
    state->has_font_decor = FALSE;
    state->has_text_dir = FALSE;
    state->has_text_gravity = FALSE;
    state->has_unicode_bidi = FALSE;
    state->has_text_anchor = FALSE;
    state->has_letter_spacing = FALSE;
    state->has_startMarker = FALSE;
    state->has_middleMarker = FALSE;
    state->has_endMarker = FALSE;
    state->has_overflow = FALSE;

    state->shape_rendering_type = SHAPE_RENDERING_AUTO;
    state->has_shape_rendering_type = FALSE;
    state->text_rendering_type = TEXT_RENDERING_AUTO;
    state->has_text_rendering_type = FALSE;

    state->styles = g_hash_table_new_full (g_str_hash, g_str_equal,
                                           g_free, (GDestroyNotify) style_value_data_free);
}

void
rsvg_state_reinit (RsvgState * state)
{
    RsvgState *parent = state->parent;
    rsvg_state_finalize (state);
    rsvg_state_init (state);
    state->parent = parent;
}

typedef int (*InheritanceFunction) (int dst, int src);

void
rsvg_state_clone (RsvgState * dst, const RsvgState * src)
{
    gint i;
    RsvgState *parent = dst->parent;

    rsvg_state_finalize (dst);

    *dst = *src;
    dst->parent = parent;
    dst->font_family = g_strdup (src->font_family);
    dst->lang = g_strdup (src->lang);
    rsvg_paint_server_ref (dst->fill);
    rsvg_paint_server_ref (dst->stroke);

    dst->styles = g_hash_table_ref (src->styles);

    if (src->dash.n_dash > 0) {
        dst->dash.dash = g_new (gdouble, src->dash.n_dash);
        for (i = 0; i < src->dash.n_dash; i++)
            dst->dash.dash[i] = src->dash.dash[i];
    }
}

/*
  This function is where all inheritance takes place. It is given a 
  base and a modifier state, as well as a function to determine
  how the base is modified and a flag as to whether things that can
  not be inherited are copied streight over, or ignored.
*/

static void
rsvg_state_inherit_run (RsvgState * dst, const RsvgState * src,
                        const InheritanceFunction function, const gboolean inherituninheritables)
{
    gint i;

    if (function (dst->has_current_color, src->has_current_color))
        dst->current_color = src->current_color;
    if (function (dst->has_flood_color, src->has_flood_color))
        dst->flood_color = src->flood_color;
    if (function (dst->has_flood_opacity, src->has_flood_opacity))
        dst->flood_opacity = src->flood_opacity;
    if (function (dst->has_fill_server, src->has_fill_server)) {
        rsvg_paint_server_ref (src->fill);
        if (dst->fill)
            rsvg_paint_server_unref (dst->fill);
        dst->fill = src->fill;
    }
    if (function (dst->has_fill_opacity, src->has_fill_opacity))
        dst->fill_opacity = src->fill_opacity;
    if (function (dst->has_fill_rule, src->has_fill_rule))
        dst->fill_rule = src->fill_rule;
    if (function (dst->has_clip_rule, src->has_clip_rule))
        dst->clip_rule = src->clip_rule;
    if (function (dst->overflow, src->overflow))
        dst->overflow = src->overflow;
    if (function (dst->has_stroke_server, src->has_stroke_server)) {
        rsvg_paint_server_ref (src->stroke);
        if (dst->stroke)
            rsvg_paint_server_unref (dst->stroke);
        dst->stroke = src->stroke;
    }
    if (function (dst->has_stroke_opacity, src->has_stroke_opacity))
        dst->stroke_opacity = src->stroke_opacity;
    if (function (dst->has_stroke_width, src->has_stroke_width))
        dst->stroke_width = src->stroke_width;
    if (function (dst->has_miter_limit, src->has_miter_limit))
        dst->miter_limit = src->miter_limit;
    if (function (dst->has_cap, src->has_cap))
        dst->cap = src->cap;
    if (function (dst->has_join, src->has_join))
        dst->join = src->join;
    if (function (dst->has_stop_color, src->has_stop_color))
        dst->stop_color = src->stop_color;
    if (function (dst->has_stop_opacity, src->has_stop_opacity))
        dst->stop_opacity = src->stop_opacity;
    if (function (dst->has_cond, src->has_cond))
        dst->cond_true = src->cond_true;
    if (function (dst->has_font_size, src->has_font_size))
        dst->font_size = src->font_size;
    if (function (dst->has_font_style, src->has_font_style))
        dst->font_style = src->font_style;
    if (function (dst->has_font_variant, src->has_font_variant))
        dst->font_variant = src->font_variant;
    if (function (dst->has_font_weight, src->has_font_weight))
        dst->font_weight = src->font_weight;
    if (function (dst->has_font_stretch, src->has_font_stretch))
        dst->font_stretch = src->font_stretch;
    if (function (dst->has_font_decor, src->has_font_decor))
        dst->font_decor = src->font_decor;
    if (function (dst->has_text_dir, src->has_text_dir))
        dst->text_dir = src->text_dir;
    if (function (dst->has_text_gravity, src->has_text_gravity))
        dst->text_gravity = src->text_gravity;
    if (function (dst->has_unicode_bidi, src->has_unicode_bidi))
        dst->unicode_bidi = src->unicode_bidi;
    if (function (dst->has_text_anchor, src->has_text_anchor))
        dst->text_anchor = src->text_anchor;
    if (function (dst->has_letter_spacing, src->has_letter_spacing))
	dst->letter_spacing = src->letter_spacing;
    if (function (dst->has_startMarker, src->has_startMarker))
        dst->startMarker = src->startMarker;
    if (function (dst->has_middleMarker, src->has_middleMarker))
        dst->middleMarker = src->middleMarker;
    if (function (dst->has_endMarker, src->has_endMarker))
        dst->endMarker = src->endMarker;
	if (function (dst->has_shape_rendering_type, src->has_shape_rendering_type))
		dst->shape_rendering_type = src->shape_rendering_type;
	if (function (dst->has_text_rendering_type, src->has_text_rendering_type))
		dst->text_rendering_type = src->text_rendering_type;

    if (function (dst->has_font_family, src->has_font_family)) {
        g_free (dst->font_family);      /* font_family is always set to something */
        dst->font_family = g_strdup (src->font_family);
    }

    if (function (dst->has_space_preserve, src->has_space_preserve))
	dst->space_preserve = src->space_preserve;

    if (function (dst->has_visible, src->has_visible))
	dst->visible = src->visible;

    if (function (dst->has_lang, src->has_lang)) {
        if (dst->has_lang)
            g_free (dst->lang);
        dst->lang = g_strdup (src->lang);
    }

    if (src->dash.n_dash > 0 && (function (dst->has_dash, src->has_dash))) {
        if (dst->has_dash)
            g_free (dst->dash.dash);

        dst->dash.dash = g_new (gdouble, src->dash.n_dash);
        dst->dash.n_dash = src->dash.n_dash;
        for (i = 0; i < src->dash.n_dash; i++)
            dst->dash.dash[i] = src->dash.dash[i];
    }

    if (function (dst->has_dashoffset, src->has_dashoffset)) {
        dst->dash.offset = src->dash.offset;
    }

    if (inherituninheritables) {
        dst->clip_path_ref = src->clip_path_ref;
        dst->mask = src->mask;
        dst->enable_background = src->enable_background;
        dst->adobe_blend = src->adobe_blend;
        dst->opacity = src->opacity;
        dst->filter = src->filter;
        dst->comp_op = src->comp_op;
    }
}

/*
  reinherit is given dst which is the top of the state stack
  and src which is the layer before in the state stack from
  which it should be inherited from 
*/

static int
reinheritfunction (int dst, int src)
{
    if (!dst)
        return 1;
    return 0;
}

void
rsvg_state_reinherit (RsvgState * dst, const RsvgState * src)
{
    rsvg_state_inherit_run (dst, src, reinheritfunction, 0);
}

/*
  dominate is given dst which is the top of the state stack
  and src which is the layer before in the state stack from
  which it should be inherited from, however if anything is
  directly specified in src (the second last layer) it will
  override anything on the top layer, this is for overrides
  in use tags 
*/

static int
dominatefunction (int dst, int src)
{
    if (!dst || src)
        return 1;
    return 0;
}

void
rsvg_state_dominate (RsvgState * dst, const RsvgState * src)
{
    rsvg_state_inherit_run (dst, src, dominatefunction, 0);
}

/* copy everything inheritable from the src to the dst */

static int
clonefunction (int dst, int src)
{
    return 1;
}

void
rsvg_state_override (RsvgState * dst, const RsvgState * src)
{
    rsvg_state_inherit_run (dst, src, clonefunction, 0);
}

/*
  put something new on the inheritance stack, dst is the top of the stack, 
  src is the state to be integrated, this is essentially the opposite of
  reinherit, because it is being given stuff to be integrated on the top, 
  rather than the context underneath.
*/

static int
inheritfunction (int dst, int src)
{
    return src;
}

void
rsvg_state_inherit (RsvgState * dst, const RsvgState * src)
{
    rsvg_state_inherit_run (dst, src, inheritfunction, 1);
}

void
rsvg_state_finalize (RsvgState * state)
{
    g_free (state->font_family);
    g_free (state->lang);
    rsvg_paint_server_unref (state->fill);
    rsvg_paint_server_unref (state->stroke);

    if (state->dash.n_dash != 0)
        g_free (state->dash.dash);

    if (state->styles) {
        g_hash_table_unref (state->styles);
        state->styles = NULL;
    }
}

/* Parse a CSS2 style argument, setting the SVG context attributes. */
static void
rsvg_parse_style_pair (RsvgHandle * ctx,
                       RsvgState * state,
                       const gchar * name,
                       const gchar * value,
                       gboolean important)
{
    StyleValueData *data;

    data = g_hash_table_lookup (state->styles, name);
    if (data && data->important && !important)
        return;

    if (name == NULL || value == NULL)
        return;

    g_hash_table_insert (state->styles,
                         (gpointer) g_strdup (name),
                         (gpointer) style_value_data_new (value, important));

    if (g_str_equal (name, "color"))
        state->current_color = rsvg_css_parse_color (value, &state->has_current_color);
    else if (g_str_equal (name, "opacity"))
        state->opacity = rsvg_css_parse_opacity (value);
    else if (g_str_equal (name, "flood-color"))
        state->flood_color = rsvg_css_parse_color (value, &state->has_flood_color);
    else if (g_str_equal (name, "flood-opacity")) {
        state->flood_opacity = rsvg_css_parse_opacity (value);
        state->has_flood_opacity = TRUE;
    } else if (g_str_equal (name, "filter"))
        state->filter = rsvg_filter_parse (ctx->priv->defs, value);
    else if (g_str_equal (name, "a:adobe-blending-mode")) {
        if (g_str_equal (value, "normal"))
            state->adobe_blend = 0;
        else if (g_str_equal (value, "multiply"))
            state->adobe_blend = 1;
        else if (g_str_equal (value, "screen"))
            state->adobe_blend = 2;
        else if (g_str_equal (value, "darken"))
            state->adobe_blend = 3;
        else if (g_str_equal (value, "lighten"))
            state->adobe_blend = 4;
        else if (g_str_equal (value, "softlight"))
            state->adobe_blend = 5;
        else if (g_str_equal (value, "hardlight"))
            state->adobe_blend = 6;
        else if (g_str_equal (value, "colordodge"))
            state->adobe_blend = 7;
        else if (g_str_equal (value, "colorburn"))
            state->adobe_blend = 8;
        else if (g_str_equal (value, "overlay"))
            state->adobe_blend = 9;
        else if (g_str_equal (value, "exclusion"))
            state->adobe_blend = 10;
        else if (g_str_equal (value, "difference"))
            state->adobe_blend = 11;
        else
            state->adobe_blend = 0;
    } else if (g_str_equal (name, "mask"))
        state->mask = rsvg_mask_parse (ctx->priv->defs, value);
    else if (g_str_equal (name, "clip-path")) {
        state->clip_path_ref = rsvg_clip_path_parse (ctx->priv->defs, value);
    } else if (g_str_equal (name, "overflow")) {
        if (!g_str_equal (value, "inherit")) {
            state->overflow = rsvg_css_parse_overflow (value, &state->has_overflow);
        }
    } else if (g_str_equal (name, "enable-background")) {
        if (g_str_equal (value, "new"))
            state->enable_background = RSVG_ENABLE_BACKGROUND_NEW;
        else
            state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    } else if (g_str_equal (name, "comp-op")) {
        if (g_str_equal (value, "clear"))
            state->comp_op = CAIRO_OPERATOR_CLEAR;
        else if (g_str_equal (value, "src"))
            state->comp_op = CAIRO_OPERATOR_SOURCE;
        else if (g_str_equal (value, "dst"))
            state->comp_op = CAIRO_OPERATOR_DEST;
        else if (g_str_equal (value, "src-over"))
            state->comp_op = CAIRO_OPERATOR_OVER;
        else if (g_str_equal (value, "dst-over"))
            state->comp_op = CAIRO_OPERATOR_DEST_OVER;
        else if (g_str_equal (value, "src-in"))
            state->comp_op = CAIRO_OPERATOR_IN;
        else if (g_str_equal (value, "dst-in"))
            state->comp_op = CAIRO_OPERATOR_DEST_IN;
        else if (g_str_equal (value, "src-out"))
            state->comp_op = CAIRO_OPERATOR_OUT;
        else if (g_str_equal (value, "dst-out"))
            state->comp_op = CAIRO_OPERATOR_DEST_OUT;
        else if (g_str_equal (value, "src-atop"))
            state->comp_op = CAIRO_OPERATOR_ATOP;
        else if (g_str_equal (value, "dst-atop"))
            state->comp_op = CAIRO_OPERATOR_DEST_ATOP;
        else if (g_str_equal (value, "xor"))
            state->comp_op = CAIRO_OPERATOR_XOR;
        else if (g_str_equal (value, "plus"))
            state->comp_op = CAIRO_OPERATOR_ADD;
        else if (g_str_equal (value, "multiply"))
            state->comp_op = CAIRO_OPERATOR_MULTIPLY;
        else if (g_str_equal (value, "screen"))
            state->comp_op = CAIRO_OPERATOR_SCREEN;
        else if (g_str_equal (value, "overlay"))
            state->comp_op = CAIRO_OPERATOR_OVERLAY;
        else if (g_str_equal (value, "darken"))
            state->comp_op = CAIRO_OPERATOR_DARKEN;
        else if (g_str_equal (value, "lighten"))
            state->comp_op = CAIRO_OPERATOR_LIGHTEN;
        else if (g_str_equal (value, "color-dodge"))
            state->comp_op = CAIRO_OPERATOR_COLOR_DODGE;
        else if (g_str_equal (value, "color-burn"))
            state->comp_op = CAIRO_OPERATOR_COLOR_BURN;
        else if (g_str_equal (value, "hard-light"))
            state->comp_op = CAIRO_OPERATOR_HARD_LIGHT;
        else if (g_str_equal (value, "soft-light"))
            state->comp_op = CAIRO_OPERATOR_SOFT_LIGHT;
        else if (g_str_equal (value, "difference"))
            state->comp_op = CAIRO_OPERATOR_DIFFERENCE;
        else if (g_str_equal (value, "exclusion"))
            state->comp_op = CAIRO_OPERATOR_EXCLUSION;
        else
            state->comp_op = CAIRO_OPERATOR_OVER;
    } else if (g_str_equal (name, "display")) {
        state->has_visible = TRUE;
        if (g_str_equal (value, "none"))
            state->visible = FALSE;
        else if (!g_str_equal (value, "inherit") != 0)
            state->visible = TRUE;
        else
            state->has_visible = FALSE;
	} else if (g_str_equal (name, "xml:space")) {
        state->has_space_preserve = TRUE;
        if (g_str_equal (value, "default"))
            state->space_preserve = FALSE;
        else if (!g_str_equal (value, "preserve") == 0)
            state->space_preserve = TRUE;
        else
            state->space_preserve = FALSE;
    } else if (g_str_equal (name, "visibility")) {
        state->has_visible = TRUE;
        if (g_str_equal (value, "visible"))
            state->visible = TRUE;
        else if (!g_str_equal (value, "inherit") != 0)
            state->visible = FALSE;     /* collapse or hidden */
        else
            state->has_visible = FALSE;
    } else if (g_str_equal (name, "fill")) {
        RsvgPaintServer *fill = state->fill;
        state->fill =
            rsvg_paint_server_parse (&state->has_fill_server, ctx->priv->defs, value, 0);
        rsvg_paint_server_unref (fill);
    } else if (g_str_equal (name, "fill-opacity")) {
        state->fill_opacity = rsvg_css_parse_opacity (value);
        state->has_fill_opacity = TRUE;
    } else if (g_str_equal (name, "fill-rule")) {
        state->has_fill_rule = TRUE;
        if (g_str_equal (value, "nonzero"))
            state->fill_rule = CAIRO_FILL_RULE_WINDING;
        else if (g_str_equal (value, "evenodd"))
            state->fill_rule = CAIRO_FILL_RULE_EVEN_ODD;
        else
            state->has_fill_rule = FALSE;
    } else if (g_str_equal (name, "clip-rule")) {
        state->has_clip_rule = TRUE;
        if (g_str_equal (value, "nonzero"))
            state->clip_rule = CAIRO_FILL_RULE_WINDING;
        else if (g_str_equal (value, "evenodd"))
            state->clip_rule = CAIRO_FILL_RULE_EVEN_ODD;
        else
            state->has_clip_rule = FALSE;
    } else if (g_str_equal (name, "stroke")) {
        RsvgPaintServer *stroke = state->stroke;

        state->stroke =
            rsvg_paint_server_parse (&state->has_stroke_server, ctx->priv->defs, value, 0);

        rsvg_paint_server_unref (stroke);
    } else if (g_str_equal (name, "stroke-width")) {
        state->stroke_width = _rsvg_css_parse_length (value);
        state->has_stroke_width = TRUE;
    } else if (g_str_equal (name, "stroke-linecap")) {
        state->has_cap = TRUE;
        if (g_str_equal (value, "butt"))
            state->cap = CAIRO_LINE_CAP_BUTT;
        else if (g_str_equal (value, "round"))
            state->cap = CAIRO_LINE_CAP_ROUND;
        else if (g_str_equal (value, "square"))
            state->cap = CAIRO_LINE_CAP_SQUARE;
        else
            g_warning (_("unknown line cap style %s\n"), value);
    } else if (g_str_equal (name, "stroke-opacity")) {
        state->stroke_opacity = rsvg_css_parse_opacity (value);
        state->has_stroke_opacity = TRUE;
    } else if (g_str_equal (name, "stroke-linejoin")) {
        state->has_join = TRUE;
        if (g_str_equal (value, "miter"))
            state->join = CAIRO_LINE_JOIN_MITER;
        else if (g_str_equal (value, "round"))
            state->join = CAIRO_LINE_JOIN_ROUND;
        else if (g_str_equal (value, "bevel"))
            state->join = CAIRO_LINE_JOIN_BEVEL;
        else
            g_warning (_("unknown line join style %s\n"), value);
    } else if (g_str_equal (name, "font-size")) {
        state->font_size = _rsvg_css_parse_length (value);
        state->has_font_size = TRUE;
    } else if (g_str_equal (name, "font-family")) {
        char *save = g_strdup (rsvg_css_parse_font_family (value, &state->has_font_family));
        g_free (state->font_family);
        state->font_family = save;
    } else if (g_str_equal (name, "xml:lang")) {
        char *save = g_strdup (value);
        g_free (state->lang);
        state->lang = save;
        state->has_lang = TRUE;
    } else if (g_str_equal (name, "font-style")) {
        state->font_style = rsvg_css_parse_font_style (value, &state->has_font_style);
    } else if (g_str_equal (name, "font-variant")) {
        state->font_variant = rsvg_css_parse_font_variant (value, &state->has_font_variant);
    } else if (g_str_equal (name, "font-weight")) {
        state->font_weight = rsvg_css_parse_font_weight (value, &state->has_font_weight);
    } else if (g_str_equal (name, "font-stretch")) {
        state->font_stretch = rsvg_css_parse_font_stretch (value, &state->has_font_stretch);
    } else if (g_str_equal (name, "text-decoration")) {
        if (g_str_equal (value, "inherit")) {
            state->has_font_decor = FALSE;
            state->font_decor = TEXT_NORMAL;
        } else {
            if (strstr (value, "underline"))
                state->font_decor |= TEXT_UNDERLINE;
            if (strstr (value, "overline"))
                state->font_decor |= TEXT_OVERLINE;
            if (strstr (value, "strike") || strstr (value, "line-through"))     /* strike though or line-through */
                state->font_decor |= TEXT_STRIKE;
            state->has_font_decor = TRUE;
        }
    } else if (g_str_equal (name, "direction")) {
        state->has_text_dir = TRUE;
        if (g_str_equal (value, "inherit")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->has_text_dir = FALSE;
        } else if (g_str_equal (value, "rtl"))
            state->text_dir = PANGO_DIRECTION_RTL;
        else                    /* ltr */
            state->text_dir = PANGO_DIRECTION_LTR;
    } else if (g_str_equal (name, "unicode-bidi")) {
        state->has_unicode_bidi = TRUE;
        if (g_str_equal (value, "inherit")) {
            state->unicode_bidi = UNICODE_BIDI_NORMAL;
            state->has_unicode_bidi = FALSE;
        } else if (g_str_equal (value, "embed"))
            state->unicode_bidi = UNICODE_BIDI_EMBED;
        else if (g_str_equal (value, "bidi-override"))
            state->unicode_bidi = UNICODE_BIDI_OVERRIDE;
        else                    /* normal */
            state->unicode_bidi = UNICODE_BIDI_NORMAL;
    } else if (g_str_equal (name, "writing-mode")) {
        /* TODO: these aren't quite right... */

        state->has_text_dir = TRUE;
        state->has_text_gravity = TRUE;
        if (g_str_equal (value, "inherit")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->has_text_dir = FALSE;
            state->text_gravity = PANGO_GRAVITY_SOUTH;
            state->has_text_gravity = FALSE;
        } else if (g_str_equal (value, "lr-tb") || g_str_equal (value, "lr")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->text_gravity = PANGO_GRAVITY_SOUTH;
        } else if (g_str_equal (value, "rl-tb") || g_str_equal (value, "rl")) {
            state->text_dir = PANGO_DIRECTION_RTL;
            state->text_gravity = PANGO_GRAVITY_SOUTH;
        } else if (g_str_equal (value, "tb-rl") || g_str_equal (value, "tb")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->text_gravity = PANGO_GRAVITY_EAST;
        }
    } else if (g_str_equal (name, "text-anchor")) {
        state->has_text_anchor = TRUE;
        if (g_str_equal (value, "inherit")) {
            state->text_anchor = TEXT_ANCHOR_START;
            state->has_text_anchor = FALSE;
        } else {
            if (strstr (value, "start"))
                state->text_anchor = TEXT_ANCHOR_START;
            else if (strstr (value, "middle"))
                state->text_anchor = TEXT_ANCHOR_MIDDLE;
            else if (strstr (value, "end"))
                state->text_anchor = TEXT_ANCHOR_END;
        }
    } else if (g_str_equal (name, "letter-spacing")) {
	state->has_letter_spacing = TRUE;
	state->letter_spacing = _rsvg_css_parse_length (value);
    } else if (g_str_equal (name, "stop-color")) {
        if (!g_str_equal (value, "inherit")) {
            state->stop_color = rsvg_css_parse_color (value, &state->has_stop_color);
        }
    } else if (g_str_equal (name, "stop-opacity")) {
        if (!g_str_equal (value, "inherit")) {
            state->has_stop_opacity = TRUE;
            state->stop_opacity = rsvg_css_parse_opacity (value);
        }
    } else if (g_str_equal (name, "marker-start")) {
        state->startMarker = rsvg_marker_parse (ctx->priv->defs, value);
        state->has_startMarker = TRUE;
    } else if (g_str_equal (name, "marker-mid")) {
        state->middleMarker = rsvg_marker_parse (ctx->priv->defs, value);
        state->has_middleMarker = TRUE;
    } else if (g_str_equal (name, "marker-end")) {
        state->endMarker = rsvg_marker_parse (ctx->priv->defs, value);
        state->has_endMarker = TRUE;
    } else if (g_str_equal (name, "stroke-miterlimit")) {
        state->has_miter_limit = TRUE;
        state->miter_limit = g_ascii_strtod (value, NULL);
    } else if (g_str_equal (name, "stroke-dashoffset")) {
        state->has_dashoffset = TRUE;
        state->dash.offset = _rsvg_css_parse_length (value);
        if (state->dash.offset.length < 0.)
            state->dash.offset.length = 0.;
    } else if (g_str_equal (name, "shape-rendering")) {
        state->has_shape_rendering_type = TRUE;

        if (g_str_equal (value, "auto") || g_str_equal (value, "default"))
            state->shape_rendering_type = SHAPE_RENDERING_AUTO;
        else if (g_str_equal (value, "optimizeSpeed"))
            state->shape_rendering_type = SHAPE_RENDERING_OPTIMIZE_SPEED;
        else if (g_str_equal (value, "crispEdges"))
            state->shape_rendering_type = SHAPE_RENDERING_CRISP_EDGES;
        else if (g_str_equal (value, "geometricPrecision"))
            state->shape_rendering_type = SHAPE_RENDERING_GEOMETRIC_PRECISION;

    } else if (g_str_equal (name, "text-rendering")) {
        state->has_text_rendering_type = TRUE;

        if (g_str_equal (value, "auto") || g_str_equal (value, "default"))
            state->text_rendering_type = TEXT_RENDERING_AUTO;
        else if (g_str_equal (value, "optimizeSpeed"))
            state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_SPEED;
        else if (g_str_equal (value, "optimizeLegibility"))
            state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_LEGIBILITY;
        else if (g_str_equal (value, "geometricPrecision"))
            state->text_rendering_type = TEXT_RENDERING_GEOMETRIC_PRECISION;

    } else if (g_str_equal (name, "stroke-dasharray")) {
        state->has_dash = TRUE;
        if (g_str_equal (value, "none")) {
            if (state->dash.n_dash != 0) {
                /* free any cloned dash data */
                g_free (state->dash.dash);
                state->dash.n_dash = 0;
            }
        } else {
            gchar **dashes = g_strsplit (value, ",", -1);
            if (NULL != dashes) {
                gint n_dashes, i;
                gboolean is_even = FALSE;
                gdouble total = 0;

                /* count the #dashes */
                for (n_dashes = 0; dashes[n_dashes] != NULL; n_dashes++);

                is_even = (n_dashes % 2 == 0);
                state->dash.n_dash = (is_even ? n_dashes : n_dashes * 2);
                state->dash.dash = g_new (double, state->dash.n_dash);

                /* TODO: handle negative value == error case */

                /* the even and base case */
                for (i = 0; i < n_dashes; i++) {
                    state->dash.dash[i] = g_ascii_strtod (dashes[i], NULL);
                    total += state->dash.dash[i];
                }
                /* if an odd number of dashes is found, it gets repeated */
                if (!is_even)
                    for (; i < state->dash.n_dash; i++)
                        state->dash.dash[i] = state->dash.dash[i - n_dashes];

                g_strfreev (dashes);
                /* If the dashes add up to 0, then it should 
                   be ignored */
                if (total == 0) {
                    g_free (state->dash.dash);
                    state->dash.n_dash = 0;
                }
            }
        }
    }
}

static void
rsvg_lookup_parse_style_pair (RsvgHandle * ctx, RsvgState * state,
                              const char *key, RsvgPropertyBag * atts)
{
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, key)) != NULL)
        rsvg_parse_style_pair (ctx, state, key, value, FALSE);
}

/* take a pair of the form (fill="#ff00ff") and parse it as a style */
void
rsvg_parse_style_pairs (RsvgHandle * ctx, RsvgState * state, RsvgPropertyBag * atts)
{
    rsvg_lookup_parse_style_pair (ctx, state, "a:adobe-blending-mode", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "clip-path", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "clip-rule", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "color", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "direction", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "display", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "enable-background", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "comp-op", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "fill", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "fill-opacity", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "fill-rule", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "filter", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "flood-color", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "flood-opacity", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-family", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-size", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-stretch", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-style", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-variant", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "font-weight", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "marker-end", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "mask", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "marker-mid", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "marker-start", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "opacity", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "overflow", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "shape-rendering", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stop-color", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stop-opacity", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-dasharray", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-dashoffset", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-linecap", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-linejoin", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-miterlimit", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-opacity", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "stroke-width", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "text-anchor", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "text-decoration", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "unicode-bidi", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "letter-spacing", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "visibility", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "writing-mode", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "xml:lang", atts);
    rsvg_lookup_parse_style_pair (ctx, state, "xml:space", atts);

    {
        /* TODO: this conditional behavior isn't quite correct, and i'm not sure it should reside here */
        gboolean cond_true, has_cond;

        cond_true = rsvg_eval_switch_attributes (atts, &has_cond);

        if (has_cond) {
            state->cond_true = cond_true;
            state->has_cond = TRUE;
        }
    }
}

static gboolean
parse_style_value (const gchar *string, gchar **value, gboolean *important)
{
    gchar **strings;

    strings = g_strsplit (string, "!", 2);

    if (strings == NULL || strings[0] == NULL) {
        g_strfreev (strings);
        return FALSE;
    }

    if (strings[1] != NULL && strings[2] == NULL &&
        g_str_equal (g_strstrip (strings[1]), "important")) {
        *important = TRUE;
    } else {
        *important = FALSE;
    }

    *value = g_strdup (g_strstrip (strings[0]));

    g_strfreev (strings);

    return TRUE;
}

/* Split a CSS2 style into individual style arguments, setting attributes
   in the SVG context.
   
   It's known that this is _way_ out of spec. A more complete CSS2
   implementation will happen later.
*/
void
rsvg_parse_style (RsvgHandle * ctx, RsvgState * state, const char *str)
{
    gchar **styles;
    guint i;

    styles = g_strsplit (str, ";", -1);
    for (i = 0; i < g_strv_length (styles); i++) {
        gchar **values;
        values = g_strsplit (styles[i], ":", 2);
        if (!values)
            continue;

        if (g_strv_length (values) == 2) {
            gboolean important;
            gchar *style_value = NULL;
            if (parse_style_value (values[1], &style_value, &important))
                rsvg_parse_style_pair (ctx, state,
                                       g_strstrip (values[0]),
                                       style_value,
                                       important);
            g_free (style_value);
        }
        g_strfreev (values);
    }
    g_strfreev (styles);
}

static void
rsvg_css_define_style (RsvgHandle * ctx,
                       const gchar * selector,
                       const gchar * style_name,
                       const gchar * style_value,
                       gboolean important)
{
    GHashTable *styles;
    gboolean need_insert = FALSE;

    /* push name/style pair into HT */
    styles = g_hash_table_lookup (ctx->priv->css_props, selector);
    if (styles == NULL) {
        styles = g_hash_table_new_full (g_str_hash, g_str_equal,
                                        g_free, (GDestroyNotify) style_value_data_free);
        g_hash_table_insert (ctx->priv->css_props, (gpointer) g_strdup (selector), styles);
        need_insert = TRUE;
    } else {
        StyleValueData *current_value;
        current_value = g_hash_table_lookup (styles, style_name);
        if (current_value == NULL || !current_value->important)
            need_insert = TRUE;
    }
    if (need_insert) {
        g_hash_table_insert (styles,
                             (gpointer) g_strdup (style_name),
                             (gpointer) style_value_data_new (style_value, important));
    }
}

typedef struct _CSSUserData {
    RsvgHandle *ctx;
    CRSelector *selector;
} CSSUserData;

static void
css_user_data_init (CSSUserData * user_data, RsvgHandle * ctx)
{
    user_data->ctx = ctx;
    user_data->selector = NULL;
}

static void
ccss_start_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;
    cr_selector_ref (a_selector_list);
    user_data->selector = a_selector_list;
}

static void
ccss_end_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    cr_selector_unref (user_data->selector);
    user_data->selector = NULL;
}

static void
ccss_property (CRDocHandler * a_handler, CRString * a_name, CRTerm * a_expr, gboolean a_important)
{
    CSSUserData *user_data;
    gchar *name = NULL;
    size_t len = 0;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    if (a_name && a_expr && user_data->selector) {
        CRSelector *cur;
        for (cur = user_data->selector; cur; cur = cur->next) {
            if (cur->simple_sel) {
                gchar *selector = (gchar *) cr_simple_sel_to_string (cur->simple_sel);
                if (selector) {
                    gchar *style_name, *style_value;
                    name = (gchar *) cr_string_peek_raw_str (a_name);
                    len = cr_string_peek_raw_str_len (a_name);
                    style_name = g_strndup (name, len);
                    style_value = (gchar *)cr_term_to_string (a_expr);
                    rsvg_css_define_style (user_data->ctx,
                                           selector,
                                           style_name,
                                           style_value,
                                           a_important);
                    g_free (selector);
                    g_free (style_name);
                    g_free (style_value);
                }
            }
        }
    }
}

static void
ccss_error (CRDocHandler * a_handler)
{
    /* yup, like i care about CSS parsing errors ;-)
       ignore, chug along */
    g_warning (_("CSS parsing error\n"));
}

static void
ccss_unrecoverable_error (CRDocHandler * a_handler)
{
    /* yup, like i care about CSS parsing errors ;-)
       ignore, chug along */
    g_warning (_("CSS unrecoverable error\n"));
}

static void
 ccss_import_style (CRDocHandler * a_this,
                    GList * a_media_list,
                    CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location);

static void
init_sac_handler (CRDocHandler * a_handler)
{
    a_handler->start_document = NULL;
    a_handler->end_document = NULL;
    a_handler->import_style = ccss_import_style;
    a_handler->namespace_declaration = NULL;
    a_handler->comment = NULL;
    a_handler->start_selector = ccss_start_selector;
    a_handler->end_selector = ccss_end_selector;
    a_handler->property = ccss_property;
    a_handler->start_font_face = NULL;
    a_handler->end_font_face = NULL;
    a_handler->start_media = NULL;
    a_handler->end_media = NULL;
    a_handler->start_page = NULL;
    a_handler->end_page = NULL;
    a_handler->ignorable_at_rule = NULL;
    a_handler->error = ccss_error;
    a_handler->unrecoverable_error = ccss_unrecoverable_error;
}

void
rsvg_parse_cssbuffer (RsvgHandle * ctx, const char *buff, size_t buflen)
{
    CRParser *parser = NULL;
    CRDocHandler *css_handler = NULL;
    CSSUserData user_data;

    if (buff == NULL || buflen == 0)
        return;

    css_handler = cr_doc_handler_new ();
    init_sac_handler (css_handler);

    css_user_data_init (&user_data, ctx);
    css_handler->app_data = &user_data;

    /* TODO: fix libcroco to take in const strings */
    parser = cr_parser_new_from_buf ((guchar *) buff, (gulong) buflen, CR_UTF_8, FALSE);
    if (parser == NULL) {
        cr_doc_handler_unref (css_handler);
        return;
    }

    cr_parser_set_sac_handler (parser, css_handler);
    cr_doc_handler_unref (css_handler);

    cr_parser_set_use_core_grammar (parser, FALSE);
    cr_parser_parse (parser);

    cr_parser_destroy (parser);
}

static void
ccss_import_style (CRDocHandler * a_this,
                   GList * a_media_list,
                   CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location)
{
    CSSUserData *user_data = (CSSUserData *) a_this->app_data;
    guint8 *stylesheet_data;
    gsize stylesheet_data_len;
    char *mime_type = NULL;

    if (a_uri == NULL)
        return;

    stylesheet_data = _rsvg_handle_acquire_data (user_data->ctx,
                                                 (gchar *) cr_string_peek_raw_str (a_uri),
                                                 &mime_type,
                                                 &stylesheet_data_len,
                                                 NULL);
    if (stylesheet_data == NULL || 
        mime_type == NULL || 
        strcmp (mime_type, "text/css") != 0) {
        g_free (stylesheet_data);
        g_free (mime_type);
        return;
    }

    rsvg_parse_cssbuffer (user_data->ctx, (const char *) stylesheet_data,
                          stylesheet_data_len);
    g_free (stylesheet_data);
    g_free (mime_type);
}

/* Parse an SVG transform string into an affine matrix. Reference: SVG
   working draft dated 1999-07-06, section 8.5. Return TRUE on
   success. */
gboolean
rsvg_parse_transform (cairo_matrix_t *dst, const char *src)
{
    int idx;
    char keyword[32];
    double args[6];
    int n_args;
    guint key_len;
    cairo_matrix_t affine;

    cairo_matrix_init_identity (dst);

    idx = 0;
    while (src[idx]) {
        /* skip initial whitespace */
        while (g_ascii_isspace (src[idx]))
            idx++;

        if (src[idx] == '\0')
            break;

        /* parse keyword */
        for (key_len = 0; key_len < sizeof (keyword); key_len++) {
            char c;

            c = src[idx];
            if (g_ascii_isalpha (c) || c == '-')
                keyword[key_len] = src[idx++];
            else
                break;
        }
        if (key_len >= sizeof (keyword))
            return FALSE;
        keyword[key_len] = '\0';

        /* skip whitespace */
        while (g_ascii_isspace (src[idx]))
            idx++;

        if (src[idx] != '(')
            return FALSE;
        idx++;

        for (n_args = 0;; n_args++) {
            char c;
            char *end_ptr;

            /* skip whitespace */
            while (g_ascii_isspace (src[idx]))
                idx++;
            c = src[idx];
            if (g_ascii_isdigit (c) || c == '+' || c == '-' || c == '.') {
                if (n_args == sizeof (args) / sizeof (args[0]))
                    return FALSE;       /* too many args */
                args[n_args] = g_ascii_strtod (src + idx, &end_ptr);
                idx = end_ptr - src;

                while (g_ascii_isspace (src[idx]))
                    idx++;

                /* skip optional comma */
                if (src[idx] == ',')
                    idx++;
            } else if (c == ')')
                break;
            else
                return FALSE;
        }
        idx++;

        /* ok, have parsed keyword and args, now modify the transform */
        if (!strcmp (keyword, "matrix")) {
            if (n_args != 6)
                return FALSE;

            cairo_matrix_init (&affine, args[0], args[1], args[2], args[3], args[4], args[5]);
            cairo_matrix_multiply (dst, &affine, dst);
        } else if (!strcmp (keyword, "translate")) {
            if (n_args == 1)
                args[1] = 0;
            else if (n_args != 2)
                return FALSE;
            cairo_matrix_init_translate (&affine, args[0], args[1]);
            cairo_matrix_multiply (dst, &affine, dst);
        } else if (!strcmp (keyword, "scale")) {
            if (n_args == 1)
                args[1] = args[0];
            else if (n_args != 2)
                return FALSE;
            cairo_matrix_init_scale (&affine, args[0], args[1]);
            cairo_matrix_multiply (dst, &affine, dst);
        } else if (!strcmp (keyword, "rotate")) {
            if (n_args == 1) {

                cairo_matrix_init_rotate (&affine, args[0] * M_PI / 180.);
                cairo_matrix_multiply (dst, &affine, dst);
            } else if (n_args == 3) {
                cairo_matrix_init_translate (&affine, args[1], args[2]);
                cairo_matrix_multiply (dst, &affine, dst);

                cairo_matrix_init_rotate (&affine, args[0] * M_PI / 180.);
                cairo_matrix_multiply (dst, &affine, dst);

                cairo_matrix_init_translate (&affine, -args[1], -args[2]);
                cairo_matrix_multiply (dst, &affine, dst);
            } else
                return FALSE;
        } else if (!strcmp (keyword, "skewX")) {
            if (n_args != 1)
                return FALSE;
            _rsvg_cairo_matrix_init_shear (&affine, args[0]);
            cairo_matrix_multiply (dst, &affine, dst);
        } else if (!strcmp (keyword, "skewY")) {
            if (n_args != 1)
                return FALSE;
            _rsvg_cairo_matrix_init_shear (&affine, args[0]);
            /* transpose the affine, given that we know [1] is zero */
            affine.yx = affine.xy;
            affine.xy = 0.;
            cairo_matrix_multiply (dst, &affine, dst);
        } else
            return FALSE;       /* unknown keyword */
    }
    return TRUE;
}

/**
 * rsvg_parse_transform_attr: Parse transform attribute and apply to state.
 * @ctx: Rsvg context.
 * @state: State in which to apply the transform.
 * @str: String containing transform.
 *
 * Parses the transform attribute in @str and applies it to @state.
 **/
static void
rsvg_parse_transform_attr (RsvgHandle * ctx, RsvgState * state, const char *str)
{
    cairo_matrix_t affine;

    if (rsvg_parse_transform (&affine, str)) {
        cairo_matrix_multiply (&state->personal_affine, &affine, &state->personal_affine);
        cairo_matrix_multiply (&state->affine, &affine, &state->affine);
    }
}

typedef struct _StylesData {
    RsvgHandle *ctx;
    RsvgState *state;
} StylesData;

static void
apply_style (const gchar *key, StyleValueData *value, gpointer user_data)
{
    StylesData *data = (StylesData *) user_data;
    rsvg_parse_style_pair (data->ctx, data->state, key, value->value, value->important);
}

static gboolean
rsvg_lookup_apply_css_style (RsvgHandle * ctx, const char *target, RsvgState * state)
{
    GHashTable *styles;

    styles = g_hash_table_lookup (ctx->priv->css_props, target);

    if (styles != NULL) {
        StylesData *data = g_new (StylesData, 1);
        data->ctx = ctx;
        data->state = state;
        g_hash_table_foreach (styles, (GHFunc) apply_style, data);
        g_free (data);
        return TRUE;
    }
    return FALSE;
}

/**
 * rsvg_parse_style_attrs: Parse style attribute.
 * @ctx: Rsvg context.
 * @state: Rsvg state
 * @tag: The SVG tag we're processing (eg: circle, ellipse), optionally %NULL
 * @klazz: The space delimited class list, optionally %NULL
 * @atts: Attributes in SAX style.
 *
 * Parses style and transform attributes and modifies state at top of
 * stack.
 **/
void
rsvg_parse_style_attrs (RsvgHandle * ctx,
                        RsvgState * state,
                        const char *tag, const char *klazz, const char *id, RsvgPropertyBag * atts)
{
    int i = 0, j = 0;
    char *target = NULL;
    gboolean found = FALSE;
    GString *klazz_list = NULL;

    if (rsvg_property_bag_size (atts) > 0)
        rsvg_parse_style_pairs (ctx, state, atts);

    /* Try to properly support all of the following, including inheritance:
     * *
     * #id
     * tag
     * tag#id
     * tag.class
     * tag.class#id
     *
     * This is basically a semi-compliant CSS2 selection engine
     */

    /* * */
    rsvg_lookup_apply_css_style (ctx, "*", state);

    /* tag */
    if (tag != NULL) {
        rsvg_lookup_apply_css_style (ctx, tag, state);
    }

    if (klazz != NULL) {
        i = strlen (klazz);
        while (j < i) {
            found = FALSE;
            klazz_list = g_string_new (".");

            while (j < i && g_ascii_isspace (klazz[j]))
                j++;

            while (j < i && !g_ascii_isspace (klazz[j]))
                g_string_append_c (klazz_list, klazz[j++]);

            /* tag.class#id */
            if (tag != NULL && klazz_list->len != 1 && id != NULL) {
                target = g_strdup_printf ("%s%s#%s", tag, klazz_list->str, id);
                found = found || rsvg_lookup_apply_css_style (ctx, target, state);
                g_free (target);
            }

            /* class#id */
            if (klazz_list->len != 1 && id != NULL) {
                target = g_strdup_printf ("%s#%s", klazz_list->str, id);
                found = found || rsvg_lookup_apply_css_style (ctx, target, state);
                g_free (target);
            }

            /* tag.class */
            if (tag != NULL && klazz_list->len != 1) {
                target = g_strdup_printf ("%s%s", tag, klazz_list->str);
                found = found || rsvg_lookup_apply_css_style (ctx, target, state);
                g_free (target);
            }

            /* didn't find anything more specific, just apply the class style */
            if (!found) {
                found = found || rsvg_lookup_apply_css_style (ctx, klazz_list->str, state);
            }
            g_string_free (klazz_list, TRUE);
        }
    }

    /* #id */
    if (id != NULL) {
        target = g_strdup_printf ("#%s", id);
        rsvg_lookup_apply_css_style (ctx, target, state);
        g_free (target);
    }

    /* tag#id */
    if (tag != NULL && id != NULL) {
        target = g_strdup_printf ("%s#%s", tag, id);
        rsvg_lookup_apply_css_style (ctx, target, state);
        g_free (target);
    }

    if (rsvg_property_bag_size (atts) > 0) {
        const char *value;

        if ((value = rsvg_property_bag_lookup (atts, "style")) != NULL)
            rsvg_parse_style (ctx, state, value);
        if ((value = rsvg_property_bag_lookup (atts, "transform")) != NULL)
            rsvg_parse_transform_attr (ctx, state, value);
    }
}

RsvgState *
rsvg_current_state (RsvgDrawingCtx * ctx)
{
    return ctx->state;
}

RsvgState *
rsvg_state_parent (RsvgState * state)
{
    return state->parent;
}

void
rsvg_state_free_all (RsvgState * state)
{
    while (state) {
        RsvgState *parent = state->parent;
        rsvg_state_finalize (state);
        g_slice_free (RsvgState, state);
        state = parent;
    }
}

/**
 * rsvg_property_bag_new:
 * @atts:
 * 
 * The property bag will NOT copy the attributes and values. If you need
 * to store them for later, use rsvg_property_bag_dup().
 * 
 * Returns: (transfer full): a new property bag
 */
RsvgPropertyBag *
rsvg_property_bag_new (const char **atts)
{
    RsvgPropertyBag *bag;
    int i;

    bag = g_hash_table_new (g_str_hash, g_str_equal);

    if (atts != NULL) {
        for (i = 0; atts[i] != NULL; i += 2)
            g_hash_table_insert (bag, (gpointer) atts[i], (gpointer) atts[i + 1]);
    }

    return bag;
}

/**
 * rsvg_property_bag_dup:
 * @bag:
 * 
 * Returns a copy of @bag that owns the attributes and values.
 * 
 * Returns: (transfer full): a new property bag
 */
RsvgPropertyBag *
rsvg_property_bag_dup (RsvgPropertyBag * bag)
{
    RsvgPropertyBag *dup;
    GHashTableIter iter;
    gpointer key, value;

    dup = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, g_free);

    g_hash_table_iter_init (&iter, bag);
    while (g_hash_table_iter_next (&iter, &key, &value))
      g_hash_table_insert (dup, 
                           (gpointer) g_strdup ((char *) key),
                           (gpointer) g_strdup ((char *) value));

    return dup;
}

void
rsvg_property_bag_free (RsvgPropertyBag * bag)
{
    g_hash_table_unref (bag);
}

const char *
rsvg_property_bag_lookup (RsvgPropertyBag * bag, const char *key)
{
    return (const char *) g_hash_table_lookup (bag, (gconstpointer) key);
}

guint
rsvg_property_bag_size (RsvgPropertyBag * bag)
{
    return g_hash_table_size (bag);
}

void
rsvg_property_bag_enumerate (RsvgPropertyBag * bag, RsvgPropertyBagEnumFunc func,
                             gpointer user_data)
{
    g_hash_table_foreach (bag, (GHFunc) func, user_data);
}

void
rsvg_state_push (RsvgDrawingCtx * ctx)
{
    RsvgState *data;
    RsvgState *baseon;

    baseon = ctx->state;
    data = g_slice_new (RsvgState);
    rsvg_state_init (data);

    if (baseon) {
        rsvg_state_reinherit (data, baseon);
        data->affine = baseon->affine;
        data->parent = baseon;
    }

    ctx->state = data;
}

void
rsvg_state_pop (RsvgDrawingCtx * ctx)
{
    RsvgState *dead_state = ctx->state;
    ctx->state = dead_state->parent;
    rsvg_state_finalize (dead_state);
    g_slice_free (RsvgState, dead_state);
}

/*
  A function for modifying the top of the state stack depending on a 
  flag given. If that flag is 0, style and transform will inherit 
  normally. If that flag is 1, style will inherit normally with the
  exception that any value explicity set on the second last level
  will have a higher precedence than values set on the last level.
  If the flag equals two then the style will be overridden totally
  however the transform will be left as is. This is because of 
  patterns which are not based on the context of their use and are 
  rather based wholly on their own loading context. Other things
  may want to have this totally disabled, and a value of three will
  achieve this.
*/

void
rsvg_state_reinherit_top (RsvgDrawingCtx * ctx, RsvgState * state, int dominate)
{
    RsvgState *current;

    if (dominate == 3)
        return;

    current = rsvg_current_state (ctx);
    /*This is a special domination mode for patterns, the transform
       is simply left as is, wheras the style is totally overridden */
    if (dominate == 2) {
        rsvg_state_override (current, state);
    } else {
        RsvgState *parent= rsvg_state_parent (current);
        rsvg_state_clone (current, state);
        if (parent) {
            if (dominate)
                rsvg_state_dominate (current, parent);
            else
                rsvg_state_reinherit (current, parent);
            cairo_matrix_multiply (&current->affine,
                                   &current->affine,
                                   &parent->affine);
        }
    }
}

void
rsvg_state_reconstruct (RsvgState * state, RsvgNode * current)
{
    if (current == NULL)
        return;
    rsvg_state_reconstruct (state, current->parent);
    rsvg_state_inherit (state, current->state);
}
