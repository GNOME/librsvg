/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

#include <glib/gtypes.h>
#include <pango/pangoft2.h>

G_BEGIN_DECLS

double
rsvg_css_parse_length (const char *str, gdouble pixels_per_inch, 
		       gint *percent, gint *em, gint *ex);

double
rsvg_css_parse_normalized_length(const char *str, gdouble pixels_per_inch,
								 gdouble width_or_height, gdouble font_size);

gboolean
rsvg_css_param_match (const char *str, const char *param_name);

int
rsvg_css_param_arg_offset (const char *str);

guint32
rsvg_css_parse_color (const char *str);

guint
rsvg_css_parse_opacity (const char *str);

double
rsvg_css_parse_angle (const char * str);

double
rsvg_css_parse_frequency (const char * str);

double
rsvg_css_parse_time (const char * str);

PangoStyle
rsvg_css_parse_font_style (const char * str, PangoStyle inherit);

PangoVariant
rsvg_css_parse_font_variant (const char * str, PangoVariant inherit);

PangoWeight
rsvg_css_parse_font_weight (const char * str, PangoWeight inherit);

PangoStretch
rsvg_css_parse_font_stretch (const char * str, PangoStretch inherit);

const char *
rsvg_css_parse_font_family (const char * str, const char * inherit);

gboolean
rsvg_css_parse_vbox (const char * vbox, double * x, double * y,
					 double * w, double * h);

void 
rsvg_css_parse_number_optional_number(const char * str, 
									  double *x, double *y);

gchar ** 
rsvg_css_parse_list(const char * in_str, guint * out_list_len);

gdouble *
rsvg_css_parse_number_list(const char * in_str, guint * out_list_len);

G_END_DECLS

#endif /* RSVG_CSS_H */
