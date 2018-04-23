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

/* Keep in sync with rust/src/state.c:EnableBackgroundC */
typedef enum {
    RSVG_ENABLE_BACKGROUND_ACCUMULATE,
    RSVG_ENABLE_BACKGROUND_NEW
} RsvgEnableBackgroundType;

/* Opaque; defined in rsvg_internals/src/state.rs */
typedef struct State State;

struct _RsvgState {
    RsvgState *parent;

    guint8 opacity;             /* 0..255 */

    RsvgPaintServer *fill;
    gboolean has_fill_server;

    RsvgPaintServer *stroke;
    gboolean has_stroke_server;

    RsvgCssColorSpec stop_color;
    gboolean has_stop_color;

    RsvgOpacitySpec stop_opacity;
    gboolean has_stop_opacity;

    guint32 current_color;
    gboolean has_current_color;

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
gboolean rsvg_state_is_visible (RsvgState *state);

G_GNUC_INTERNAL
char *rsvg_state_get_clip_path (RsvgState *state);

G_GNUC_INTERNAL
char *rsvg_state_get_filter (RsvgState *state);

G_GNUC_INTERNAL
char *rsvg_state_get_mask (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_opacity (RsvgState *state);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_stroke (RsvgState *state);

G_GNUC_INTERNAL
RsvgCssColorSpec *rsvg_state_get_stop_color (RsvgState *state);

G_GNUC_INTERNAL
RsvgOpacitySpec *rsvg_state_get_stop_opacity (RsvgState *state);

G_GNUC_INTERNAL
guint32 rsvg_state_get_current_color (RsvgState *state);

G_GNUC_INTERNAL
RsvgPaintServer *rsvg_state_get_fill (RsvgState *state);

G_GNUC_INTERNAL
guint32 rsvg_state_get_flood_color (RsvgState *state);

G_GNUC_INTERNAL
guint8 rsvg_state_get_flood_opacity (RsvgState *state);

G_GNUC_INTERNAL
cairo_operator_t rsvg_state_get_comp_op (RsvgState *state);

G_GNUC_INTERNAL
RsvgEnableBackgroundType rsvg_state_get_enable_background (RsvgState *state);

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
