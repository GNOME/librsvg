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
  
   Author: Raph Levien <raph@artofcode.com>
*/

#include "config.h"
#include "rsvg-css.h"

#include <glib.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <stdio.h>

#define POINTS_PER_INCH (72.0)
#define CM_PER_INCH     (2.54)
#define MM_PER_INCH     (25.4)
#define PICA_PER_INCH   (6.0)

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
    {
      return 0.0;
    }

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
				 gdouble width_or_height, gdouble font_size,
				 gdouble x_height)
{
  gint percent, em, ex;
  percent = em = ex = FALSE;

  double length = rsvg_css_parse_length (str, pixels_per_inch, &percent, &em, &ex);
  if (percent)
    return length * width_or_height;
  else if (em)
    return length * font_size;
  else if (ex)
    return length * x_height;
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
    in_percent = 100;
  else if (in_percent <= 0)
    return 0;

  return (gint)floor(255. * (double)in_percent / 100.);
}

static gint
rsvg_css_clip_rgb (gint rgb)
{
  /* spec says to clip these values */
  if (rgb > 255)
    rgb = 255;
  else if (rgb < 0)
    rgb = 0;

  return rgb;
}

typedef struct
{
  const char * name;
  guint rgb;
} ColorPair;

/* compare function for bsearch */
static int
rsvg_css_color_compare (const void * a, const void * b)
{
  const char * needle = (const char *)a;
  const ColorPair * haystack = (const ColorPair *)b;

  return g_ascii_strcasecmp (needle, haystack->name);
}

/* Parse a CSS2 color, returning rgb */
guint32
rsvg_css_parse_color (const char *str)
{
  gint val = 0;

  /* todo: better failure detection */

#ifdef VERBOSE
  g_print ("color = %s\n", str);
#endif
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
#ifdef VERBOSE
      printf ("val = %x\n", val);
#endif
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

      val =  ((r << 16) | (g << 8) | (b));
    }
  else
    {
      const static ColorPair color_list [] =
	{
	  {"aqua",    0x00FFFF},
	  {"black",   0x000000},
	  {"blue",    0x0000FF},
	  {"fuchsia", 0xFF00FF},
	  {"gray",    0x808080},
	  {"green",   0x008000},
	  {"lime",    0x00FF00},
	  {"maroon",  0x800000},
	  {"navy",    0x000080},
	  {"olive",   0x808000},
	  {"purple",  0x800080},
	  {"red",     0xFF0000},
	  {"silver",  0xC0C0C0},
	  {"teal",    0x008080},
	  {"white",   0xFFFFFF},
	  {"yellow",  0xFFFF00}
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

guint
rsvg_css_parse_opacity (const char *str)
{
  char *end_ptr;
  double opacity;

  opacity = g_ascii_strtod (str, &end_ptr);

  if (end_ptr && end_ptr[0] == '%')
    opacity *= 0.01;

  return floor (opacity * 255 + 0.5);
}
