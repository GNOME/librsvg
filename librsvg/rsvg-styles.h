/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-styles.h: Handle SVG styles

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

#ifndef RSVG_STYLES_H
#define RSVG_STYLES_H

#include <cairo.h>
#include "rsvg.h"
#include "rsvg-css.h"
#include "rsvg-paint-server.h"

#include <libxml/SAX.h>

G_BEGIN_DECLS 

/* Keep in sync with rust/src/state.rs:TextDecoration */
typedef struct {
    gboolean overline;
    gboolean underline;
    gboolean strike;
} TextDecoration;

typedef enum {
    TEXT_ANCHOR_START,
    TEXT_ANCHOR_MIDDLE,
    TEXT_ANCHOR_END
} TextAnchor;

/* Keep in sync with rust/src/state.c:UnicodeBidi */
typedef enum {
    UNICODE_BIDI_NORMAL = 0,
    UNICODE_BIDI_EMBED = 1,
    UNICODE_BIDI_OVERRIDE = 2
} UnicodeBidi;

typedef enum {
    RSVG_ENABLE_BACKGROUND_ACCUMULATE,
    RSVG_ENABLE_BACKGROUND_NEW
} RsvgEnableBackgroundType;

/* Opaque; defined in rsvg_internals/src/length.rs */
typedef struct RsvgStrokeDasharray RsvgStrokeDasharray;

struct _RsvgState {
    RsvgState *parent;
    cairo_matrix_t affine;
    cairo_matrix_t personal_affine;

    char *filter;
    char *mask;
    char *clip_path;
    guint8 opacity;             /* 0..255 */
    double baseline_shift;
    gboolean has_baseline_shift;

    RsvgPaintServer *fill;
    gboolean has_fill_server;
    guint8 fill_opacity;        /* 0..255 */
    gboolean has_fill_opacity;
    cairo_fill_rule_t fill_rule;
    gboolean has_fill_rule;
    cairo_fill_rule_t clip_rule;
    gboolean has_clip_rule;

    gboolean overflow;
    gboolean has_overflow;

    RsvgPaintServer *stroke;
    gboolean has_stroke_server;
    guint8 stroke_opacity;      /* 0..255 */
    gboolean has_stroke_opacity;
    RsvgLength stroke_width;
    gboolean has_stroke_width;
    double miter_limit;
    gboolean has_miter_limit;

    cairo_line_cap_t cap;
    gboolean has_cap;
    cairo_line_join_t join;
    gboolean has_join;

    RsvgLength font_size;
    gboolean has_font_size;
    char *font_family;
    gboolean has_font_family;
    char *lang;
    gboolean has_lang;
    PangoStyle font_style;
    gboolean has_font_style;
    PangoVariant font_variant;
    gboolean has_font_variant;
    PangoWeight font_weight;
    gboolean has_font_weight;
    PangoStretch font_stretch;
    gboolean has_font_stretch;
    TextDecoration font_decor;
    gboolean has_font_decor;
    PangoDirection text_dir;
    gboolean has_text_dir;
    PangoGravity text_gravity;
    gboolean has_text_gravity;
    UnicodeBidi unicode_bidi;
    gboolean has_unicode_bidi;
    TextAnchor text_anchor;
    gboolean has_text_anchor;
    RsvgLength letter_spacing;
    gboolean has_letter_spacing;

    guint text_offset;

    RsvgCssColorSpec stop_color;
    gboolean has_stop_color;

    RsvgOpacitySpec stop_opacity;
    gboolean has_stop_opacity;

    gboolean visible;
    gboolean has_visible;

    gboolean space_preserve;
    gboolean has_space_preserve;

    gboolean has_cond;
    gboolean cond_true;

    RsvgStrokeDasharray *dash;
    gboolean has_dash;
    RsvgLength dash_offset;
    gboolean has_dashoffset;

    guint32 current_color;
    gboolean has_current_color;

    guint32 flood_color;
    gboolean has_flood_color;

    guchar flood_opacity;
    gboolean has_flood_opacity;

    char *startMarker;
    char *middleMarker;
    char *endMarker;
    gboolean has_startMarker;
    gboolean has_middleMarker;
    gboolean has_endMarker;

    cairo_operator_t comp_op;
    RsvgEnableBackgroundType enable_background;

    cairo_antialias_t shape_rendering_type;
    gboolean has_shape_rendering_type;

    cairo_antialias_t text_rendering_type;
    gboolean has_text_rendering_type;

    GHashTable *styles;
};

G_GNUC_INTERNAL
RsvgState *rsvg_state_new (void);

G_GNUC_INTERNAL
void rsvg_state_free (RsvgState *state);

G_GNUC_INTERNAL
void rsvg_state_reinit      (RsvgState * state);
G_GNUC_INTERNAL
void rsvg_state_clone       (RsvgState * dst, const RsvgState * src);

G_GNUC_INTERNAL
void rsvg_state_free_all    (RsvgState * state);

G_GNUC_INTERNAL
void rsvg_parse_presentation_attributes (RsvgState * state, RsvgPropertyBag * atts);
G_GNUC_INTERNAL
void rsvg_parse_style	    (RsvgState *state, const char *str);
G_GNUC_INTERNAL
void rsvg_parse_cssbuffer   (RsvgHandle *handle, const char *buff, size_t buflen);
G_GNUC_INTERNAL
void rsvg_parse_style_attrs (RsvgHandle *handle, RsvgNode *node, const char *tag,
                             const char *klazz, const char *id, RsvgPropertyBag * atts);

/* Implemented in rust/src/transform.rs */
G_GNUC_INTERNAL
gboolean rsvg_parse_transform   (cairo_matrix_t *matrix, const char *src) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
RsvgState *rsvg_state_parent    (RsvgState * state);

G_GNUC_INTERNAL
void       rsvg_state_pop       (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void       rsvg_state_push      (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
RsvgState *rsvg_current_state   (RsvgDrawingCtx * ctx);

G_GNUC_INTERNAL
void rsvg_state_reinherit_top	(RsvgDrawingCtx * ctx, RsvgState * state, int dominate);

G_GNUC_INTERNAL
void rsvg_state_reconstruct	(RsvgState * state, RsvgNode * current);

G_GNUC_INTERNAL
cairo_matrix_t rsvg_state_get_affine (RsvgState *state);

G_GNUC_INTERNAL
gboolean rsvg_state_is_overflow (RsvgState *state);

G_GNUC_INTERNAL
gboolean rsvg_state_has_overflow (RsvgState *state);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_stroke (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_stroke_opacity (RsvgState *state);

G_GNUC_INTERNAL
RsvgLength rsvg_state_get_stroke_width (RsvgState *state);

G_GNUC_INTERNAL
double rsvg_state_get_miter_limit (RsvgState *state);

G_GNUC_INTERNAL
cairo_line_cap_t rsvg_state_get_line_cap (RsvgState *state);

G_GNUC_INTERNAL
cairo_line_join_t rsvg_state_get_line_join (RsvgState *state);

G_GNUC_INTERNAL
gboolean rsvg_state_get_cond_true (RsvgState *state);

G_GNUC_INTERNAL
void rsvg_state_set_cond_true (RsvgState *state, gboolean cond_true);

G_GNUC_INTERNAL
RsvgCssColorSpec *rsvg_state_get_stop_color (RsvgState *state);

G_GNUC_INTERNAL
RsvgOpacitySpec *rsvg_state_get_stop_opacity (RsvgState *state);

G_GNUC_INTERNAL
RsvgStrokeDasharray *rsvg_state_get_stroke_dasharray (RsvgState *state);

G_GNUC_INTERNAL
RsvgLength rsvg_state_get_dash_offset (RsvgState *state);

G_GNUC_INTERNAL
guint32 rsvg_state_get_current_color (RsvgState *state);

G_GNUC_INTERNAL
const char *rsvg_state_get_language (RsvgState *state);

G_GNUC_INTERNAL
UnicodeBidi rsvg_state_get_unicode_bidi (RsvgState *state);

G_GNUC_INTERNAL
PangoDirection rsvg_state_get_text_dir (RsvgState *state);

G_GNUC_INTERNAL
PangoGravity rsvg_state_get_text_gravity (RsvgState *state);

G_GNUC_INTERNAL
const char *rsvg_state_get_font_family (RsvgState *state);

G_GNUC_INTERNAL
PangoStyle rsvg_state_get_font_style (RsvgState *state);

G_GNUC_INTERNAL
PangoVariant rsvg_state_get_font_variant (RsvgState *state);

G_GNUC_INTERNAL
PangoWeight rsvg_state_get_font_weight (RsvgState *state);

G_GNUC_INTERNAL
PangoStretch rsvg_state_get_font_stretch (RsvgState *state);

G_GNUC_INTERNAL
RsvgLength rsvg_state_get_letter_spacing (RsvgState *state);

G_GNUC_INTERNAL
const TextDecoration *rsvg_state_get_font_decor (RsvgState *state);

G_GNUC_INTERNAL
cairo_fill_rule_t rsvg_state_get_clip_rule (RsvgState *state);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_fill (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_fill_opacity (RsvgState *state);

G_GNUC_INTERNAL
cairo_fill_rule_t rsvg_state_get_fill_rule (RsvgState *state);

G_GNUC_INTERNAL
cairo_antialias_t rsvg_state_get_shape_rendering_type (RsvgState *state);

G_GNUC_INTERNAL
cairo_antialias_t rsvg_state_get_text_rendering_type (RsvgState *state);

G_END_DECLS

#endif                          /* RSVG_STYLES_H */
