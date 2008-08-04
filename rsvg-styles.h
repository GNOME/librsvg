/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 8; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

enum {
    FILL_RULE_EVENODD = 0,
    FILL_RULE_NONZERO = 1
};

typedef enum {
    UNICODE_BIDI_NORMAL = 0,
    UNICODE_BIDI_EMBED = 1,
    UNICODE_BIDI_OVERRIDE = 2
} UnicodeBidi;

typedef enum {
    SHAPE_RENDERING_AUTO = 0,
    SHAPE_RENDERING_OPTIMIZE_SPEED,
    SHAPE_RENDERING_CRISP_EDGES,
    SHAPE_RENDERING_GEOMETRIC_PRECISION
} ShapeRenderingProperty;

typedef enum {
    TEXT_RENDERING_AUTO = 0,
    TEXT_RENDERING_OPTIMIZE_SPEED,
    TEXT_RENDERING_OPTIMIZE_LEGIBILITY,
    TEXT_RENDERING_GEOMETRIC_PRECISION
} TextRenderingProperty;

typedef enum {
    RSVG_COMP_OP_CLEAR,
    RSVG_COMP_OP_SRC,
    RSVG_COMP_OP_DST,
    RSVG_COMP_OP_SRC_OVER,
    RSVG_COMP_OP_DST_OVER,
    RSVG_COMP_OP_SRC_IN,
    RSVG_COMP_OP_DST_IN,
    RSVG_COMP_OP_SRC_OUT,
    RSVG_COMP_OP_DST_OUT,
    RSVG_COMP_OP_SRC_ATOP,
    RSVG_COMP_OP_DST_ATOP,
    RSVG_COMP_OP_XOR,
    RSVG_COMP_OP_PLUS,
    RSVG_COMP_OP_MULTIPLY,
    RSVG_COMP_OP_SCREEN,
    RSVG_COMP_OP_OVERLAY,
    RSVG_COMP_OP_DARKEN,
    RSVG_COMP_OP_LIGHTEN,
    RSVG_COMP_OP_COLOR_DODGE,
    RSVG_COMP_OP_COLOR_BURN,
    RSVG_COMP_OP_HARD_LIGHT,
    RSVG_COMP_OP_SOFT_LIGHT,
    RSVG_COMP_OP_DIFFERENCE,
    RSVG_COMP_OP_EXCLUSION
} RsvgCompOpType;

typedef enum {
    RSVG_ENABLE_BACKGROUND_ACCUMULATE,
    RSVG_ENABLE_BACKGROUND_NEW
} RsvgEnableBackgroundType;

/* enums and data structures are ABI compatible with libart */

typedef enum {
    RSVG_PATH_STROKE_JOIN_MITER,
    RSVG_PATH_STROKE_JOIN_ROUND,
    RSVG_PATH_STROKE_JOIN_BEVEL
} RsvgPathStrokeJoinType;

typedef enum {
    RSVG_PATH_STROKE_CAP_BUTT,
    RSVG_PATH_STROKE_CAP_ROUND,
    RSVG_PATH_STROKE_CAP_SQUARE
} RsvgPathStrokeCapType;

typedef struct _RsvgVpathDash RsvgVpathDash;

struct _RsvgVpathDash {
    RsvgLength offset;
    int n_dash;
    double *dash;
};

/* end libart theft... */

struct _RsvgState {
    double affine[6];
    double personal_affine[6];

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

    RsvgPathStrokeCapType cap;
    gboolean has_cap;
    RsvgPathStrokeJoinType join;
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

    RsvgCompOpType comp_op;
    RsvgEnableBackgroundType enable_background;

    ShapeRenderingProperty shape_rendering_type;
    gboolean has_shape_rendering_type;
    
    TextRenderingProperty text_rendering_type;
    gboolean has_text_rendering_type;
};

RsvgState *rsvg_state_new ();

void rsvg_state_init	    (RsvgState * state);
void rsvg_state_clone	    (RsvgState * dst, const RsvgState * src);
void rsvg_state_inherit	    (RsvgState * dst, const RsvgState * src);
void rsvg_state_reinherit   (RsvgState * dst, const RsvgState * src);
void rsvg_state_dominate    (RsvgState * dst, const RsvgState * src);
void rsvg_state_override    (RsvgState * dst, const RsvgState * src);
void rsvg_state_finalize    (RsvgState * state);

void rsvg_parse_style_pairs (RsvgHandle * ctx, RsvgState * state, RsvgPropertyBag * atts);
void rsvg_parse_style_pair  (RsvgHandle * ctx, RsvgState * state, const char *key, const char *val);
void rsvg_parse_style	    (RsvgHandle * ctx, RsvgState * state, const char *str);
void rsvg_parse_cssbuffer   (RsvgHandle * ctx, const char *buff, size_t buflen);

void rsvg_parse_style_attrs (RsvgHandle * ctx, RsvgState * state, const char *tag,
                             const char *klazz, const char *id, RsvgPropertyBag * atts);

gdouble rsvg_viewport_percentage    (gdouble width, gdouble height);
gdouble rsvg_dpi_percentage	    (RsvgHandle * ctx);

gboolean rsvg_parse_transform	    (double dst[6], const char *src);

RsvgState *rsvg_state_parent	(RsvgDrawingCtx * ctx);
RsvgState *rsvg_state_current	(RsvgDrawingCtx * ctx);

void rsvg_state_pop	(RsvgDrawingCtx * ctx);
void rsvg_state_push	(RsvgDrawingCtx * ctx);

void rsvg_state_reinherit_top	(RsvgDrawingCtx * ctx, RsvgState * state, int dominate);

void rsvg_state_reconstruct	(RsvgState * state, RsvgNode * current);

G_END_DECLS

#endif                          /* RSVG_STYLES_H */
