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
rsvg_css_parse_length (const char *str, gdouble pixels_per_inch, gint *fixed)
{
  double length = 0.0;
  char *p = NULL;
  
  /* 
   *  The supported CSS length unit specifiers are: 
   *  em, ex, px, pt, pc, cm, mm, in, and percentages. 
   */

  length = g_ascii_strtod (str, &p);
  
  /* todo: error condition - figure out how to best represent it */
  if ((length == -HUGE_VAL || length == HUGE_VAL) && (ERANGE == errno))
    {
      *fixed = FALSE;
      return 0.0;
    }

  *fixed = TRUE;

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
      else if (!strcmp(p, "%"))
	{
	  *fixed = FALSE;
	  length *= 0.01;
	}
      /* todo: em, ex */
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
				 gdouble normalize_to)
{
  gint fixed = FALSE;

  double length = rsvg_css_parse_length (str, pixels_per_inch, &fixed);
  if (fixed)
    return length;

  /* length is a percent, normalize */
  return (length * normalize_to);
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

  return (gint)floor(25500.0 / (double)in_percent);
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

/* Parse a CSS2 color, returning rgb */
guint32
rsvg_css_parse_color (const char *str)
{
  gint val = 0;
  static GHashTable *colors = NULL;

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
  else if (strstr (str, "rgb") != 0)
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
      GString * string, * tmp;
      if (!colors)
	{
	  colors = g_hash_table_new (g_str_hash, g_str_equal);
	  
	  g_hash_table_insert (colors, "black",    GINT_TO_POINTER (0x000000));
	  g_hash_table_insert (colors, "silver",   GINT_TO_POINTER (0xc0c0c0));
	  g_hash_table_insert (colors, "gray",     GINT_TO_POINTER (0x808080));
	  g_hash_table_insert (colors, "white",    GINT_TO_POINTER (0xFFFFFF));
	  g_hash_table_insert (colors, "maroon",   GINT_TO_POINTER (0x800000));
	  g_hash_table_insert (colors, "red",      GINT_TO_POINTER (0xFF0000));
	  g_hash_table_insert (colors, "purple",   GINT_TO_POINTER (0x800080));
	  g_hash_table_insert (colors, "fuchsia",  GINT_TO_POINTER (0xFF00FF));
	  g_hash_table_insert (colors, "green",    GINT_TO_POINTER (0x008000));
	  g_hash_table_insert (colors, "lime",     GINT_TO_POINTER (0x00FF00));
	  g_hash_table_insert (colors, "olive",    GINT_TO_POINTER (0x808000));
	  g_hash_table_insert (colors, "yellow",   GINT_TO_POINTER (0xFFFF00));
	  g_hash_table_insert (colors, "navy",     GINT_TO_POINTER (0x000080));
	  g_hash_table_insert (colors, "blue",     GINT_TO_POINTER (0x0000FF));
	  g_hash_table_insert (colors, "teal",     GINT_TO_POINTER (0x008080));
	  g_hash_table_insert (colors, "aqua",     GINT_TO_POINTER (0x00FFFF));
	}

      tmp = g_string_new (str);
      string = g_string_ascii_down (tmp);

      /* this will default to black on a failed lookup */
      val = GPOINTER_TO_INT (g_hash_table_lookup (colors, string->str)); 

      g_string_free (tmp, TRUE);
      g_string_free (string, TRUE);
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

double
rsvg_css_parse_fontsize (const char *str)
{
  char *end_ptr;
  double size;

  /* todo: handle absolute-size and relative-size tags and proper units */
  /* todo: should this call rsvg_css_parse_length and then modify the return value? */
  size = g_ascii_strtod (str, &end_ptr);

  if (end_ptr && end_ptr[0] == '%')
    size = (36 * size * 0.01); /* todo: egregious hack */

  return size;
}
