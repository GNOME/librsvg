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

typedef enum {
    RSVG_ENABLE_BACKGROUND_ACCUMULATE,
    RSVG_ENABLE_BACKGROUND_NEW
} RsvgEnableBackgroundType;

/* Opaque; defined in rsvg_internals/src/length.rs */
typedef struct RsvgStrokeDasharray RsvgStrokeDasharray;

/* Opaque; defined in rsvg_internals/src/state.rs */
typedef struct State State;

struct _RsvgState {
    RsvgState *parent;

    char *filter;
    char *mask;
    char *clip_path;
    guint8 opacity;             /* 0..255 */

    RsvgPaintServer *fill;
    gboolean has_fill_server;
    guint8 fill_opacity;        /* 0..255 */
    gboolean has_fill_opacity;
    cairo_fill_rule_t clip_rule;
    gboolean has_clip_rule;

    RsvgPaintServer *stroke;
    gboolean has_stroke_server;
    guint8 stroke_opacity;      /* 0..255 */
    gboolean has_stroke_opacity;
    RsvgLength stroke_width;
    gboolean has_stroke_width;
    double miter_limit;
    gboolean has_miter_limit;

    PangoWeight font_weight;
    gboolean has_font_weight;
    PangoStretch font_stretch;
    gboolean has_font_stretch;
    PangoDirection text_dir;
    gboolean has_text_dir;
    PangoGravity text_gravity;
    gboolean has_text_gravity;

    guint text_offset;

    RsvgCssColorSpec stop_color;
    gboolean has_stop_color;

    RsvgOpacitySpec stop_opacity;
    gboolean has_stop_opacity;

    gboolean visible;
    gboolean has_visible;

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

    State *state_rust;
};

G_GNUC_INTERNAL
RsvgState *rsvg_state_new (void);

G_GNUC_INTERNAL
RsvgState *rsvg_state_new_with_parent (RsvgState *parent);

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
gboolean rsvg_parse_style_attribute_contents (RsvgState *state, const char *str) G_GNUC_WARN_UNUSED_RESULT;
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

/* Implemented in rust/src/state.rs */
G_GNUC_INTERNAL
void rsvg_state_reconstruct (RsvgState *state, RsvgNode *current);

G_GNUC_INTERNAL
cairo_matrix_t rsvg_state_get_affine (const RsvgState *state);

G_GNUC_INTERNAL
void rsvg_state_set_affine (RsvgState *state, cairo_matrix_t affine);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_stroke (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_stroke_opacity (RsvgState *state);

G_GNUC_INTERNAL
RsvgLength rsvg_state_get_stroke_width (RsvgState *state);

G_GNUC_INTERNAL
double rsvg_state_get_miter_limit (RsvgState *state);

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
PangoDirection rsvg_state_get_text_dir (RsvgState *state);

G_GNUC_INTERNAL
PangoGravity rsvg_state_get_text_gravity (RsvgState *state);

G_GNUC_INTERNAL
PangoWeight rsvg_state_get_font_weight (RsvgState *state);

G_GNUC_INTERNAL
PangoStretch rsvg_state_get_font_stretch (RsvgState *state);

G_GNUC_INTERNAL
cairo_fill_rule_t rsvg_state_get_clip_rule (RsvgState *state);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_fill (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_fill_opacity (RsvgState *state);

G_GNUC_INTERNAL
cairo_antialias_t rsvg_state_get_shape_rendering_type (RsvgState *state);

G_GNUC_INTERNAL
cairo_antialias_t rsvg_state_get_text_rendering_type (RsvgState *state);

G_GNUC_INTERNAL
cairo_operator_t rsvg_state_get_comp_op (RsvgState *state);

G_GNUC_INTERNAL
void rsvg_state_dominate (RsvgState *state, const RsvgState *src);

G_GNUC_INTERNAL
void rsvg_state_force (RsvgState *state, const RsvgState *src);

G_GNUC_INTERNAL
void rsvg_state_inherit (RsvgState *state, const RsvgState *src);

G_GNUC_INTERNAL
void rsvg_state_reinherit (RsvgState *state, const RsvgState *src);

G_GNUC_INTERNAL
State *rsvg_state_get_state_rust (RsvgState *state);

G_END_DECLS

#endif                          /* RSVG_STYLES_H */
