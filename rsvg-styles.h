/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
#include "rsvg-paint-server.h"

#include <libxml/SAX.h>
#include <pango/pango.h>

G_BEGIN_DECLS 

typedef int TextDecoration;

enum {
    TEXT_NORMAL = 0x00,
    TEXT_OVERLINE = 0x01,
    TEXT_UNDERLINE = 0x02,
    TEXT_STRIKE = 0x04
};

typedef enum {
    TEXT_ANCHOR_START,
    TEXT_ANCHOR_MIDDLE,
    TEXT_ANCHOR_END
} TextAnchor;

typedef enum {
    UNICODE_BIDI_NORMAL = 0,
    UNICODE_BIDI_EMBED = 1,
    UNICODE_BIDI_OVERRIDE = 2
} UnicodeBidi;

typedef enum {
    RSVG_ENABLE_BACKGROUND_ACCUMULATE,
    RSVG_ENABLE_BACKGROUND_NEW
} RsvgEnableBackgroundType;

/* enums and data structures are ABI compatible with libart */

typedef struct _RsvgVpathDash RsvgVpathDash;

struct _RsvgVpathDash {
    RsvgLength offset;
    int n_dash;
    double *dash;
};

/* end libart theft... */

struct _RsvgState {
    RsvgState *parent;
    cairo_matrix_t affine;
    cairo_matrix_t personal_affine;

    RsvgFilter *filter;
    void *mask;
    void *clip_path_ref;
    guint8 adobe_blend;         /* 0..11 */
    guint8 opacity;             /* 0..255 */

    RsvgPaintServer *fill;
    gboolean has_fill_server;
    guint8 fill_opacity;        /* 0..255 */
    gboolean has_fill_opacity;
    gint fill_rule;
    gboolean has_fill_rule;
    gint clip_rule;
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

    guint32 stop_color;         /* rgb */
    gboolean has_stop_color;
    gint stop_opacity;          /* 0..255 */
    gboolean has_stop_opacity;

    gboolean visible;
    gboolean has_visible;

    gboolean space_preserve;
    gboolean has_space_preserve;

    gboolean has_cond;
    gboolean cond_true;

    RsvgVpathDash dash;
    gboolean has_dash;
    gboolean has_dashoffset;

    guint32 current_color;
    gboolean has_current_color;

    guint32 flood_color;
    gboolean has_flood_color;

    guchar flood_opacity;
    gboolean has_flood_opacity;

    RsvgNode *startMarker;
    RsvgNode *middleMarker;
    RsvgNode *endMarker;
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
void rsvg_state_init        (RsvgState * state);
G_GNUC_INTERNAL
void rsvg_state_reinit      (RsvgState * state);
G_GNUC_INTERNAL
void rsvg_state_clone       (RsvgState * dst, const RsvgState * src);
G_GNUC_INTERNAL
void rsvg_state_inherit     (RsvgState * dst, const RsvgState * src);
G_GNUC_INTERNAL
void rsvg_state_reinherit   (RsvgState * dst, const RsvgState * src);
G_GNUC_INTERNAL
void rsvg_state_dominate    (RsvgState * dst, const RsvgState * src);
G_GNUC_INTERNAL
void rsvg_state_override    (RsvgState * dst, const RsvgState * src);
G_GNUC_INTERNAL
void rsvg_state_finalize    (RsvgState * state);
G_GNUC_INTERNAL
void rsvg_state_free_all    (RsvgState * state);

G_GNUC_INTERNAL
void rsvg_parse_style_pairs (RsvgHandle * ctx, RsvgState * state, RsvgPropertyBag * atts);
G_GNUC_INTERNAL
void rsvg_parse_style	    (RsvgHandle * ctx, RsvgState * state, const char *str);
G_GNUC_INTERNAL
void rsvg_parse_cssbuffer   (RsvgHandle * ctx, const char *buff, size_t buflen);
G_GNUC_INTERNAL
void rsvg_parse_style_attrs (RsvgHandle * ctx, RsvgState * state, const char *tag,
                             const char *klazz, const char *id, RsvgPropertyBag * atts);

G_GNUC_INTERNAL
gdouble rsvg_viewport_percentage (gdouble width, gdouble height);
G_GNUC_INTERNAL
gdouble rsvg_dpi_percentage      (RsvgHandle * ctx);

G_GNUC_INTERNAL
gboolean rsvg_parse_transform   (cairo_matrix_t *matrix, const char *src);

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

G_END_DECLS

#endif                          /* RSVG_STYLES_H */
