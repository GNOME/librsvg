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
#include <pango/pangoft2.h>

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

typedef struct {
	double affine[6];
	
	gint opacity; /* 0..255 */
	
	RsvgPaintServer *fill;
	gint fill_opacity; /* 0..255 */
	
	RsvgPaintServer *stroke;
	gint stroke_opacity; /* 0..255 */
	double stroke_width;
	double miter_limit;
	
	ArtPathStrokeCapType cap;
	ArtPathStrokeJoinType join;
	
	double         font_size;
	char         * font_family;
	char         * lang;
	PangoStyle     font_style;
	PangoVariant   font_variant;
	PangoWeight    font_weight;
	PangoStretch   font_stretch;
	TextDecoration font_decor;
	PangoDirection text_dir;
	TextAnchor     text_anchor;	

	guint text_offset;
	
	guint32 stop_color; /* rgb */
	gint stop_opacity;  /* 0..255 */
	
	gboolean visible;

	ArtVpathDash dash;
	
	GdkPixbuf *save_pixbuf;
} RsvgState;

void rsvg_state_init (RsvgState *state);
void rsvg_state_clone (RsvgState *dst, const RsvgState *src);
void rsvg_state_finalize (RsvgState *state);

gboolean rsvg_is_style_arg(const char *str);
void rsvg_parse_style_pair (RsvgHandle *ctx, RsvgState *state, 
							const char *key, const char *val);
void rsvg_parse_style (RsvgHandle *ctx, RsvgState *state, const char *str);
void rsvg_parse_cssbuffer (RsvgHandle *ctx, const char * buff, size_t buflen);

void rsvg_parse_style_attrs (RsvgHandle *ctx, RsvgState *state, const char * tag,
							 const char * klazz, const char * id,
							 const xmlChar **atts);

gdouble rsvg_viewport_percentage (gdouble width, gdouble height);
void rsvg_pop_opacity_group (RsvgHandle *ctx, int opacity);
void rsvg_push_opacity_group (RsvgHandle *ctx);
gboolean rsvg_parse_transform (double dst[6], const char *src);

RsvgState * rsvg_state_current (RsvgHandle *ctx);

G_END_DECLS

#endif /* RSVG_STYLES_H */
