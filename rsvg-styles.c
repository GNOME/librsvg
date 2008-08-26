/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

#define RSVG_DEFAULT_FONT "Times New Roman"

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

    _rsvg_affine_identity (state->affine);
    _rsvg_affine_identity (state->personal_affine);
    state->mask = NULL;
    state->opacity = 0xff;
    state->adobe_blend = 0;
    state->fill = rsvg_paint_server_parse (NULL, NULL, "#000", 0);
    state->fill_opacity = 0xff;
    state->stroke_opacity = 0xff;
    state->stroke_width = _rsvg_css_parse_length ("1");
    state->miter_limit = 4;
    state->cap = RSVG_PATH_STROKE_CAP_BUTT;
    state->join = RSVG_PATH_STROKE_JOIN_MITER;
    state->stop_opacity = 0xff;
    state->fill_rule = FILL_RULE_NONZERO;
    state->clip_rule = FILL_RULE_NONZERO;
    state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    state->comp_op = RSVG_COMP_OP_SRC_OVER;
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
}

typedef int (*InheritanceFunction) (int dst, int src);

void
rsvg_state_clone (RsvgState * dst, const RsvgState * src)
{
    gint i;

    rsvg_state_finalize (dst);

    *dst = *src;
    dst->font_family = g_strdup (src->font_family);
    dst->lang = g_strdup (src->lang);
    rsvg_paint_server_ref (dst->fill);
    rsvg_paint_server_ref (dst->stroke);

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
}

/* Parse a CSS2 style argument, setting the SVG context attributes. */
static void
rsvg_parse_style_arg (RsvgHandle * ctx, RsvgState * state, const char *str)
{
    int arg_off;

    arg_off = rsvg_css_param_arg_offset (str);

    if (rsvg_css_param_match (str, "color"))
        state->current_color = rsvg_css_parse_color (str + arg_off, &state->has_current_color);
    else if (rsvg_css_param_match (str, "opacity"))
        state->opacity = rsvg_css_parse_opacity (str + arg_off);
    else if (rsvg_css_param_match (str, "flood-color"))
        state->flood_color = rsvg_css_parse_color (str + arg_off, &state->has_flood_color);
    else if (rsvg_css_param_match (str, "flood-opacity")) {
        state->flood_opacity = rsvg_css_parse_opacity (str + arg_off);
        state->has_flood_opacity = TRUE;
    } else if (rsvg_css_param_match (str, "filter"))
        state->filter = rsvg_filter_parse (ctx->priv->defs, str + arg_off);
    else if (rsvg_css_param_match (str, "a:adobe-blending-mode")) {
        if (!strcmp (str + arg_off, "normal"))
            state->adobe_blend = 0;
        else if (!strcmp (str + arg_off, "multiply"))
            state->adobe_blend = 1;
        else if (!strcmp (str + arg_off, "screen"))
            state->adobe_blend = 2;
        else if (!strcmp (str + arg_off, "darken"))
            state->adobe_blend = 3;
        else if (!strcmp (str + arg_off, "lighten"))
            state->adobe_blend = 4;
        else if (!strcmp (str + arg_off, "softlight"))
            state->adobe_blend = 5;
        else if (!strcmp (str + arg_off, "hardlight"))
            state->adobe_blend = 6;
        else if (!strcmp (str + arg_off, "colordodge"))
            state->adobe_blend = 7;
        else if (!strcmp (str + arg_off, "colorburn"))
            state->adobe_blend = 8;
        else if (!strcmp (str + arg_off, "overlay"))
            state->adobe_blend = 9;
        else if (!strcmp (str + arg_off, "exclusion"))
            state->adobe_blend = 10;
        else if (!strcmp (str + arg_off, "difference"))
            state->adobe_blend = 11;
        else
            state->adobe_blend = 0;
    } else if (rsvg_css_param_match (str, "mask"))
        state->mask = rsvg_mask_parse (ctx->priv->defs, str + arg_off);
    else if (rsvg_css_param_match (str, "clip-path")) {
        state->clip_path_ref = rsvg_clip_path_parse (ctx->priv->defs, str + arg_off);
    } else if (rsvg_css_param_match (str, "overflow")) {
        if (strcmp (str + arg_off, "inherit")) {
            state->overflow = rsvg_css_parse_overflow (str + arg_off, &state->has_overflow);
        }
    } else if (rsvg_css_param_match (str, "enable-background")) {
        if (!strcmp (str + arg_off, "new"))
            state->enable_background = RSVG_ENABLE_BACKGROUND_NEW;
        else
            state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    } else if (rsvg_css_param_match (str, "comp-op")) {
        if (!strcmp (str + arg_off, "clear"))
            state->comp_op = RSVG_COMP_OP_CLEAR;
        else if (!strcmp (str + arg_off, "src"))
            state->comp_op = RSVG_COMP_OP_SRC;
        else if (!strcmp (str + arg_off, "dst"))
            state->comp_op = RSVG_COMP_OP_DST;
        else if (!strcmp (str + arg_off, "src-over"))
            state->comp_op = RSVG_COMP_OP_SRC_OVER;
        else if (!strcmp (str + arg_off, "dst-over"))
            state->comp_op = RSVG_COMP_OP_DST_OVER;
        else if (!strcmp (str + arg_off, "src-in"))
            state->comp_op = RSVG_COMP_OP_SRC_IN;
        else if (!strcmp (str + arg_off, "dst-in"))
            state->comp_op = RSVG_COMP_OP_DST_IN;
        else if (!strcmp (str + arg_off, "src-out"))
            state->comp_op = RSVG_COMP_OP_SRC_OUT;
        else if (!strcmp (str + arg_off, "dst-out"))
            state->comp_op = RSVG_COMP_OP_DST_OUT;
        else if (!strcmp (str + arg_off, "src-atop"))
            state->comp_op = RSVG_COMP_OP_SRC_ATOP;
        else if (!strcmp (str + arg_off, "dst-atop"))
            state->comp_op = RSVG_COMP_OP_DST_ATOP;
        else if (!strcmp (str + arg_off, "xor"))
            state->comp_op = RSVG_COMP_OP_XOR;
        else if (!strcmp (str + arg_off, "plus"))
            state->comp_op = RSVG_COMP_OP_PLUS;
        else if (!strcmp (str + arg_off, "multiply"))
            state->comp_op = RSVG_COMP_OP_MULTIPLY;
        else if (!strcmp (str + arg_off, "screen"))
            state->comp_op = RSVG_COMP_OP_SCREEN;
        else if (!strcmp (str + arg_off, "overlay"))
            state->comp_op = RSVG_COMP_OP_OVERLAY;
        else if (!strcmp (str + arg_off, "darken"))
            state->comp_op = RSVG_COMP_OP_DARKEN;
        else if (!strcmp (str + arg_off, "lighten"))
            state->comp_op = RSVG_COMP_OP_LIGHTEN;
        else if (!strcmp (str + arg_off, "color-dodge"))
            state->comp_op = RSVG_COMP_OP_COLOR_DODGE;
        else if (!strcmp (str + arg_off, "color-burn"))
            state->comp_op = RSVG_COMP_OP_COLOR_BURN;
        else if (!strcmp (str + arg_off, "hard-light"))
            state->comp_op = RSVG_COMP_OP_HARD_LIGHT;
        else if (!strcmp (str + arg_off, "soft-light"))
            state->comp_op = RSVG_COMP_OP_SOFT_LIGHT;
        else if (!strcmp (str + arg_off, "difference"))
            state->comp_op = RSVG_COMP_OP_DIFFERENCE;
        else if (!strcmp (str + arg_off, "exclusion"))
            state->comp_op = RSVG_COMP_OP_EXCLUSION;
        else
            state->comp_op = RSVG_COMP_OP_SRC_OVER;
    } else if (rsvg_css_param_match (str, "display")) {
        state->has_visible = TRUE;
        if (!strcmp (str + arg_off, "none"))
            state->visible = FALSE;
        else if (strcmp (str + arg_off, "inherit") != 0)
            state->visible = TRUE;
        else
            state->has_visible = FALSE;
	} else if (rsvg_css_param_match (str, "xml:space")) {
        state->has_space_preserve = TRUE;
        if (!strcmp (str + arg_off, "default"))
            state->space_preserve = FALSE;
        else if (strcmp (str + arg_off, "preserve") == 0)
            state->space_preserve = TRUE;
        else
            state->space_preserve = FALSE;
    } else if (rsvg_css_param_match (str, "visibility")) {
        state->has_visible = TRUE;
        if (!strcmp (str + arg_off, "visible"))
            state->visible = TRUE;
        else if (strcmp (str + arg_off, "inherit") != 0)
            state->visible = FALSE;     /* collapse or hidden */
        else
            state->has_visible = FALSE;
    } else if (rsvg_css_param_match (str, "fill")) {
        RsvgPaintServer *fill = state->fill;
        state->fill =
            rsvg_paint_server_parse (&state->has_fill_server, ctx->priv->defs, str + arg_off, 0);
        rsvg_paint_server_unref (fill);
    } else if (rsvg_css_param_match (str, "fill-opacity")) {
        state->fill_opacity = rsvg_css_parse_opacity (str + arg_off);
        state->has_fill_opacity = TRUE;
    } else if (rsvg_css_param_match (str, "fill-rule")) {
        state->has_fill_rule = TRUE;
        if (!strcmp (str + arg_off, "nonzero"))
            state->fill_rule = FILL_RULE_NONZERO;
        else if (!strcmp (str + arg_off, "evenodd"))
            state->fill_rule = FILL_RULE_EVENODD;
        else
            state->has_fill_rule = FALSE;
    } else if (rsvg_css_param_match (str, "clip-rule")) {
        state->has_clip_rule = TRUE;
        if (!strcmp (str + arg_off, "nonzero"))
            state->clip_rule = FILL_RULE_NONZERO;
        else if (!strcmp (str + arg_off, "evenodd"))
            state->clip_rule = FILL_RULE_EVENODD;
        else
            state->has_clip_rule = FALSE;
    } else if (rsvg_css_param_match (str, "stroke")) {
        RsvgPaintServer *stroke = state->stroke;

        state->stroke =
            rsvg_paint_server_parse (&state->has_stroke_server, ctx->priv->defs, str + arg_off, 0);

        rsvg_paint_server_unref (stroke);
    } else if (rsvg_css_param_match (str, "stroke-width")) {
        state->stroke_width = _rsvg_css_parse_length (str + arg_off);
        state->has_stroke_width = TRUE;
    } else if (rsvg_css_param_match (str, "stroke-linecap")) {
        state->has_cap = TRUE;
        if (!strcmp (str + arg_off, "butt"))
            state->cap = RSVG_PATH_STROKE_CAP_BUTT;
        else if (!strcmp (str + arg_off, "round"))
            state->cap = RSVG_PATH_STROKE_CAP_ROUND;
        else if (!strcmp (str + arg_off, "square"))
            state->cap = RSVG_PATH_STROKE_CAP_SQUARE;
        else
            g_warning (_("unknown line cap style %s\n"), str + arg_off);
    } else if (rsvg_css_param_match (str, "stroke-opacity")) {
        state->stroke_opacity = rsvg_css_parse_opacity (str + arg_off);
        state->has_stroke_opacity = TRUE;
    } else if (rsvg_css_param_match (str, "stroke-linejoin")) {
        state->has_join = TRUE;
        if (!strcmp (str + arg_off, "miter"))
            state->join = RSVG_PATH_STROKE_JOIN_MITER;
        else if (!strcmp (str + arg_off, "round"))
            state->join = RSVG_PATH_STROKE_JOIN_ROUND;
        else if (!strcmp (str + arg_off, "bevel"))
            state->join = RSVG_PATH_STROKE_JOIN_BEVEL;
        else
            g_warning (_("unknown line join style %s\n"), str + arg_off);
    } else if (rsvg_css_param_match (str, "font-size")) {
        state->font_size = _rsvg_css_parse_length (str + arg_off);
        state->has_font_size = TRUE;
    } else if (rsvg_css_param_match (str, "font-family")) {
        char *save = g_strdup (rsvg_css_parse_font_family (str + arg_off, &state->has_font_family));
        g_free (state->font_family);
        state->font_family = save;
    } else if (rsvg_css_param_match (str, "xml:lang")) {
        char *save = g_strdup (str + arg_off);
        g_free (state->lang);
        state->lang = save;
        state->has_lang = TRUE;
    } else if (rsvg_css_param_match (str, "font-style")) {
        state->font_style = rsvg_css_parse_font_style (str + arg_off, &state->has_font_style);
    } else if (rsvg_css_param_match (str, "font-variant")) {
        state->font_variant = rsvg_css_parse_font_variant (str + arg_off, &state->has_font_variant);
    } else if (rsvg_css_param_match (str, "font-weight")) {
        state->font_weight = rsvg_css_parse_font_weight (str + arg_off, &state->has_font_weight);
    } else if (rsvg_css_param_match (str, "font-stretch")) {
        state->font_stretch = rsvg_css_parse_font_stretch (str + arg_off, &state->has_font_stretch);
    } else if (rsvg_css_param_match (str, "text-decoration")) {
        if (!strcmp (str + arg_off, "inherit")) {
            state->has_font_decor = FALSE;
            state->font_decor = TEXT_NORMAL;
        } else {
            if (strstr (str + arg_off, "underline"))
                state->font_decor |= TEXT_UNDERLINE;
            if (strstr (str + arg_off, "overline"))
                state->font_decor |= TEXT_OVERLINE;
            if (strstr (str + arg_off, "strike") || strstr (str + arg_off, "line-through"))     /* strike though or line-through */
                state->font_decor |= TEXT_STRIKE;
            state->has_font_decor = TRUE;
        }
    } else if (rsvg_css_param_match (str, "direction")) {
        state->has_text_dir = TRUE;
        if (!strcmp (str + arg_off, "inherit")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->has_text_dir = FALSE;
        } else if (!strcmp (str + arg_off, "rtl"))
            state->text_dir = PANGO_DIRECTION_RTL;
        else                    /* ltr */
            state->text_dir = PANGO_DIRECTION_LTR;
    } else if (rsvg_css_param_match (str, "unicode-bidi")) {
        state->has_unicode_bidi = TRUE;
        if (!strcmp (str + arg_off, "inherit")) {
            state->unicode_bidi = PANGO_DIRECTION_LTR;
            state->has_unicode_bidi = FALSE;
        } else if (!strcmp (str + arg_off, "embed"))
            state->unicode_bidi = UNICODE_BIDI_EMBED;
        else if (!strcmp (str + arg_off, "bidi-override"))
            state->unicode_bidi = UNICODE_BIDI_OVERRIDE;
        else                    /* normal */
            state->unicode_bidi = UNICODE_BIDI_NORMAL;
    } else if (rsvg_css_param_match (str, "writing-mode")) {
        /* TODO: these aren't quite right... */

        state->has_text_dir = TRUE;
        if (!strcmp (str + arg_off, "inherit")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->has_text_dir = FALSE;
        } else if (!strcmp (str + arg_off, "lr-tb") || !strcmp (str + arg_off, "tb"))
            state->text_dir = PANGO_DIRECTION_TTB_LTR;
        else if (!strcmp (str + arg_off, "rl"))
            state->text_dir = PANGO_DIRECTION_RTL;
        else if (!strcmp (str + arg_off, "tb-rl") || !strcmp (str + arg_off, "rl-tb"))
            state->text_dir = PANGO_DIRECTION_TTB_RTL;
        else
            state->text_dir = PANGO_DIRECTION_LTR;
    } else if (rsvg_css_param_match (str, "text-anchor")) {
        state->has_text_anchor = TRUE;
        if (!strcmp (str + arg_off, "inherit")) {
            state->text_anchor = TEXT_ANCHOR_START;
            state->has_text_anchor = FALSE;
        } else {
            if (strstr (str + arg_off, "start"))
                state->text_anchor = TEXT_ANCHOR_START;
            else if (strstr (str + arg_off, "middle"))
                state->text_anchor = TEXT_ANCHOR_MIDDLE;
            else if (strstr (str + arg_off, "end"))
                state->text_anchor = TEXT_ANCHOR_END;
        }
    } else if (rsvg_css_param_match (str, "letter-spacing")) {
	state->has_letter_spacing = TRUE;
	state->letter_spacing = _rsvg_css_parse_length (str + arg_off);
    } else if (rsvg_css_param_match (str, "stop-color")) {
        if (strcmp (str + arg_off, "inherit")) {
            state->stop_color = rsvg_css_parse_color (str + arg_off, &state->has_stop_color);
        }
    } else if (rsvg_css_param_match (str, "stop-opacity")) {
        if (strcmp (str + arg_off, "inherit")) {
            state->has_stop_opacity = TRUE;
            state->stop_opacity = rsvg_css_parse_opacity (str + arg_off);
        }
    } else if (rsvg_css_param_match (str, "marker-start")) {
        state->startMarker = rsvg_marker_parse (ctx->priv->defs, str + arg_off);
        state->has_startMarker = TRUE;
    } else if (rsvg_css_param_match (str, "marker-mid")) {
        state->middleMarker = rsvg_marker_parse (ctx->priv->defs, str + arg_off);
        state->has_middleMarker = TRUE;
    } else if (rsvg_css_param_match (str, "marker-end")) {
        state->endMarker = rsvg_marker_parse (ctx->priv->defs, str + arg_off);
        state->has_endMarker = TRUE;
    } else if (rsvg_css_param_match (str, "stroke-miterlimit")) {
        state->has_miter_limit = TRUE;
        state->miter_limit = g_ascii_strtod (str + arg_off, NULL);
    } else if (rsvg_css_param_match (str, "stroke-dashoffset")) {
        state->has_dashoffset = TRUE;
        state->dash.offset = _rsvg_css_parse_length (str + arg_off);
        if (state->dash.offset.length < 0.)
            state->dash.offset.length = 0.;
	} else if (rsvg_css_param_match (str, "shape-rendering")) {
		state->has_shape_rendering_type = TRUE;

        if (!strcmp (str + arg_off, "auto") || !strcmp (str + arg_off, "default"))
			state->shape_rendering_type = SHAPE_RENDERING_AUTO;
        else if (!strcmp (str + arg_off, "optimizeSpeed"))
			state->shape_rendering_type = SHAPE_RENDERING_OPTIMIZE_SPEED;
        else if (!strcmp (str + arg_off, "crispEdges"))
			state->shape_rendering_type = SHAPE_RENDERING_CRISP_EDGES;
        else if (!strcmp (str + arg_off, "geometricPrecision"))
			state->shape_rendering_type = SHAPE_RENDERING_GEOMETRIC_PRECISION;

	} else if (rsvg_css_param_match (str, "text-rendering")) {
		state->has_text_rendering_type = TRUE;

        if (!strcmp (str + arg_off, "auto") || !strcmp (str + arg_off, "default"))
			state->text_rendering_type = TEXT_RENDERING_AUTO;
        else if (!strcmp (str + arg_off, "optimizeSpeed"))
			state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_SPEED;
        else if (!strcmp (str + arg_off, "optimizeLegibility"))
			state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_LEGIBILITY;
        else if (!strcmp (str + arg_off, "geometricPrecision"))
			state->text_rendering_type = TEXT_RENDERING_GEOMETRIC_PRECISION;

    } else if (rsvg_css_param_match (str, "stroke-dasharray")) {
        state->has_dash = TRUE;
        if (!strcmp (str + arg_off, "none")) {
            if (state->dash.n_dash != 0) {
                /* free any cloned dash data */
                g_free (state->dash.dash);
                state->dash.n_dash = 0;
            }
        } else {
            gchar **dashes = g_strsplit (str + arg_off, ",", -1);
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

void
rsvg_parse_style_pair (RsvgHandle * ctx, RsvgState * state, const char *key, const char *val)
{
    gchar *str = g_strdup_printf ("%s:%s", key, val);
    rsvg_parse_style_arg (ctx, state, str);
    g_free (str);
}

static void
rsvg_lookup_parse_style_pair (RsvgHandle * ctx, RsvgState * state,
                              const char *key, RsvgPropertyBag * atts)
{
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, key)) != NULL)
        rsvg_parse_style_pair (ctx, state, key, value);
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

/* Split a CSS2 style into individual style arguments, setting attributes
   in the SVG context.
   
   It's known that this is _way_ out of spec. A more complete CSS2
   implementation will happen later.
*/
void
rsvg_parse_style (RsvgHandle * ctx, RsvgState * state, const char *str)
{
    int start, end;
    char *arg;

    start = 0;
    while (str[start] != '\0') {
        for (end = start; str[end] != '\0' && str[end] != ';'; end++);
        arg = g_new (char, 1 + end - start);
        memcpy (arg, str + start, end - start);
        arg[end - start] = '\0';
        rsvg_parse_style_arg (ctx, state, arg);
        g_free (arg);
        start = end;
        if (str[start] == ';')
            start++;
        while (str[start] == ' ')
            start++;
    }
}

static void
rsvg_css_define_style (RsvgHandle * ctx, const gchar * style_name, const char *style_def)
{
    GString *str = g_string_new (style_def);
    char *existing = NULL;

    /* push name/style pair into HT */
    existing = (char *) g_hash_table_lookup (ctx->priv->css_props, style_name);
    if (existing != NULL)
        g_string_append_len (str, existing, strlen (existing));

    /* will destroy the existing key and value for us */
    g_hash_table_insert (ctx->priv->css_props, (gpointer) g_strdup ((gchar *) style_name),
                         (gpointer) str->str);
    g_string_free (str, FALSE);
}

#ifdef HAVE_LIBCROCO

#include <libcroco/libcroco.h>

typedef struct _CSSUserData {
    RsvgHandle *ctx;
    GString *def;
} CSSUserData;

static void
css_user_data_init (CSSUserData * user_data, RsvgHandle * ctx)
{
    user_data->ctx = ctx;
    user_data->def = NULL;
}

static void
ccss_start_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;
    user_data->def = g_string_new (NULL);
}

static void
ccss_end_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;
    CRSelector *cur;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    if (a_selector_list)
        for (cur = a_selector_list; cur; cur = cur->next) {
            if (cur->simple_sel) {
                gchar *style_name = (gchar *) cr_simple_sel_to_string (cur->simple_sel);
                if (style_name) {
                    rsvg_css_define_style (user_data->ctx, style_name, user_data->def->str);
                    g_free (style_name);
                }
            }
        }

    g_string_free (user_data->def, TRUE);
}

static void
ccss_property (CRDocHandler * a_handler, CRString * a_name, CRTerm * a_expr, gboolean a_important)
{
    CSSUserData *user_data;
    gchar *expr = NULL, *name = NULL;
    size_t len = 0;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    if (a_name && a_expr && user_data->def) {
        name = (gchar *) cr_string_peek_raw_str (a_name);
        len = cr_string_peek_raw_str_len (a_name);

        g_string_append_len (user_data->def, (gchar *) name, len);
        g_string_append (user_data->def, ": ");
        expr = (gchar *) cr_term_to_string (a_expr);
        g_string_append_len (user_data->def, expr, strlen (expr));
        g_free (expr);
        g_string_append (user_data->def, "; ");
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

static void
rsvg_real_parse_cssbuffer (RsvgHandle * ctx, const char *buff, size_t buflen)
{
    enum CRStatus status = CR_OK;
    CRParser *parser = NULL;
    CRDocHandler *css_handler = NULL;
    CSSUserData user_data;

    css_handler = cr_doc_handler_new ();
    init_sac_handler (css_handler);

    css_user_data_init (&user_data, ctx);
    css_handler->app_data = &user_data;

    /* TODO: fix libcroco to take in const strings */
    parser = cr_parser_new_from_buf ((guchar *) buff, (gulong) buflen, CR_UTF_8, FALSE);
    status = cr_parser_set_sac_handler (parser, css_handler);

    if (status != CR_OK) {
        g_warning (_("Error setting CSS SAC handler\n"));
        cr_parser_destroy (parser);
        return;
    }

    status = cr_parser_set_use_core_grammar (parser, FALSE);
    status = cr_parser_parse (parser);

    cr_parser_destroy (parser);
}

static void
ccss_import_style (CRDocHandler * a_this,
                   GList * a_media_list,
                   CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location)
{
    if (a_uri) {
        GByteArray *stylesheet_data;
        CSSUserData *user_data;

        user_data = (CSSUserData *) a_this->app_data;

        stylesheet_data =
            _rsvg_acquire_xlink_href_resource ((gchar *) cr_string_peek_raw_str (a_uri),
                                               rsvg_handle_get_base_uri (user_data->ctx), NULL);
        if (stylesheet_data) {
            rsvg_real_parse_cssbuffer (user_data->ctx, (const char *) stylesheet_data->data,
                                       (size_t) stylesheet_data->len);
            g_byte_array_free (stylesheet_data, TRUE);
        }
    }
}

#else                           /* !HAVE_LIBCROCO */

static void
rsvg_real_parse_cssbuffer (RsvgHandle * ctx, const char *buff, size_t buflen)
{
    /*
     * Extremely poor man's CSS parser. Not robust. Not compliant.
     * See also: http://www.w3.org/TR/REC-CSS2/syndata.html
     */

    size_t loc = 0;

    while (loc < buflen) {
        GString *style_name = g_string_new (NULL);
        GString *style_props = g_string_new (NULL);

        /* advance to the style's name */
        while (loc < buflen && g_ascii_isspace (buff[loc]))
            loc++;

        while (loc < buflen && !g_ascii_isspace (buff[loc]))
            g_string_append_c (style_name, buff[loc++]);

        /* advance to the first { that defines the style's properties */
        while (loc < buflen && buff[loc++] != '{');

        while (loc < buflen && g_ascii_isspace (buff[loc]))
            loc++;

        while (loc < buflen && buff[loc] != '}') {
            /* suck in and append our property */
            while (loc < buflen && buff[loc] != ';' && buff[loc] != '}')
                g_string_append_c (style_props, buff[loc++]);

            if (loc == buflen || buff[loc] == '}')
                break;
            else {
                g_string_append_c (style_props, ';');

                /* advance to the next property */
                loc++;
                while (loc < buflen && g_ascii_isspace (buff[loc]))
                    loc++;
            }
        }

        rsvg_css_define_style (ctx, style_name->str, style_props->str);
        g_string_free (style_name, TRUE);
        g_string_free (style_props, TRUE);

        loc++;
        while (loc < buflen && g_ascii_isspace (buff[loc]))
            loc++;
    }
}

#endif                          /* HAVE_LIBCROCO */

void
rsvg_parse_cssbuffer (RsvgHandle * ctx, const char *buff, size_t buflen)
{
    /* delegate off to the builtin or libcroco implementation */
    rsvg_real_parse_cssbuffer (ctx, buff, buflen);
}

/* Parse an SVG transform string into an affine matrix. Reference: SVG
   working draft dated 1999-07-06, section 8.5. Return TRUE on
   success. */
gboolean
rsvg_parse_transform (double dst[6], const char *src)
{
    int idx;
    char keyword[32];
    double args[6];
    int n_args;
    guint key_len;
    double tmp_affine[6];

    _rsvg_affine_identity (dst);

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
            _rsvg_affine_multiply (dst, args, dst);
        } else if (!strcmp (keyword, "translate")) {
            if (n_args == 1)
                args[1] = 0;
            else if (n_args != 2)
                return FALSE;
            _rsvg_affine_translate (tmp_affine, args[0], args[1]);
            _rsvg_affine_multiply (dst, tmp_affine, dst);
        } else if (!strcmp (keyword, "scale")) {
            if (n_args == 1)
                args[1] = args[0];
            else if (n_args != 2)
                return FALSE;
            _rsvg_affine_scale (tmp_affine, args[0], args[1]);
            _rsvg_affine_multiply (dst, tmp_affine, dst);
        } else if (!strcmp (keyword, "rotate")) {
            if (n_args == 1) {
                _rsvg_affine_rotate (tmp_affine, args[0]);
                _rsvg_affine_multiply (dst, tmp_affine, dst);
            } else if (n_args == 3) {
                _rsvg_affine_translate (tmp_affine, args[1], args[2]);
                _rsvg_affine_multiply (dst, tmp_affine, dst);

                _rsvg_affine_rotate (tmp_affine, args[0]);
                _rsvg_affine_multiply (dst, tmp_affine, dst);

                _rsvg_affine_translate (tmp_affine, -args[1], -args[2]);
                _rsvg_affine_multiply (dst, tmp_affine, dst);
            } else
                return FALSE;
        } else if (!strcmp (keyword, "skewX")) {
            if (n_args != 1)
                return FALSE;
            _rsvg_affine_shear (tmp_affine, args[0]);
            _rsvg_affine_multiply (dst, tmp_affine, dst);
        } else if (!strcmp (keyword, "skewY")) {
            if (n_args != 1)
                return FALSE;
            _rsvg_affine_shear (tmp_affine, args[0]);
            /* transpose the affine, given that we know [1] is zero */
            tmp_affine[1] = tmp_affine[2];
            tmp_affine[2] = 0;
            _rsvg_affine_multiply (dst, tmp_affine, dst);
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
    double affine[6];

    if (rsvg_parse_transform (affine, str)) {
        _rsvg_affine_multiply (state->personal_affine, affine, state->personal_affine);
        _rsvg_affine_multiply (state->affine, affine, state->affine);
    }
}

static gboolean
rsvg_lookup_apply_css_style (RsvgHandle * ctx, const char *target, RsvgState * state)
{
    const char *value = (const char *) g_hash_table_lookup (ctx->priv->css_props, target);

    if (value != NULL) {
        rsvg_parse_style (ctx, state, value);
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

    if (klazz != NULL) {
        i = strlen (klazz);
        while (j < i) {
            found = FALSE;
            klazz_list = g_string_new (".");

            while (j < i && g_ascii_isspace (klazz[j]))
                j++;

            while (j < i && !g_ascii_isspace (klazz[j]))
                g_string_append_c (klazz_list, klazz[j++]);

            /* tag.class */
            if (tag != NULL && klazz_list->len != 1) {
                target = g_strdup_printf ("%s%s", tag, klazz_list->str);
                found = found || rsvg_lookup_apply_css_style (ctx, target, state);
                g_free (target);
            }

            /* tag.class#id */
            if (tag != NULL && klazz_list->len != 1 && id != NULL) {
                target = g_strdup_printf ("%s%s#%s", tag, klazz_list->str, id);
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

    /* tag#id */
    if (tag != NULL && id != NULL && !found) {
        target = g_strdup_printf ("%s#%s", tag, id);
        rsvg_lookup_apply_css_style (ctx, target, state);
        g_free (target);
    }

    /* #id */
    if (id != NULL && !found) {
        target = g_strdup_printf ("#%s", id);
        found = rsvg_lookup_apply_css_style (ctx, target, state);
        g_free (target);
    }

    /* tag */
    if (tag != NULL && !found)
        found = rsvg_lookup_apply_css_style (ctx, tag, state);

    if (rsvg_property_bag_size (atts) > 0) {
        const char *value;

        rsvg_parse_style_pairs (ctx, state, atts);

        if ((value = rsvg_property_bag_lookup (atts, "style")) != NULL)
            rsvg_parse_style (ctx, state, value);
        if ((value = rsvg_property_bag_lookup (atts, "transform")) != NULL)
            rsvg_parse_transform_attr (ctx, state, value);
    }
}

RsvgState *
rsvg_state_current (RsvgDrawingCtx * ctx)
{
    return g_slist_nth_data (ctx->state, 0);
}

RsvgState *
rsvg_state_parent (RsvgDrawingCtx * ctx)
{
    return g_slist_nth_data (ctx->state, 1);
}

RsvgPropertyBag *
rsvg_property_bag_new (const char **atts)
{
    RsvgPropertyBag *bag;
    int i;

    bag = g_new (RsvgPropertyBag, 1);
    bag->props = g_hash_table_new_full (g_str_hash, g_str_equal, NULL, NULL);

    if (atts != NULL) {
        for (i = 0; atts[i] != NULL; i += 2)
            g_hash_table_insert (bag->props, (gpointer) atts[i], (gpointer) atts[i + 1]);
    }

    return bag;
}

void
rsvg_property_bag_free (RsvgPropertyBag * bag)
{
    g_hash_table_destroy (bag->props);
    g_free (bag);
}

G_CONST_RETURN char *
rsvg_property_bag_lookup (RsvgPropertyBag * bag, const char *key)
{
    return (const char *) g_hash_table_lookup (bag->props, (gconstpointer) key);
}

guint
rsvg_property_bag_size (RsvgPropertyBag * bag)
{
    return g_hash_table_size (bag->props);
}

void
rsvg_property_bag_enumerate (RsvgPropertyBag * bag, RsvgPropertyBagEnumFunc func,
                             gpointer user_data)
{
    g_hash_table_foreach (bag->props, (GHFunc) func, user_data);
}

void
rsvg_state_push (RsvgDrawingCtx * ctx)
{
    RsvgState *data;
    RsvgState *baseon;

    baseon = (RsvgState *) g_slist_nth_data (ctx->state, 0);
    data = g_slice_new (RsvgState);

    if (baseon) {
        int i;
        rsvg_state_init (data);
        rsvg_state_reinherit (data, baseon);
        for (i = 0; i < 6; i++)
            data->affine[i] = baseon->affine[i];
    } else
        rsvg_state_init (data);

    ctx->state = g_slist_prepend (ctx->state, data);
}

void
rsvg_state_pop (RsvgDrawingCtx * ctx)
{
    GSList *link = g_slist_nth (ctx->state, 0);
    RsvgState *dead_state = (RsvgState *) link->data;

    rsvg_state_finalize (dead_state);
    ctx->state = g_slist_delete_link (ctx->state, link);
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
    if (dominate == 3)
        return;

    /*This is a special domination mode for patterns, the transform
       is simply left as is, wheras the style is totally overridden */
    if (dominate == 2) {
        rsvg_state_override (rsvg_state_current (ctx), state);
    } else if (dominate) {
        RsvgState *parent;
        rsvg_state_clone (rsvg_state_current (ctx), state);

        parent = rsvg_state_parent (ctx);
        if (parent) {
            rsvg_state_dominate (rsvg_state_current (ctx), rsvg_state_parent (ctx));
            _rsvg_affine_multiply (rsvg_state_current (ctx)->affine,
                                   rsvg_state_current (ctx)->affine,
                                   rsvg_state_parent (ctx)->affine);
        }
    } else {
        RsvgState *parent;
        rsvg_state_clone (rsvg_state_current (ctx), state);

        parent = rsvg_state_parent (ctx);
        if (parent) {
            rsvg_state_reinherit (rsvg_state_current (ctx), rsvg_state_parent (ctx));
            _rsvg_affine_multiply (rsvg_state_current (ctx)->affine,
                                   rsvg_state_current (ctx)->affine,
                                   rsvg_state_parent (ctx)->affine);
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
