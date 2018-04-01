/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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

#include <errno.h>
#include <string.h>
#include <math.h>

#include "rsvg-attributes.h"
#include "rsvg-private.h"
#include "rsvg-filter.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"

#include <libcroco/libcroco.h>

typedef gboolean (*InheritanceFunction) (gboolean dst_has_prop, gboolean src_has_prop);

/* Defined in rust/src/length.rs */
extern RsvgStrokeDasharray *rsvg_parse_stroke_dasharray(const char *str);
extern RsvgStrokeDasharray *rsvg_stroke_dasharray_clone(RsvgStrokeDasharray *dash);
extern void rsvg_stroke_dasharray_free(RsvgStrokeDasharray *dash);

/* Defined in rsvg_internals/src/state.rs */
extern State *rsvg_state_rust_new(void);
extern void rsvg_state_rust_free(State *state);
extern State *rsvg_state_rust_clone(State *state);
extern cairo_matrix_t rsvg_state_rust_get_affine(const State *state);
extern void rsvg_state_rust_set_affine(State *state, cairo_matrix_t affine);

extern gboolean rsvg_state_rust_parse_style_pair(State *state, RsvgAttribute attr, const char *value)
    G_GNUC_WARN_UNUSED_RESULT;

extern void rsvg_state_rust_inherit_run(State *dst, State *src, InheritanceFunction inherit_fn);

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

static StyleValueData *
style_value_data_new (const gchar *value, gboolean important)
{
    StyleValueData *ret;

    ret = g_new0 (StyleValueData, 1);
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

static void
rsvg_state_init (RsvgState * state)
{
    memset (state, 0, sizeof (RsvgState));

    state->parent = NULL;

    state->mask = NULL;
    state->opacity = 0xff;
    state->current_color = 0xff000000; /* See bgo#764808; we don't inherit CSS
                                        * from the public API, so start off with
                                        * opaque black instead of transparent.
                                        */
    state->fill = rsvg_paint_server_parse (NULL, "#000");
    state->fill_opacity = 0xff;
    state->stroke_opacity = 0xff;
    state->stroke_width = rsvg_length_parse ("1", LENGTH_DIR_BOTH);
    state->miter_limit = 4;

    /* The following two start as INHERIT, even though has_stop_color and
     * has_stop_opacity get initialized to FALSE below.  This is so that the
     * first pass of rsvg_state_inherit_run(), called from
     * rsvg_state_reconstruct() from the "stop" element code, will correctly
     * initialize the destination state from the toplevel element.
     *
     */
    state->stop_color.kind = RSVG_CSS_COLOR_SPEC_INHERIT;
    state->stop_opacity.kind = RSVG_OPACITY_INHERIT;

    state->clip_rule = CAIRO_FILL_RULE_WINDING;
    state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    state->comp_op = CAIRO_OPERATOR_OVER;
    state->flood_color = 0;
    state->flood_opacity = 255;

    state->font_weight = PANGO_WEIGHT_NORMAL;
    state->font_stretch = PANGO_STRETCH_NORMAL;
    state->text_dir = PANGO_DIRECTION_LTR;
    state->text_gravity = PANGO_GRAVITY_SOUTH;
    state->unicode_bidi = UNICODE_BIDI_NORMAL;
    state->visible = TRUE;
    state->cond_true = TRUE;
    state->filter = NULL;
    state->clip_path = NULL;
    state->startMarker = NULL;
    state->middleMarker = NULL;
    state->endMarker = NULL;

    state->has_current_color = FALSE;
    state->has_flood_color = FALSE;
    state->has_flood_opacity = FALSE;
    state->has_fill_server = FALSE;
    state->has_fill_opacity = FALSE;
    state->has_clip_rule = FALSE;
    state->has_stroke_server = FALSE;
    state->has_stroke_opacity = FALSE;
    state->has_stroke_width = FALSE;
    state->has_miter_limit = FALSE;
    state->has_dash = FALSE;
    state->has_dashoffset = FALSE;
    state->has_visible = FALSE;
    state->has_cond = FALSE;
    state->has_stop_color = FALSE;
    state->has_stop_opacity = FALSE;
    state->has_font_weight = FALSE;
    state->has_font_stretch = FALSE;
    state->has_text_dir = FALSE;
    state->has_text_gravity = FALSE;
    state->has_unicode_bidi = FALSE;
    state->has_startMarker = FALSE;
    state->has_middleMarker = FALSE;
    state->has_endMarker = FALSE;

    state->shape_rendering_type = SHAPE_RENDERING_AUTO;
    state->has_shape_rendering_type = FALSE;
    state->text_rendering_type = TEXT_RENDERING_AUTO;
    state->has_text_rendering_type = FALSE;

    state->styles = g_hash_table_new_full (g_str_hash, g_str_equal,
                                           g_free, (GDestroyNotify) style_value_data_free);

    state->state_rust = rsvg_state_rust_new();
}

RsvgState *
rsvg_state_new_with_parent (RsvgState *parent)
{
    RsvgState *state;

    state = g_slice_new (RsvgState);
    rsvg_state_init (state);

    if (parent) {
        rsvg_state_reinherit (state, parent);
        rsvg_state_set_affine (state, rsvg_state_get_affine (parent));
        state->parent = parent;
    }

    return state;
}

RsvgState *
rsvg_state_new (void)
{
    return rsvg_state_new_with_parent (NULL);
}

static void
rsvg_state_finalize (RsvgState * state)
{
    g_free (state->filter);
    state->filter = NULL;

    g_free (state->mask);
    state->mask = NULL;

    g_free (state->clip_path);
    state->clip_path = NULL;

    g_free (state->startMarker);
    state->startMarker = NULL;

    g_free (state->middleMarker);
    state->middleMarker = NULL;

    g_free (state->endMarker);
    state->endMarker = NULL;

    rsvg_paint_server_unref (state->fill);
    state->fill = NULL;

    rsvg_paint_server_unref (state->stroke);
    state->stroke = NULL;

    if (state->dash) {
        rsvg_stroke_dasharray_free (state->dash);
        state->dash = NULL;
    }

    if (state->styles) {
        g_hash_table_unref (state->styles);
        state->styles = NULL;
    }

    if (state->state_rust) {
        rsvg_state_rust_free (state->state_rust);
        state->state_rust = NULL;
    }
}

void
rsvg_state_free (RsvgState *state)
{
    g_assert (state != NULL);

    rsvg_state_finalize (state);
    g_slice_free (RsvgState, state);
}

void
rsvg_state_reinit (RsvgState * state)
{
    RsvgState *parent = state->parent;
    rsvg_state_finalize (state);
    rsvg_state_init (state);
    state->parent = parent;
}

void
rsvg_state_clone (RsvgState * dst, const RsvgState * src)
{
    RsvgState *parent = dst->parent;

    rsvg_state_finalize (dst);

    *dst = *src;
    dst->parent = parent;
    dst->filter = g_strdup (src->filter);
    dst->mask = g_strdup (src->mask);
    dst->clip_path = g_strdup (src->clip_path);
    dst->startMarker = g_strdup (src->startMarker);
    dst->middleMarker = g_strdup (src->middleMarker);
    dst->endMarker = g_strdup (src->endMarker);
    rsvg_paint_server_ref (dst->fill);
    rsvg_paint_server_ref (dst->stroke);

    dst->styles = g_hash_table_ref (src->styles);

    if (src->dash) {
        dst->dash = rsvg_stroke_dasharray_clone (src->dash);
    }

    dst->state_rust = rsvg_state_rust_clone(src->state_rust);
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
    if (function (dst->has_clip_rule, src->has_clip_rule))
        dst->clip_rule = src->clip_rule;
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
    if (function (dst->has_stop_color, src->has_stop_color)) {
        if (dst->stop_color.kind == RSVG_CSS_COLOR_SPEC_INHERIT) {
            dst->has_stop_color = TRUE;
            dst->stop_color = src->stop_color;
        }
    }
    if (function (dst->has_stop_opacity, src->has_stop_opacity)) {
        if (dst->stop_opacity.kind == RSVG_OPACITY_INHERIT) {
            dst->has_stop_opacity = TRUE;
            dst->stop_opacity = src->stop_opacity;
        }
    }
    if (function (dst->has_cond, src->has_cond))
        dst->cond_true = src->cond_true;
    if (function (dst->has_font_weight, src->has_font_weight))
        dst->font_weight = src->font_weight;
    if (function (dst->has_font_stretch, src->has_font_stretch))
        dst->font_stretch = src->font_stretch;
    if (function (dst->has_text_dir, src->has_text_dir))
        dst->text_dir = src->text_dir;
    if (function (dst->has_text_gravity, src->has_text_gravity))
        dst->text_gravity = src->text_gravity;
    if (function (dst->has_unicode_bidi, src->has_unicode_bidi))
        dst->unicode_bidi = src->unicode_bidi;
    if (function (dst->has_startMarker, src->has_startMarker)) {
        g_free (dst->startMarker);
        dst->startMarker = g_strdup (src->startMarker);
    }
    if (function (dst->has_middleMarker, src->has_middleMarker)) {
        g_free (dst->middleMarker);
        dst->middleMarker = g_strdup (src->middleMarker);
    }
    if (function (dst->has_endMarker, src->has_endMarker)) {
        g_free (dst->endMarker);
        dst->endMarker = g_strdup (src->endMarker);
    }
    if (function (dst->has_shape_rendering_type, src->has_shape_rendering_type))
            dst->shape_rendering_type = src->shape_rendering_type;
    if (function (dst->has_text_rendering_type, src->has_text_rendering_type))
            dst->text_rendering_type = src->text_rendering_type;

    if (function (dst->has_visible, src->has_visible))
        dst->visible = src->visible;

    if (function (dst->has_dash, src->has_dash)) {
        if (dst->dash) {
            rsvg_stroke_dasharray_free (dst->dash);
            dst->dash = NULL;
        }

        if (src->dash) {
            dst->dash = rsvg_stroke_dasharray_clone (src->dash);
        }
    }

    if (function (dst->has_dashoffset, src->has_dashoffset)) {
        dst->dash_offset = src->dash_offset;
    }

    rsvg_state_rust_inherit_run (dst->state_rust, src->state_rust, function);

    if (inherituninheritables) {
        g_free (dst->clip_path);
        dst->clip_path = g_strdup (src->clip_path);
        g_free (dst->mask);
        dst->mask = g_strdup (src->mask);
        g_free (dst->filter);
        dst->filter = g_strdup (src->filter);
        dst->enable_background = src->enable_background;
        dst->opacity = src->opacity;
        dst->comp_op = src->comp_op;
    }
}

/*
  reinherit is given dst which is the top of the state stack
  and src which is the layer before in the state stack from
  which it should be inherited from 
*/

static gboolean
reinheritfunction (gboolean dst, gboolean src)
{
    if (!dst)
        return TRUE;
    return FALSE;
}

void
rsvg_state_reinherit (RsvgState *dst, const RsvgState *src)
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

static gboolean
dominatefunction (gboolean dst, gboolean src)
{
    if (!dst || src)
        return TRUE;
    return FALSE;
}

void
rsvg_state_dominate (RsvgState *dst, const RsvgState *src)
{
    rsvg_state_inherit_run (dst, src, dominatefunction, 0);
}

/* copy everything inheritable from the src to the dst */

static gboolean
forcefunction (gboolean dst, gboolean src)
{
    return TRUE;
}

void
rsvg_state_force (RsvgState *dst, const RsvgState *src)
{
    rsvg_state_inherit_run (dst, src, forcefunction, 0);
}

/*
  put something new on the inheritance stack, dst is the top of the stack, 
  src is the state to be integrated, this is essentially the opposite of
  reinherit, because it is being given stuff to be integrated on the top, 
  rather than the context underneath.
*/

static gboolean
inheritfunction (gboolean dst, gboolean src)
{
    return src;
}

void
rsvg_state_inherit (RsvgState *dst, const RsvgState *src)
{
    rsvg_state_inherit_run (dst, src, inheritfunction, 1);
}

typedef enum {
    PAIR_SOURCE_STYLE,
    PAIR_SOURCE_PRESENTATION_ATTRIBUTE
} PairSource;

static gboolean
rsvg_parse_style_pair (RsvgState *state,
                       const gchar *name,
                       RsvgAttribute attr,
                       const gchar *value,
                       gboolean important,
                       PairSource source) G_GNUC_WARN_UNUSED_RESULT;

/* Parse a CSS2 style argument, setting the SVG context attributes. */
static gboolean
rsvg_parse_style_pair (RsvgState *state,
                       const gchar *name,
                       RsvgAttribute attr,
                       const gchar *value,
                       gboolean important,
                       PairSource source)
{
    StyleValueData *data;
    gboolean success = TRUE;

    if (name == NULL || value == NULL)
        return success;

    data = g_hash_table_lookup (state->styles, name);
    if (data && data->important && !important)
        return success;

    g_hash_table_insert (state->styles,
                         (gpointer) g_strdup (name),
                         (gpointer) style_value_data_new (value, important));

    switch (attr) {
    case RSVG_ATTRIBUTE_COLOR:
    {
        RsvgCssColorSpec spec;

        spec = rsvg_css_parse_color (value, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_NO);
        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            /* FIXME: we should inherit; see how stop-color is handled in rsvg-styles.c */
            state->has_current_color = FALSE;
            break;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            state->current_color = spec.argb;
            state->has_current_color = TRUE;
            break;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            /* FIXME: no error handling */
            state->has_current_color = FALSE;
            break;

        default:
            g_assert_not_reached ();
        }
    }
    break;

    case RSVG_ATTRIBUTE_OPACITY:
    {
        RsvgOpacitySpec spec;

        spec = rsvg_css_parse_opacity (value);
        if (spec.kind == RSVG_OPACITY_SPECIFIED) {
            state->opacity = spec.opacity;
        } else {
            state->opacity = 0;
            /* FIXME: handle INHERIT and PARSE_ERROR */
        }
    }
    break;

    case RSVG_ATTRIBUTE_FLOOD_COLOR:
    {
        RsvgCssColorSpec spec;

        spec = rsvg_css_parse_color (value, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_YES);
        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            /* FIXME: we should inherit; see how stop-color is handled in rsvg-styles.c */
            state->has_current_color = FALSE;
            break;

        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            /* FIXME: in the caller, fix up the current color */
            state->has_flood_color = FALSE;
            break;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            state->flood_color = spec.argb;
            state->has_flood_color = TRUE;
            break;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            /* FIXME: no error handling */
            state->has_current_color = FALSE;
            break;

        default:
            g_assert_not_reached ();
        }
    }
    break;

    case RSVG_ATTRIBUTE_FLOOD_OPACITY:
    {
        RsvgOpacitySpec spec;

        spec = rsvg_css_parse_opacity (value);
        if (spec.kind == RSVG_OPACITY_SPECIFIED) {
            state->flood_opacity = spec.opacity;
        } else {
            state->flood_opacity = 0;
            /* FIXME: handle INHERIT and PARSE_ERROR */
        }

        state->has_flood_opacity = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_FILTER:
    {
        g_free (state->filter);
        state->filter = rsvg_get_url_string (value, NULL);
    }
    break;

    case RSVG_ATTRIBUTE_MASK:
    {
        g_free (state->mask);
        state->mask = rsvg_get_url_string (value, NULL);
    }
    break;

    case RSVG_ATTRIBUTE_CLIP_PATH:
    {
        g_free (state->clip_path);
        state->clip_path = rsvg_get_url_string (value, NULL);
    }
    break;

    case RSVG_ATTRIBUTE_ENABLE_BACKGROUND:
    {
        if (g_str_equal (value, "new"))
            state->enable_background = RSVG_ENABLE_BACKGROUND_NEW;
        else
            state->enable_background = RSVG_ENABLE_BACKGROUND_ACCUMULATE;
    }
    break;

    case RSVG_ATTRIBUTE_COMP_OP:
    {
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
    }
    break;

    case RSVG_ATTRIBUTE_DISPLAY:
    {
        state->has_visible = TRUE;
        if (g_str_equal (value, "none"))
            state->visible = FALSE;
        else if (!g_str_equal (value, "inherit"))
            state->visible = TRUE;
        else
            state->has_visible = FALSE;
    }
    break;

    case RSVG_ATTRIBUTE_VISIBILITY:
    {
        state->has_visible = TRUE;
        if (g_str_equal (value, "visible"))
            state->visible = TRUE;
        else if (!g_str_equal (value, "inherit"))
            state->visible = FALSE;     /* collapse or hidden */
        else
            state->has_visible = FALSE;
    }
    break;

    case RSVG_ATTRIBUTE_FILL:
    {
        RsvgPaintServer *fill = state->fill;
        state->fill =
            rsvg_paint_server_parse (&state->has_fill_server, value);
        rsvg_paint_server_unref (fill);
    }
    break;

    case RSVG_ATTRIBUTE_FILL_OPACITY:
    {
        RsvgOpacitySpec spec;

        spec = rsvg_css_parse_opacity (value);
        if (spec.kind == RSVG_OPACITY_SPECIFIED) {
            state->fill_opacity = spec.opacity;
        } else {
            state->fill_opacity = 0;
            /* FIXME: handle INHERIT and PARSE_ERROR */
        }

        state->has_fill_opacity = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_CLIP_RULE:
    {
        state->has_clip_rule = TRUE;
        if (g_str_equal (value, "nonzero"))
            state->clip_rule = CAIRO_FILL_RULE_WINDING;
        else if (g_str_equal (value, "evenodd"))
            state->clip_rule = CAIRO_FILL_RULE_EVEN_ODD;
        else
            state->has_clip_rule = FALSE;
    }
    break;

    case RSVG_ATTRIBUTE_STROKE:
    {
        RsvgPaintServer *stroke = state->stroke;

        state->stroke =
            rsvg_paint_server_parse (&state->has_stroke_server, value);

        rsvg_paint_server_unref (stroke);
    }
    break;

    case RSVG_ATTRIBUTE_STROKE_WIDTH:
    {
        state->stroke_width = rsvg_length_parse (value, LENGTH_DIR_BOTH);
        state->has_stroke_width = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_STROKE_OPACITY:
    {
        RsvgOpacitySpec spec;

        spec = rsvg_css_parse_opacity (value);
        if (spec.kind == RSVG_OPACITY_SPECIFIED) {
            state->stroke_opacity = spec.opacity;
        } else {
            state->stroke_opacity = 0;
            /* FIXME: handle INHERIT and PARSE_ERROR */
        }

        state->has_stroke_opacity = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_FONT_WEIGHT:
    {
        state->font_weight = rsvg_css_parse_font_weight (value, &state->has_font_weight);
    }
    break;

    case RSVG_ATTRIBUTE_FONT_STRETCH:
    {
        state->font_stretch = rsvg_css_parse_font_stretch (value, &state->has_font_stretch);
    }
    break;

    case RSVG_ATTRIBUTE_DIRECTION:
    {
        state->has_text_dir = TRUE;
        if (g_str_equal (value, "inherit")) {
            state->text_dir = PANGO_DIRECTION_LTR;
            state->has_text_dir = FALSE;
        } else if (g_str_equal (value, "rtl"))
            state->text_dir = PANGO_DIRECTION_RTL;
        else                    /* ltr */
            state->text_dir = PANGO_DIRECTION_LTR;
    }
    break;

    case RSVG_ATTRIBUTE_UNICODE_BIDI:
    {
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
    }
    break;

    case RSVG_ATTRIBUTE_WRITING_MODE:
    {
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
    }
    break;

    case RSVG_ATTRIBUTE_STOP_COLOR:
    {
        state->has_stop_color = TRUE;
        state->stop_color = rsvg_css_parse_color (value, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_YES);
    }
    break;

    case RSVG_ATTRIBUTE_STOP_OPACITY:
    {
        state->stop_opacity = rsvg_css_parse_opacity (value);
        state->has_stop_opacity = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_MARKER_START:
    {
        g_free (state->startMarker);
        state->startMarker = rsvg_get_url_string (value, NULL);
        state->has_startMarker = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_MARKER_MID:
    {
        g_free (state->middleMarker);
        state->middleMarker = rsvg_get_url_string (value, NULL);
        state->has_middleMarker = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_MARKER_END:
    {
        g_free (state->endMarker);
        state->endMarker = rsvg_get_url_string (value, NULL);
        state->has_endMarker = TRUE;
    }
    break;

    case RSVG_ATTRIBUTE_MARKER:
    {
        /* FIXME: ugly special case.  "marker" is a shorthand property, and can
         * only be used in a CSS style (or style attribute in an SVG element),
         * not as a presentation attribute.
         */
        if (source == PAIR_SOURCE_STYLE) {
            if (!state->has_startMarker) {
                g_free (state->startMarker);
                state->startMarker = rsvg_get_url_string (value, NULL);
                state->has_startMarker = TRUE;
            }

            if (!state->has_middleMarker) {
                g_free (state->middleMarker);
                state->middleMarker = rsvg_get_url_string (value, NULL);
                state->has_middleMarker = TRUE;
            }

            if (!state->has_endMarker) {
                g_free (state->endMarker);
                state->endMarker = rsvg_get_url_string (value, NULL);
                state->has_endMarker = TRUE;
            }
        }
    }
    break;

    case RSVG_ATTRIBUTE_STROKE_MITERLIMIT:
    {
        state->has_miter_limit = TRUE;
        state->miter_limit = g_ascii_strtod (value, NULL);
    }
    break;

    case RSVG_ATTRIBUTE_STROKE_DASHOFFSET:
    {
        state->has_dashoffset = TRUE;
        state->dash_offset = rsvg_length_parse (value, LENGTH_DIR_BOTH);
        if (state->dash_offset.length < 0.)
            state->dash_offset.length = 0.;
    }
    break;

    case RSVG_ATTRIBUTE_SHAPE_RENDERING:
    {
        state->has_shape_rendering_type = TRUE;

        if (g_str_equal (value, "auto") || g_str_equal (value, "default"))
            state->shape_rendering_type = SHAPE_RENDERING_AUTO;
        else if (g_str_equal (value, "optimizeSpeed"))
            state->shape_rendering_type = SHAPE_RENDERING_OPTIMIZE_SPEED;
        else if (g_str_equal (value, "crispEdges"))
            state->shape_rendering_type = SHAPE_RENDERING_CRISP_EDGES;
        else if (g_str_equal (value, "geometricPrecision"))
            state->shape_rendering_type = SHAPE_RENDERING_GEOMETRIC_PRECISION;
    }
    break;

    case RSVG_ATTRIBUTE_TEXT_RENDERING:
    {
        state->has_text_rendering_type = TRUE;

        if (g_str_equal (value, "auto") || g_str_equal (value, "default"))
            state->text_rendering_type = TEXT_RENDERING_AUTO;
        else if (g_str_equal (value, "optimizeSpeed"))
            state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_SPEED;
        else if (g_str_equal (value, "optimizeLegibility"))
            state->text_rendering_type = TEXT_RENDERING_OPTIMIZE_LEGIBILITY;
        else if (g_str_equal (value, "geometricPrecision"))
            state->text_rendering_type = TEXT_RENDERING_GEOMETRIC_PRECISION;
    }
    break;

    case RSVG_ATTRIBUTE_STROKE_DASHARRAY:
    {
        /* FIXME: the following returns NULL on error; find a way to propagate
         * errors from here.
         */
        RsvgStrokeDasharray *dash = rsvg_parse_stroke_dasharray (value);

        if (dash) {
            state->has_dash = TRUE;
            state->dash = dash;
        }
    }
    break;

    default:
        success = rsvg_state_rust_parse_style_pair(state->state_rust,
                                                   attr,
                                                   value);
        break;
    }

    return success;
}

/* returns TRUE if this element should be processed according to <switch> semantics
   http://www.w3.org/TR/SVG/struct.html#SwitchElement */
static gboolean
rsvg_eval_switch_attributes (RsvgPropertyBag * atts, gboolean * p_has_cond)
{
    gboolean required_features_ok = TRUE;
    gboolean required_extensions_ok = TRUE;
    gboolean system_language_ok = TRUE;
    gboolean has_cond = FALSE;

    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_REQUIRED_FEATURES:
            required_features_ok = rsvg_cond_check_required_features (value);
            has_cond = TRUE;
            break;

        case RSVG_ATTRIBUTE_REQUIRED_EXTENSIONS:
            required_extensions_ok = rsvg_cond_check_required_extensions (value);
            has_cond = TRUE;
            break;

        case RSVG_ATTRIBUTE_SYSTEM_LANGUAGE:
            system_language_ok = rsvg_cond_check_system_language (value);
            has_cond = TRUE;
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);

    if (p_has_cond)
        *p_has_cond = has_cond;

    return required_features_ok && required_extensions_ok && system_language_ok;
}

/* take a pair of the form (fill="#ff00ff") and parse it as a style */
void
rsvg_parse_presentation_attributes (RsvgState * state, RsvgPropertyBag * atts)
{
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;
    gboolean success;

    success = TRUE;

    iter = rsvg_property_bag_iter_begin (atts);

    while (success && rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        success = rsvg_parse_style_pair (state, key, attr, value, FALSE, PAIR_SOURCE_PRESENTATION_ATTRIBUTE);
    }

    rsvg_property_bag_iter_end (iter);

    if (!success) {
        return; /* FIXME: propagate errors upstream */
    }

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
gboolean
rsvg_parse_style_attribute_contents (RsvgState *state, const char *str)
{
    gchar **styles;
    guint i;
    gboolean success = TRUE;

    styles = g_strsplit (str, ";", -1);
    for (i = 0; success && i < g_strv_length (styles); i++) {
        gchar **values;
        values = g_strsplit (styles[i], ":", 2);
        if (!values)
            continue;

        if (g_strv_length (values) == 2) {
            gboolean important;
            gchar *style_value = NULL;
            gchar *first_value = values[0];
            gchar *second_value = values[1];
            gchar **split_list;

            /* Just remove single quotes in a trivial way.  No handling for any
             * special character inside the quotes is done.  This relates
             * especially to font-family names but cases with special characters
             * are rare.
             *
             * We need a real CSS parser, sigh.
             */
            split_list = g_strsplit (second_value, "'", -1);
            second_value = g_strjoinv(NULL, split_list);
            g_strfreev(split_list);

            if (parse_style_value (second_value, &style_value, &important)) {
                RsvgAttribute attr;

                g_strstrip (first_value);

                if (rsvg_attribute_from_name (first_value, &attr)) {
                    success = rsvg_parse_style_pair (state,
                                                     first_value,
                                                     attr,
                                                     style_value,
                                                     important,
                                                     PAIR_SOURCE_STYLE);
                }
            }

            g_free (style_value);
            g_free (second_value);
        }
        g_strfreev (values);
    }
    g_strfreev (styles);

    return success;
}

static void
rsvg_css_define_style (RsvgHandle *handle,
                       const gchar *selector,
                       const gchar *style_name,
                       const gchar *style_value,
                       gboolean important)
{
    GHashTable *styles;
    gboolean need_insert = FALSE;

    /* push name/style pair into HT */
    styles = g_hash_table_lookup (handle->priv->css_props, selector);
    if (styles == NULL) {
        styles = g_hash_table_new_full (g_str_hash, g_str_equal,
                                        g_free, (GDestroyNotify) style_value_data_free);
        g_hash_table_insert (handle->priv->css_props, (gpointer) g_strdup (selector), styles);
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
    RsvgHandle *handle;
    CRSelector *selector;
} CSSUserData;

static void
css_user_data_init (CSSUserData *user_data, RsvgHandle *handle)
{
    user_data->handle = handle;
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
                    rsvg_css_define_style (user_data->handle,
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
    g_message (_("CSS parsing error\n"));
}

static void
ccss_unrecoverable_error (CRDocHandler * a_handler)
{
    /* yup, like i care about CSS parsing errors ;-)
       ignore, chug along */
    g_message (_("CSS unrecoverable error\n"));
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
rsvg_parse_cssbuffer (RsvgHandle *handle, const char *buff, size_t buflen)
{
    CRParser *parser = NULL;
    CRDocHandler *css_handler = NULL;
    CSSUserData user_data;

    if (buff == NULL || buflen == 0)
        return;

    css_handler = cr_doc_handler_new ();
    init_sac_handler (css_handler);

    css_user_data_init (&user_data, handle);
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

    /* FIXME: we aren't reporting errors in the CSS; we have no way to know if
     * we should print the "buff" for diagnostics.
     */

    cr_parser_destroy (parser);
}

static void
ccss_import_style (CRDocHandler * a_this,
                   GList * a_media_list,
                   CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location)
{
    CSSUserData *user_data = (CSSUserData *) a_this->app_data;
    char *stylesheet_data;
    gsize stylesheet_data_len;
    char *mime_type = NULL;

    if (a_uri == NULL)
        return;

    stylesheet_data = _rsvg_handle_acquire_data (user_data->handle,
                                                 cr_string_peek_raw_str (a_uri),
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

    rsvg_parse_cssbuffer (user_data->handle, stylesheet_data, stylesheet_data_len);
    g_free (stylesheet_data);
    g_free (mime_type);
}

/**
 * rsvg_parse_transform_attr:
 * @state: State in which to apply the transform.
 * @str: String containing transform.
 *
 * Parses the transform attribute in @str and applies it to @state.
 **/
G_GNUC_WARN_UNUSED_RESULT static gboolean
rsvg_parse_transform_attr (RsvgState *state, const char *str)
{
    cairo_matrix_t affine;

    if (rsvg_parse_transform (&affine, str)) {
        cairo_matrix_t state_affine = rsvg_state_get_affine (state);
        cairo_matrix_multiply (&state_affine, &affine, &state_affine);
        rsvg_state_set_affine (state, state_affine);
        return TRUE;
    } else {
        return FALSE;
    }
}

static void
apply_style (const gchar *key, StyleValueData *value, gpointer user_data)
{
    RsvgState *state = user_data;
    RsvgAttribute attr;

    if (rsvg_attribute_from_name (key, &attr)) {
        gboolean success = rsvg_parse_style_pair (
            state, key, attr, value->value, value->important, PAIR_SOURCE_STYLE);
        /* FIXME: propagate errors upstream */
    }
}

static gboolean
rsvg_lookup_apply_css_style (RsvgHandle *handle, const char *target, RsvgState * state)
{
    GHashTable *styles;

    styles = g_hash_table_lookup (handle->priv->css_props, target);

    if (styles != NULL) {
        g_hash_table_foreach (styles, (GHFunc) apply_style, state);
        return TRUE;
    }
    return FALSE;
}

/**
 * rsvg_parse_style_attrs:
 * @handle: Rsvg handle.
 * @node: Rsvg node whose state should be modified
 * @tag: (nullable): The SVG tag we're processing (eg: circle, ellipse), optionally %NULL
 * @klazz: (nullable): The space delimited class list, optionally %NULL
 * @atts: Attributes in SAX style.
 *
 * Parses style and transform attributes and modifies state at top of
 * stack.
 **/
void
rsvg_parse_style_attrs (RsvgHandle *handle,
                        RsvgNode *node,
                        const char *tag, const char *klazz, const char *id, RsvgPropertyBag * atts)
{
    int i = 0, j = 0;
    char *target = NULL;
    gboolean found = FALSE;
    GString *klazz_list = NULL;
    RsvgState *state;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;
    gboolean success = TRUE;

    state = rsvg_node_get_state (node);

    rsvg_parse_presentation_attributes (state, atts);

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
    rsvg_lookup_apply_css_style (handle, "*", state);

    /* tag */
    if (tag != NULL) {
        rsvg_lookup_apply_css_style (handle, tag, state);
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
                found = found || rsvg_lookup_apply_css_style (handle, target, state);
                g_free (target);
            }

            /* class#id */
            if (klazz_list->len != 1 && id != NULL) {
                target = g_strdup_printf ("%s#%s", klazz_list->str, id);
                found = found || rsvg_lookup_apply_css_style (handle, target, state);
                g_free (target);
            }

            /* tag.class */
            if (tag != NULL && klazz_list->len != 1) {
                target = g_strdup_printf ("%s%s", tag, klazz_list->str);
                found = found || rsvg_lookup_apply_css_style (handle, target, state);
                g_free (target);
            }

            /* didn't find anything more specific, just apply the class style */
            if (!found) {
                found = found || rsvg_lookup_apply_css_style (handle, klazz_list->str, state);
            }
            g_string_free (klazz_list, TRUE);
        }
    }

    /* #id */
    if (id != NULL) {
        target = g_strdup_printf ("#%s", id);
        rsvg_lookup_apply_css_style (handle, target, state);
        g_free (target);
    }

    /* tag#id */
    if (tag != NULL && id != NULL) {
        target = g_strdup_printf ("%s#%s", tag, id);
        rsvg_lookup_apply_css_style (handle, target, state);
        g_free (target);
    }

    iter = rsvg_property_bag_iter_begin (atts);

    while (success && rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_STYLE:
            success = rsvg_parse_style_attribute_contents (state, value);
            break;

        case RSVG_ATTRIBUTE_TRANSFORM:
            if (!rsvg_parse_transform_attr (state, value)) {
                rsvg_node_set_attribute_parse_error (node,
                                                     "transform",
                                                     "Invalid transformation");
            }
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);

    /* FIXME: propagate errors upstream */
    /* return success; */
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

        rsvg_state_free (state);

        state = parent;
    }
}

cairo_matrix_t
rsvg_state_get_affine (const RsvgState *state)
{
    return rsvg_state_rust_get_affine (state->state_rust);
}

void
rsvg_state_set_affine (RsvgState *state, cairo_matrix_t affine)
{
    rsvg_state_rust_set_affine (state->state_rust, affine);
}

RsvgPaintServer *
rsvg_state_get_stroke (RsvgState *state)
{
    return state->stroke;
}

guint8
rsvg_state_get_stroke_opacity (RsvgState *state)
{
    return state->stroke_opacity;
}

RsvgLength
rsvg_state_get_stroke_width (RsvgState *state)
{
    return state->stroke_width;
}

double
rsvg_state_get_miter_limit (RsvgState *state)
{
    return state->miter_limit;
}

gboolean
rsvg_state_get_cond_true (RsvgState *state)
{
    return state->cond_true;
}

void
rsvg_state_set_cond_true (RsvgState *state, gboolean cond_true)
{
    state->cond_true = cond_true;
}

RsvgCssColorSpec *
rsvg_state_get_stop_color (RsvgState *state)
{
    if (state->has_stop_color) {
        return &state->stop_color;
    } else {
        return NULL;
    }
}

RsvgOpacitySpec *
rsvg_state_get_stop_opacity (RsvgState *state)
{
    if (state->has_stop_opacity) {
        return &state->stop_opacity;
    } else {
        return NULL;
    }
}

RsvgStrokeDasharray *
rsvg_state_get_stroke_dasharray (RsvgState *state)
{
    return state->dash;
}

RsvgLength
rsvg_state_get_dash_offset (RsvgState *state)
{
    return state->dash_offset;
}

guint32
rsvg_state_get_current_color (RsvgState *state)
{
    return state->current_color;
}

UnicodeBidi
rsvg_state_get_unicode_bidi (RsvgState *state)
{
    return state->unicode_bidi;
}

PangoDirection
rsvg_state_get_text_dir (RsvgState *state)
{
    return state->text_dir;
}

PangoGravity
rsvg_state_get_text_gravity (RsvgState *state)
{
    return state->text_gravity;
}

PangoWeight
rsvg_state_get_font_weight (RsvgState *state)
{
    return state->font_weight;
}

PangoStretch
rsvg_state_get_font_stretch (RsvgState *state)
{
    return state->font_stretch;
}

cairo_fill_rule_t
rsvg_state_get_clip_rule (RsvgState *state)
{
    return state->clip_rule;
}

RsvgPaintServer *
rsvg_state_get_fill (RsvgState *state)
{
    return state->fill;
}

guint8
rsvg_state_get_fill_opacity (RsvgState *state)
{
    return state->fill_opacity;
}

cairo_antialias_t
rsvg_state_get_shape_rendering_type (RsvgState *state)
{
    return state->shape_rendering_type;
}

cairo_antialias_t
rsvg_state_get_text_rendering_type (RsvgState *state)
{
    return state->text_rendering_type;
}

cairo_operator_t
rsvg_state_get_comp_op (RsvgState *state)
{
    return state->comp_op;
}

State *
rsvg_state_get_state_rust (RsvgState *state)
{
    return state->state_rust;
}
