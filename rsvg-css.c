/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-css.c: Parse CSS basic data types.
 
   Copyright (C) 2000 Eazel, Inc.
  
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
  
   Authors: Dom Lachowicz <cinamod@hotmail.com> 
   Raph Levien <raph@artofcode.com>
*/

#include "config.h"
#include "rsvg-css.h"

#include <glib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <errno.h>
#include <math.h>

#define POINTS_PER_INCH (72.0)
#define CM_PER_INCH     (2.54)
#define MM_PER_INCH     (25.4)
#define PICA_PER_INCH   (6.0)

#ifndef HAVE_STRTOK_R

static char *
strtok_r(char *s, const char *delim, char **last)
{
	char *p;

	if (s == NULL)
		s = *last;

	if (s == NULL)
		return NULL;

	while (*s && strchr (delim, *s))
		s++;

	if (*s == '\0') {
		*last = NULL;
		return NULL;
	}

	p = s;
	while (*p && !strchr (delim, *p))
		p++;

	if (*p == '\0')
		*last = NULL;
	else {
		*p = '\0';
		p++;
		*last = p;
	}
	
	return s;
}

#endif /* !HAVE_STRTOK_R */

/**
 * rsvg_css_parse_vbox
 * @vbox: The CSS viewBox
 * @x : The X output
 * @y: The Y output
 * @w: The Width output
 * @h: The Height output
 *
 * Returns: Success or failure
 */
gboolean
rsvg_css_parse_vbox (const char * vbox, double * x, double * y,
					 double * w, double * h)
{
	/* TODO: make me cleaner and more efficient */
	char *ptr, *tok;
	char *str = g_strdup (vbox);
	gboolean has_vbox = FALSE;
	
	tok = strtok_r (str, ", \t", &ptr);
	if (tok != NULL) {
		*x = g_ascii_strtod (tok, NULL);
		tok = strtok_r (NULL, ", \t", &ptr);
		if (tok != NULL) {
			*y = g_ascii_strtod (tok, NULL);
			tok = strtok_r (NULL, ", \t", &ptr);
			if (tok != NULL) {
				*w = g_ascii_strtod (tok, NULL);
				tok = strtok_r (NULL, ", \t", &ptr);
				if (tok != NULL) {
					*h = g_ascii_strtod (tok, NULL);
					has_vbox = TRUE;
				}
			}
		}
	}
	g_free (str);
	
	return has_vbox;
}

/**
 * rsvg_css_parse_length: Parse CSS2 length to a pixel value.
 * @str: Original string.
 * @pixels_per_inch: Pixels per inch
 * @fixed: Where to store boolean value of whether length is fixed.
 *
 * Parses a CSS2 length into a pixel value.
 *
 * Returns: returns the length.
 **/
double
rsvg_css_parse_length (const char *str, gdouble pixels_per_inch, 
					   gint *percent, gint *em, gint *ex)
{
	double length = 0.0;
	char *p = NULL;
	
	/* 
	 *  The supported CSS length unit specifiers are: 
	 *  em, ex, px, pt, pc, cm, mm, in, and %
	 */
	*percent = FALSE;
	*em      = FALSE;
	*ex      = FALSE;
	
	length = g_ascii_strtod (str, &p);
	
	/* todo: error condition - figure out how to best represent it */
	if ((length == -HUGE_VAL || length == HUGE_VAL) && (ERANGE == errno))
		return 0.0;
	
	/* test for either pixels or no unit, which is assumed to be pixels */
	if (p && (strcmp(p, "px") != 0))
		{
			if (!strcmp(p, "pt"))
				length *= (pixels_per_inch / POINTS_PER_INCH);
			else if (!strcmp(p, "in"))
				length *= pixels_per_inch;
			else if (!strcmp(p, "cm"))
				length *= (pixels_per_inch / CM_PER_INCH);
			else if (!strcmp(p, "mm"))
				length *= (pixels_per_inch / MM_PER_INCH);
			else if (!strcmp(p, "pc"))
				length *= (pixels_per_inch / PICA_PER_INCH);
			else if (!strcmp(p, "em"))
				*em = TRUE;
			else if (!strcmp(p, "ex"))
				*ex = TRUE;
			else if (!strcmp(p, "%"))
				{
					*percent = TRUE;
					length *= 0.01;
				}
		}
	
	return length;
}

/**
 * rsvg_css_parse_normalized_length: Parse CSS2 length to a pixel value.
 * @str: Original string.
 * @pixels_per_inch: Pixels per inch
 * @normalize_to: Bounding box's width or height, as appropriate, or 0
 *
 * Parses a CSS2 length into a pixel value.
 *
 * Returns: returns the length.
 */
double
rsvg_css_parse_normalized_length(const char *str, gdouble pixels_per_inch,
								 gdouble width_or_height, gdouble font_size)
{
	double length;
	gint percent, em, ex;
	percent = em = ex = FALSE;
	
	length = rsvg_css_parse_length (str, pixels_per_inch, &percent, &em, &ex);
	if (percent)
		return length * width_or_height;
	else if (em)
		return length * font_size;
	else if (ex)
		return length * font_size * 2.0; /* stolen from imagemagick */
	else
		return length;
}

gboolean
rsvg_css_param_match (const char *str, const char *param_name)
{
	int i;
	
	for (i = 0; str[i] != '\0' && str[i] != ':'; i++)
		if (param_name[i] != str[i])
			return FALSE;
	return str[i] == ':' && param_name[i] == '\0';
}

int
rsvg_css_param_arg_offset (const char *str)
{
	int i;
	
	for (i = 0; str[i] != '\0' && str[i] != ':'; i++);
	if (str[i] != '\0') i++;
	for (; str[i] == ' '; i++);
	return i;
}

static gint
rsvg_css_clip_rgb_percent (gint in_percent)
{
	/* spec says to clip these values */
	if (in_percent > 100)
		return 255;
	else if (in_percent <= 0)
		return 0;	
	return (gint)floor(255. * (double)in_percent / 100. + 0.5);
}

static gint
rsvg_css_clip_rgb (gint rgb)
{
	/* spec says to clip these values */
	if (rgb > 255)
		return 255;
	else if (rgb < 0)
		return 0;	
	return rgb;
}

typedef struct
{
	const char * name;
	guint rgb;
} ColorPair;

/* compare function callback for bsearch */
static int
rsvg_css_color_compare (const void * a, const void * b)
{
	const char * needle = (const char *)a;
	const ColorPair * haystack = (const ColorPair *)b;
	
	return g_ascii_strcasecmp (needle, haystack->name);
}

/* pack 3 [0,255] ints into one 32 bit one */
#define PACK_RGB(r,g,b) (((r) << 16) | ((g) << 8) | (b))

/**
 * Parse a CSS2 color specifier, return RGB value
 */
guint32
rsvg_css_parse_color (const char *str)
{
	gint val = 0;
	
	if (str[0] == '#')
		{
			int i;
			for (i = 1; str[i]; i++)
				{
					int hexval;
					if (str[i] >= '0' && str[i] <= '9')
						hexval = str[i] - '0';
					else if (str[i] >= 'A' && str[i] <= 'F')
						hexval = str[i] - 'A' + 10;
					else if (str[i] >= 'a' && str[i] <= 'f')
						hexval = str[i] - 'a' + 10;
					else
						break;
					val = (val << 4) + hexval;
				}
			/* handle #rgb case */
			if (i == 4)
				{
					val = ((val & 0xf00) << 8) |
						((val & 0x0f0) << 4) |
						(val & 0x00f);
					val |= val << 4;
				}
		}
	/* i want to use g_str_has_prefix but it isn't in my gstrfuncs.h?? */
	else if (strstr (str, "rgb") != NULL)
		{
			gint r, g, b;
			r = g = b = 0;
			
			if (strstr (str, "%") != 0)
				{
					/* assume "rgb (r%, g%, b%)" */
					if (3 == sscanf (str, " rgb ( %d %% , %d %% , %d %% ) ", &r, &g, &b))
						{
							r = rsvg_css_clip_rgb_percent (r);
							g = rsvg_css_clip_rgb_percent (g);
							b = rsvg_css_clip_rgb_percent (b);
						}
					else
						r = g = b = 0;
				}
			else
				{
					/* assume "rgb (r, g, b)" */
					if (3 == sscanf (str, " rgb ( %d , %d , %d ) ", &r, &g, &b))
						{
							r = rsvg_css_clip_rgb (r);
							g = rsvg_css_clip_rgb (g);
							b = rsvg_css_clip_rgb (b);
						}
					else
						r = g = b = 0;
				}
			
			val = PACK_RGB (r,g,b);
		}
	else
		{
			const static ColorPair color_list [] =
				{
					{ "aliceblue",            PACK_RGB (240,248,255) },
					{ "antiquewhite",         PACK_RGB (250,235,215) },
					{ "aqua",                 PACK_RGB (0,255,255) },
					{ "aquamarine",           PACK_RGB (127,255,212) },
					{ "azure",                PACK_RGB (240,255,255) },
					{ "beige",                PACK_RGB (245,245,220) },
					{ "bisque",               PACK_RGB (255,228,196) },
					{ "black",                PACK_RGB (0,0,0) },
					{ "blanchedalmond",       PACK_RGB (255,235,205) },
					{ "blue",                 PACK_RGB (0,0,255) },
					{ "blueviolet",           PACK_RGB (138,43,226) },
					{ "brown",                PACK_RGB (165,42,42) },
					{ "burlywood",            PACK_RGB (222,184,135) },
					{ "cadetblue",            PACK_RGB (95,158,160) },
					{ "chartreuse",           PACK_RGB (127,255,0) },
					{ "chocolate",            PACK_RGB (210,105,30) },
					{ "coral",                PACK_RGB (255,127,80) },
					{ "cornflowerblue",       PACK_RGB (100,149,237) },
					{ "cornsilk",             PACK_RGB (255,248,220) },
					{ "crimson",              PACK_RGB (220,20,60) },
					{ "cyan",                 PACK_RGB (0,255,255) },
					{ "darkblue",             PACK_RGB (0,0,139) },
					{ "darkcyan",             PACK_RGB (0,139,139) },
					{ "darkgoldenrod",        PACK_RGB (184,132,11) },
					{ "darkgray",             PACK_RGB (169,169,169) },
					{ "darkgreen",            PACK_RGB (0,100,0) },
					{ "darkgrey",             PACK_RGB (169,169,169) },
					{ "darkkhaki",            PACK_RGB (189,183,107) },
					{ "darkmagenta",          PACK_RGB (139,0,139) },
					{ "darkolivegreen",       PACK_RGB (85,107,47) },
					{ "darkorange",           PACK_RGB (255,140,0) },
					{ "darkorchid",           PACK_RGB (153,50,204) },
					{ "darkred",              PACK_RGB (139,0,0) },
					{ "darksalmon",           PACK_RGB (233,150,122) },
					{ "darkseagreen",         PACK_RGB (143,188,143) },
					{ "darkslateblue",        PACK_RGB (72,61,139) },
					{ "darkslategray",        PACK_RGB (47,79,79) },
					{ "darkslategrey",        PACK_RGB (47,79,79) },
					{ "darkturquoise",        PACK_RGB (0,206,209) },
					{ "darkviolet",           PACK_RGB (148,0,211) },
					{ "deeppink",             PACK_RGB (255,20,147) },
					{ "deepskyblue",          PACK_RGB (0,191,255) },
					{ "dimgray",              PACK_RGB (105,105,105) },
					{ "dimgrey",              PACK_RGB (105,105,105) },
					{ "dogerblue",            PACK_RGB (30,144,255) },
					{ "firebrick",            PACK_RGB (178,34,34) },
					{ "floralwhite" ,         PACK_RGB (255,255,240)},
					{ "forestgreen",          PACK_RGB (34,139,34) },
					{ "fuchsia",              PACK_RGB (255,0,255) },
					{ "gainsboro",            PACK_RGB (220,220,220) },
					{ "ghostwhite",           PACK_RGB (248,248,255) },
					{ "gold",                 PACK_RGB (255,215,0) },
					{ "goldenrod",            PACK_RGB (218,165,32) },
					{ "gray",                 PACK_RGB (128,128,128) },
					{ "grey",                 PACK_RGB (128,128,128) },
					{ "green",                PACK_RGB (0,128,0)},
					{ "greenyellow",          PACK_RGB (173,255,47) },
					{ "honeydew",             PACK_RGB (240,255,240) },
					{ "hotpink",              PACK_RGB (255,105,180) },
					{ "indianred",            PACK_RGB (205,92,92) },
					{ "indigo",               PACK_RGB (75,0,130) },
					{ "ivory",                PACK_RGB (255,255,240) },
					{ "khaki",                PACK_RGB (240,230,140) },
					{ "lavender",             PACK_RGB (230,230,250) },
					{ "lavenderblush",        PACK_RGB (255,240,245) },
					{ "lawngreen",            PACK_RGB (124,252,0) },
					{ "lemonchiffon",         PACK_RGB (255,250,205) },
					{ "lightblue",            PACK_RGB (173,216,230) },
					{ "lightcoral",           PACK_RGB (240,128,128) },
					{ "lightcyan",            PACK_RGB (224,255,255) },
					{ "lightgoldenrodyellow", PACK_RGB (250,250,210) },
					{ "lightgray",            PACK_RGB (211,211,211) },
					{ "lightgreen",           PACK_RGB (144,238,144) },
					{ "lightgrey",            PACK_RGB (211,211,211) },
					{ "lightpink",            PACK_RGB (255,182,193) },
					{ "lightsalmon",          PACK_RGB (255,160,122) },
					{ "lightseagreen",        PACK_RGB (32,178,170) },
					{ "lightskyblue",         PACK_RGB (135,206,250) },
					{ "lightslategray",       PACK_RGB (119,136,153) },
					{ "lightslategrey",       PACK_RGB (119,136,153) },
					{ "lightsteelblue",       PACK_RGB (176,196,222) },
					{ "lightyellow",          PACK_RGB (255,255,224) },
					{ "lime",                 PACK_RGB (0,255,0) },
					{ "limegreen",            PACK_RGB (50,205,50) },
					{ "linen",                PACK_RGB (250,240,230) },
					{ "magenta",              PACK_RGB (255,0,255) },
					{ "maroon",               PACK_RGB (128,0,0) },
					{ "mediumaquamarine",     PACK_RGB (102,205,170) },
					{ "mediumblue",           PACK_RGB (0,0,205) },
					{ "mediumorchid",         PACK_RGB (186,85,211) },
					{ "mediumpurple",         PACK_RGB (147,112,219) },
					{ "mediumseagreen",       PACK_RGB (60,179,113) },
					{ "mediumslateblue",      PACK_RGB (123,104,238) },
					{ "mediumspringgreen",    PACK_RGB (0,250,154) },
					{ "mediumturquoise",      PACK_RGB (72,209,204) },
					{ "mediumvioletred",      PACK_RGB (199,21,133) },
					{ "mediumnightblue",      PACK_RGB (25,25,112) },
					{ "mintcream",            PACK_RGB (245,255,250) },
					{ "mintyrose",            PACK_RGB (255,228,225) },
					{ "moccasin",             PACK_RGB (255,228,181) },
					{ "navajowhite",          PACK_RGB (255,222,173) },
					{ "navy",                 PACK_RGB (0,0,128) },
					{ "oldlace",              PACK_RGB (253,245,230) },
					{ "olive",                PACK_RGB (128,128,0) },
					{ "oliverab",             PACK_RGB (107,142,35) },
					{ "orange",               PACK_RGB (255,165,0) },
					{ "orangered",            PACK_RGB (255,69,0) },
					{ "orchid",               PACK_RGB (218,112,214) },
					{ "palegoldenrod",        PACK_RGB (238,232,170) },
					{ "palegreen",            PACK_RGB (152,251,152) },
					{ "paleturquoise",        PACK_RGB (175,238,238) },
					{ "palevioletred",        PACK_RGB (219,112,147) },
					{ "papayawhip",           PACK_RGB (255,239,213) },
					{ "peachpuff",            PACK_RGB (255,218,185) },
					{ "peru",                 PACK_RGB (205,133,63) },
					{ "pink",                 PACK_RGB (255,192,203) },
					{ "plum",                 PACK_RGB (221,160,203) },
					{ "powderblue",           PACK_RGB (176,224,230) },
					{ "purple",               PACK_RGB (128,0,128) },
					{ "red",                  PACK_RGB (255,0,0) },
					{ "rosybrown",            PACK_RGB (188,143,143) },
					{ "royalblue",            PACK_RGB (65,105,225) },
					{ "saddlebrown",          PACK_RGB (139,69,19) },
					{ "salmon",               PACK_RGB (250,128,114) },
					{ "sandybrown",           PACK_RGB (244,164,96) },
					{ "seagreen",             PACK_RGB (46,139,87) },
					{ "seashell",             PACK_RGB (255,245,238) },
					{ "sienna",               PACK_RGB (160,82,45) },
					{ "silver",               PACK_RGB (192,192,192) },
					{ "skyblue",              PACK_RGB (135,206,235) },
					{ "slateblue",            PACK_RGB (106,90,205) },
					{ "slategray",            PACK_RGB (112,128,144) },
					{ "slategrey",            PACK_RGB (112,128,114) },
					{ "snow",                 PACK_RGB (255,255,250) },
					{ "springgreen",          PACK_RGB (0,255,127) },
					{ "steelblue",            PACK_RGB (70,130,180) },
					{ "tan",                  PACK_RGB (210,180,140) },
					{ "teal",                 PACK_RGB (0,128,128) },
					{ "thistle",              PACK_RGB (216,191,216) },
					{ "tomato",               PACK_RGB (255,99,71) },
					{ "turquoise",            PACK_RGB (64,224,208) },
					{ "violet",               PACK_RGB (238,130,238) },
					{ "wheat",                PACK_RGB (245,222,179) },
					{ "white",                PACK_RGB (255,255,255) },
					{ "whitesmoke",           PACK_RGB (245,245,245) },
					{ "yellow",               PACK_RGB (255,255,0) },
					{ "yellowgreen",          PACK_RGB (154,205,50) }
				};
			
			ColorPair * result = bsearch (str, color_list, 
										  sizeof (color_list)/sizeof (color_list[0]),
										  sizeof (ColorPair),
										  rsvg_css_color_compare);
			
			/* default to black on failed lookup */
			if (result == NULL)
				val = 0;
			else
				val = result->rgb;
		}
	
	return val;
}

#undef PACK_RGB

guint
rsvg_css_parse_opacity (const char *str)
{
	char *end_ptr;
	double opacity;

	opacity = g_ascii_strtod (str, &end_ptr);
	
	if (end_ptr && end_ptr[0] == '%')
		opacity *= 0.01;
	
	return (guint)floor (opacity * 255. + 0.5);
}

/*
  <angle>: An angle value is a <number>  optionally followed immediately with 
  an angle unit identifier. Angle unit identifiers are:

    * deg: degrees
    * grad: grads
    * rad: radians

    For properties defined in [CSS2], an angle unit identifier must be provided.
    For SVG-specific attributes and properties, the angle unit identifier is 
    optional. If not provided, the angle value is assumed to be in degrees.
*/
double
rsvg_css_parse_angle (const char * str)
{
	double degrees;
	char *end_ptr;
	
	degrees = g_ascii_strtod (str, &end_ptr);
	
	/* todo: error condition - figure out how to best represent it */
	if ((degrees == -HUGE_VAL || degrees == HUGE_VAL) && (ERANGE == errno))
		return 0.0;
	
	if (end_ptr)
		{
			if (!strcmp(end_ptr, "rad"))
				return degrees * 180. / G_PI;
			else if (!strcmp(end_ptr, "grad"))
				return degrees * 360. / 400.;
		}
	
	return degrees;
}

/*
  <frequency>: Frequency values are used with aural properties. The normative 
  definition of frequency values can be found in [CSS2-AURAL]. A frequency 
  value is a <number> immediately followed by a frequency unit identifier. 
  Frequency unit identifiers are:

    * Hz: Hertz
    * kHz: kilo Hertz

    Frequency values may not be negative.
*/
double
rsvg_css_parse_frequency (const char * str)
{
	double f_hz;
	char *end_ptr;
	
	f_hz = g_ascii_strtod (str, &end_ptr);
	
	/* todo: error condition - figure out how to best represent it */
	if ((f_hz == -HUGE_VAL || f_hz == HUGE_VAL) && (ERANGE == errno))
		return 0.0;
	
	if (end_ptr && !strcmp(end_ptr, "kHz"))
		return f_hz * 1000.;
	
	return f_hz;
}

/*
  <time>: A time value is a <number> immediately followed by a time unit 
  identifier. Time unit identifiers are:
  
  * ms: milliseconds
  * s: seconds
  
  Time values are used in CSS properties and may not be negative.
*/
double
rsvg_css_parse_time (const char * str)
{
	double ms;
	char *end_ptr;
	
	ms = g_ascii_strtod (str, &end_ptr);
	
	/* todo: error condition - figure out how to best represent it */
	if ((ms == -HUGE_VAL || ms == HUGE_VAL) && (ERANGE == errno))
		return 0.0;
	
	if (end_ptr && !strcmp (end_ptr, "s"))
		return ms * 1000.;
	
	return ms;
}

PangoStyle
rsvg_css_parse_font_style (const char * str, PangoStyle inherit)
{
	if (str)
		{
			if (!strcmp(str, "oblique"))
				return PANGO_STYLE_OBLIQUE;
			if (!strcmp(str, "italic"))
				return PANGO_STYLE_ITALIC;
			else if (!strcmp(str, "inherit"))
				return inherit;
		}
	return PANGO_STYLE_NORMAL;
}

PangoVariant
rsvg_css_parse_font_variant (const char * str, PangoVariant inherit)
{
	if (str)
    {
		if (!strcmp(str, "small-caps"))
			return PANGO_VARIANT_SMALL_CAPS;
		else if (!strcmp(str, "inherit"))
			return inherit;
    }
	return PANGO_VARIANT_NORMAL;
}

PangoWeight
rsvg_css_parse_font_weight (const char * str, PangoWeight inherit)
{
	if (str)
		{
			if (!strcmp (str, "lighter"))
				return PANGO_WEIGHT_LIGHT;
			else if (!strcmp (str, "bold"))
				return PANGO_WEIGHT_BOLD;
			else if (!strcmp (str, "bolder"))
				return PANGO_WEIGHT_ULTRABOLD;
			else if (!strcmp (str, "100"))
				return (PangoWeight)100;
			else if (!strcmp (str, "200"))
				return (PangoWeight)200;
			else if (!strcmp (str, "300"))
				return (PangoWeight)300;
			else if (!strcmp (str, "400"))
				return (PangoWeight)400;
			else if (!strcmp (str, "500"))
				return (PangoWeight)500;
			else if (!strcmp (str, "600"))
				return (PangoWeight)600;
			else if (!strcmp (str, "700"))
				return (PangoWeight)700;
			else if (!strcmp (str, "800"))
				return (PangoWeight)800;
			else if (!strcmp (str, "900"))
				return (PangoWeight)900;
			else if (!strcmp(str, "inherit"))
				return inherit;
		}
	
	return PANGO_WEIGHT_NORMAL; 
}

PangoStretch
rsvg_css_parse_font_stretch (const char * str, PangoStretch inherit)
{
	if (str)
		{
			if (!strcmp (str, "ultra-condensed"))
				return PANGO_STRETCH_ULTRA_CONDENSED;
			else if (!strcmp (str, "extra-condensed"))
				return PANGO_STRETCH_EXTRA_CONDENSED;
			else if (!strcmp (str, "condensed") || !strcmp (str, "narrower")) /* narrower not quite correct */
				return PANGO_STRETCH_CONDENSED;
			else if (!strcmp (str, "semi-condensed"))
				return PANGO_STRETCH_SEMI_CONDENSED;
			else if (!strcmp (str, "semi-expanded"))
				return PANGO_STRETCH_SEMI_EXPANDED;
			else if (!strcmp (str, "expanded") || !strcmp (str, "wider")) /* wider not quite correct */
				return PANGO_STRETCH_EXPANDED;
			else if (!strcmp (str, "extra-expanded"))
				return PANGO_STRETCH_EXTRA_EXPANDED;
			else if (!strcmp (str, "ultra-expanded"))
				return PANGO_STRETCH_ULTRA_EXPANDED;
			else if (!strcmp(str, "inherit"))
				return inherit;
		}
	return PANGO_STRETCH_NORMAL;
}

const char *
rsvg_css_parse_font_family (const char * str, const char * inherit)
{
	if (!str)
		return NULL;	
	else if (!strcmp (str, "inherit"))
		return inherit;
	else
		return str;
}
