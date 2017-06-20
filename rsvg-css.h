/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-css.h : CSS utility functions

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
#ifndef RSVG_CSS_H
#define RSVG_CSS_H

#include <glib.h>

#ifdef RSVG_COMPILATION
#include <pango/pango.h>
#include "rsvg-private.h"
#endif

G_BEGIN_DECLS

#define RSVG_ASPECT_RATIO_NONE (0)
#define RSVG_ASPECT_RATIO_XMIN_YMIN (1 << 0)
#define RSVG_ASPECT_RATIO_XMID_YMIN (1 << 1)
#define RSVG_ASPECT_RATIO_XMAX_YMIN (1 << 2)
#define RSVG_ASPECT_RATIO_XMIN_YMID (1 << 3)
#define RSVG_ASPECT_RATIO_XMID_YMID (1 << 4)
#define RSVG_ASPECT_RATIO_XMAX_YMID (1 << 5)
#define RSVG_ASPECT_RATIO_XMIN_YMAX (1 << 6)
#define RSVG_ASPECT_RATIO_XMID_YMAX (1 << 7)
#define RSVG_ASPECT_RATIO_XMAX_YMAX (1 << 8)
#define RSVG_ASPECT_RATIO_SLICE (1 << 30)
#define RSVG_ASPECT_RATIO_DEFER (1 << 31)

/* Keep this in sync with rust/src/color.rs:ColorKind */
typedef enum {
    RSVG_CSS_COLOR_SPEC_INHERIT,
    RSVG_CSS_COLOR_SPEC_CURRENT_COLOR,
    RSVG_CSS_COLOR_SPEC_ARGB,
    RSVG_CSS_COLOR_PARSE_ERROR
} RsvgCssColorKind;

/* Keep this in sync with rust/src/color.rs:RsvgCssColor */
typedef struct {
    RsvgCssColorKind kind;
    guint32 argb; /* only valid if kind == RSVG_CSS_COLOR_SPEC_ARGB */
} RsvgCssColorSpec;

typedef enum {
    ALLOW_INHERIT_NO,
    ALLOW_INHERIT_YES,
} AllowInherit;

typedef enum {
    ALLOW_CURRENT_COLOR_NO,
    ALLOW_CURRENT_COLOR_YES
} AllowCurrentColor;

/* This one is semi-public for mis-use in rsvg-convert */
RsvgCssColorSpec rsvg_css_parse_color_ (const char       *str,
                                        AllowInherit      allow_inherit,
                                        AllowCurrentColor allow_current_color);

#ifdef RSVG_COMPILATION

/* Implemented in rust/src/color.rs */
G_GNUC_INTERNAL
RsvgCssColorSpec rsvg_css_parse_color (const char       *str,
                                       AllowInherit      allow_inherit,
                                       AllowCurrentColor allow_current_color);

/* Keep this in sync with rust/src/opacity.rs:OpacityKind */
typedef enum {
    RSVG_OPACITY_INHERIT,
    RSVG_OPACITY_SPECIFIED,
    RSVG_OPACITY_PARSE_ERROR
} RsvgOpacityKind;

/* Keep this in sync with rust/src/opacity.rs:OpacitySpec */
typedef struct {
    RsvgOpacityKind kind;
    guint8 opacity; /* 0..255; only valid if kind == RSVG_OPACITY_SPECIFIED */
} RsvgOpacitySpec;

/* This is implemented in rust/src/opacity.rs */
G_GNUC_INTERNAL
RsvgOpacitySpec rsvg_css_parse_opacity (const char *str);

/* This is implemented in rust/src/aspect_ratio.rs */
G_GNUC_INTERNAL
guint32 rsvg_aspect_ratio_parse (const char *str);

/* This is implemented in rust/src/aspect_ratio.rs */
G_GNUC_INTERNAL
void rsvg_aspect_ratio_compute (guint32 aspect,
                                double object_width,
                                double object_height,
                                double *dest_x,
                                double *dest_y,
                                double *dest_width,
                                double *dest_height);

G_GNUC_INTERNAL
PangoStyle   rsvg_css_parse_font_style      (const char *str, gboolean * inherit);
G_GNUC_INTERNAL
PangoVariant rsvg_css_parse_font_variant    (const char *str, gboolean * inherit);
G_GNUC_INTERNAL
PangoWeight	 rsvg_css_parse_font_weight	    (const char *str, gboolean * inherit);
G_GNUC_INTERNAL
PangoStretch rsvg_css_parse_font_stretch    (const char *str, gboolean * inherit);
G_GNUC_INTERNAL
const char  *rsvg_css_parse_font_family	    (const char *str, gboolean * inherit);

/* Implemented in rust/src/parsers.rs */
G_GNUC_INTERNAL
gboolean rsvg_css_parse_number_optional_number (const char *str, double *out_x, double *out_y);

G_GNUC_INTERNAL
gchar       **rsvg_css_parse_list           (const char *in_str, guint * out_list_len);

/* Keep in sync with rust/src/parsers.rs:NumberListLength */
typedef enum {
    NUMBER_LIST_LENGTH_EXACT,
    NUMBER_LIST_LENGTH_MAXIMUM
} NumberListLength;

G_GNUC_INTERNAL
gboolean rsvg_css_parse_number_list (const char      *in_str,
                                     NumberListLength nlength,
                                     gsize            size,
                                     gdouble         **out_list,
                                     gsize           *out_list_len);

G_GNUC_INTERNAL
gboolean      rsvg_css_parse_overflow       (const char *str, gboolean * inherit);
G_GNUC_INTERNAL
char        **rsvg_css_parse_xml_attribute_string   (const char *attribute_string);

#endif /* RSVG_COMPILATION */

G_END_DECLS

#endif  
