/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
#include <libart_lgpl/art_svp_vpath_stroke.h>
#include <libart_lgpl/art_vpath_dash.h>
#include <pango/pango.h>

G_BEGIN_DECLS

typedef int TextDecoration;

enum {
	TEXT_NORMAL    = 0x00,
	TEXT_OVERLINE  = 0x01,
	TEXT_UNDERLINE = 0x02,
	TEXT_STRIKE    = 0x04
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

struct _RsvgState {
	double affine[6];
	double personal_affine[6];
	
	guint8 opacity; /* 0..255 */
	
	RsvgPaintServer *fill;
	int has_fill_server : 1;
	guint8 fill_opacity; /* 0..255 */
	int has_fill_opacity : 1;
	gint fill_rule;	
	int has_fill_rule : 1;
	gint clip_rule;	
	int has_clip_rule : 1;

	RsvgFilter *filter;
	void *mask;
	void *clip_path_ref;
	int backgroundnew : 1;
	guint8 adobe_blend; /* 0..11 */

	RsvgPaintServer *stroke;
	int has_stroke_server : 1;
	guint8 stroke_opacity; /* 0..255 */
	int has_stroke_opacity : 1;
	double stroke_width;
	int has_stroke_width : 1;
	double miter_limit;
	int has_miter_limit : 1;
	
	ArtPathStrokeCapType cap;
	int has_cap : 1;
	ArtPathStrokeJoinType join;
	int has_join : 1;
	
	double         font_size;
	int has_font_size : 1;
	char         * font_family;
	int has_font_family : 1;
	char         * lang;
	int has_lang : 1;
	PangoStyle     font_style;
	int has_font_style : 1;
	PangoVariant   font_variant;
	int has_font_variant : 1;
	PangoWeight    font_weight;
	int has_font_weight : 1;
	PangoStretch   font_stretch;
	int has_font_stretch : 1;
	TextDecoration font_decor;
	int has_font_decor : 1;
	PangoDirection text_dir;
	int has_text_dir : 1;
	UnicodeBidi unicode_bidi;
	int has_unicode_bidi : 1;
	TextAnchor     text_anchor;
	int has_text_anchor : 1;	

	guint text_offset;
	
	guint32 stop_color; /* rgb */
	int has_stop_color : 1;
	gint stop_opacity;  /* 0..255 */
	int has_stop_opacity : 1;
	
	int visible : 1;
	int has_visible : 1;

	int has_cond : 1;
	int cond_true : 1;

	ArtVpathDash dash;
	int has_dash : 1;

	guint32 current_color;
	int has_current_color : 1;

	RsvgDefVal * startMarker;
	RsvgDefVal * middleMarker;
	RsvgDefVal * endMarker;	
	int has_startMarker : 1;
	int has_middleMarker : 1;
	int has_endMarker : 1;	

	GdkPixbuf *save_pixbuf;
	ArtIRect underbbox;

	ArtSVP * clippath;
	int clip_path_loaded : 1;
};

void rsvg_state_init (RsvgState *state);
void rsvg_state_clone (RsvgState *dst, const RsvgState *src);
void rsvg_state_inherit (RsvgState *dst, const RsvgState *src);
void rsvg_state_reinherit (RsvgState *dst, const RsvgState *src);
void rsvg_state_dominate (RsvgState *dst, const RsvgState *src);
void rsvg_state_finalize (RsvgState *state);

void rsvg_parse_style_pairs (RsvgHandle *ctx, RsvgState *state, 
							 RsvgPropertyBag *atts);
void rsvg_parse_style_pair (RsvgHandle *ctx, RsvgState *state, 
							const char *key, const char *val);
void rsvg_parse_style (RsvgHandle *ctx, RsvgState *state, const char *str);
void rsvg_parse_cssbuffer (RsvgHandle *ctx, const char * buff, size_t buflen);

void rsvg_parse_style_attrs (RsvgHandle *ctx, RsvgState *state, const char * tag,
							 const char * klazz, const char * id,
							 RsvgPropertyBag *atts);

gdouble rsvg_viewport_percentage (gdouble width, gdouble height);
gdouble rsvg_dpi_percentage (RsvgHandle * ctx);

void rsvg_pop_discrete_layer(RsvgHandle *ctx);
void rsvg_push_discrete_layer (RsvgHandle *ctx);
gboolean rsvg_needs_discrete_layer(RsvgState *state);
gboolean rsvg_parse_transform (double dst[6], const char *src);

RsvgState * rsvg_state_parent (RsvgHandle *ctx);
RsvgState * rsvg_state_current (RsvgHandle *ctx);
double rsvg_state_current_font_size (RsvgHandle *ctx);

void rsvg_state_clip_path_assure(RsvgHandle * ctx);

void rsvg_state_pop(RsvgHandle * ctx);
void rsvg_state_push(RsvgHandle * ctx);

void rsvg_state_reinherit_top(RsvgHandle * ctx, RsvgState * state, int dominate);

G_END_DECLS

#endif /* RSVG_STYLES_H */
