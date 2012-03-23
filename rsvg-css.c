/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
#include "rsvg-private.h"
#include "rsvg-styles.h"

#include <glib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef HAVE_STRINGS_H
#include <strings.h>
#endif
#include <errno.h>
#include <math.h>

#include <libxml/parser.h>

#include <libcroco/libcroco.h>

#define POINTS_PER_INCH (72.0)
#define CM_PER_INCH     (2.54)
#define MM_PER_INCH     (25.4)
#define PICA_PER_INCH   (6.0)

#define SETINHERIT() G_STMT_START {if (inherit != NULL) *inherit = TRUE;} G_STMT_END
#define UNSETINHERIT() G_STMT_START {if (inherit != NULL) *inherit = FALSE;} G_STMT_END

/**
 * rsvg_css_parse_vbox
 * @vbox: The CSS viewBox
 * @x : The X output
 * @y: The Y output
 * @w: The Width output
 * @h: The Height output
 *
 * Returns: 
 */
RsvgViewBox
rsvg_css_parse_vbox (const char *vbox)
{
    RsvgViewBox vb;
    gdouble *list;
    guint list_len;
    vb.active = FALSE;

    vb.rect.x = vb.rect.y = 0;
    vb.rect.width = vb.rect.height = 0;

    list = rsvg_css_parse_number_list (vbox, &list_len);

    if (!(list && list_len))
        return vb;
    else if (list_len != 4) {
        g_free (list);
        return vb;
    } else {
        vb.rect.x = list[0];
        vb.rect.y = list[1];
        vb.rect.width = list[2];
        vb.rect.height = list[3];
        vb.active = TRUE;

        g_free (list);
        return vb;
    }
}

typedef enum _RelativeSize {
    RELATIVE_SIZE_NORMAL,
    RELATIVE_SIZE_SMALLER,
    RELATIVE_SIZE_LARGER
} RelativeSize;

static double
rsvg_css_parse_raw_length (const char *str, gboolean * in,
                           gboolean * percent, gboolean * em, gboolean * ex, RelativeSize * relative_size)
{
    double length = 0.0;
    char *p = NULL;

    /* 
     *  The supported CSS length unit specifiers are: 
     *  em, ex, px, pt, pc, cm, mm, in, and %
     */
    *percent = FALSE;
    *em = FALSE;
    *ex = FALSE;
    *relative_size = RELATIVE_SIZE_NORMAL;

    length = g_ascii_strtod (str, &p);

    if ((length == -HUGE_VAL || length == HUGE_VAL) && (ERANGE == errno)) {
        /* todo: error condition - figure out how to best represent it */
        return 0.0;
    }

    /* test for either pixels or no unit, which is assumed to be pixels */
    if (p && *p && (strcmp (p, "px") != 0)) {
        if (!strcmp (p, "pt")) {
            length /= POINTS_PER_INCH;
            *in = TRUE;
        } else if (!strcmp (p, "in"))
            *in = TRUE;
        else if (!strcmp (p, "cm")) {
            length /= CM_PER_INCH;
            *in = TRUE;
        } else if (!strcmp (p, "mm")) {
            length /= MM_PER_INCH;
            *in = TRUE;
        } else if (!strcmp (p, "pc")) {
            length /= PICA_PER_INCH;
            *in = TRUE;
        } else if (!strcmp (p, "em"))
            *em = TRUE;
        else if (!strcmp (p, "ex"))
            *ex = TRUE;
        else if (!strcmp (p, "%")) {
            *percent = TRUE;
            length *= 0.01;
        } else {
            double pow_factor = 0.0;

            if (!g_ascii_strcasecmp (p, "larger")) {
                *relative_size = RELATIVE_SIZE_LARGER;
                return 0.0;
            } else if (!g_ascii_strcasecmp (p, "smaller")) {
                *relative_size = RELATIVE_SIZE_SMALLER;
                return 0.0;
            } else if (!g_ascii_strcasecmp (p, "xx-small")) {
                pow_factor = -3.0;
            } else if (!g_ascii_strcasecmp (p, "x-small")) {
                pow_factor = -2.0;
            } else if (!g_ascii_strcasecmp (p, "small")) {
                pow_factor = -1.0;
            } else if (!g_ascii_strcasecmp (p, "medium")) {
                pow_factor = 0.0;
            } else if (!g_ascii_strcasecmp (p, "large")) {
                pow_factor = 1.0;
            } else if (!g_ascii_strcasecmp (p, "x-large")) {
                pow_factor = 2.0;
            } else if (!g_ascii_strcasecmp (p, "xx-large")) {
                pow_factor = 3.0;
            } else {
                return 0.0;
            }

            length = 12.0 * pow (1.2, pow_factor) / POINTS_PER_INCH;
            *in = TRUE;
        }
    }

    return length;
}

RsvgLength
_rsvg_css_parse_length (const char *str)
{
    RsvgLength out;
    gboolean percent, em, ex, in;
    RelativeSize relative_size = RELATIVE_SIZE_NORMAL;
    percent = em = ex = in = FALSE;

    out.length = rsvg_css_parse_raw_length (str, &in, &percent, &em, &ex, &relative_size);
    if (percent)
        out.factor = 'p';
    else if (em)
        out.factor = 'm';
    else if (ex)
        out.factor = 'x';
    else if (in)
        out.factor = 'i';
    else if (relative_size == RELATIVE_SIZE_LARGER)
        out.factor = 'l';
    else if (relative_size == RELATIVE_SIZE_SMALLER)
        out.factor = 's';
    else
        out.factor = '\0';
    return out;
}

double
_rsvg_css_normalize_font_size (RsvgState * state, RsvgDrawingCtx * ctx)
{
    RsvgState *parent;

    switch (state->font_size.factor) {
    case 'p':
    case 'm':
    case 'x':
        parent= rsvg_state_parent (state);
        if (parent) {
            double parent_size;
            parent_size = _rsvg_css_normalize_font_size (parent, ctx);
            return state->font_size.length * parent_size;
        }
        break;
    default:
        return _rsvg_css_normalize_length (&state->font_size, ctx, 'v');
        break;
    }

    return 12.;
}

double
_rsvg_css_normalize_length (const RsvgLength * in, RsvgDrawingCtx * ctx, char dir)
{
    if (in->factor == '\0')
        return in->length;
    else if (in->factor == 'p') {
        if (dir == 'h')
            return in->length * ctx->vb.rect.width;
        if (dir == 'v')
            return in->length * ctx->vb.rect.height;
        if (dir == 'o')
            return in->length * rsvg_viewport_percentage (ctx->vb.rect.width,
                                                          ctx->vb.rect.height);
    } else if (in->factor == 'm' || in->factor == 'x') {
        double font = _rsvg_css_normalize_font_size (rsvg_current_state (ctx), ctx);
        if (in->factor == 'm')
            return in->length * font;
        else
            return in->length * font / 2.;
    } else if (in->factor == 'i') {
        if (dir == 'h')
            return in->length * ctx->dpi_x;
        if (dir == 'v')
            return in->length * ctx->dpi_y;
        if (dir == 'o')
            return in->length * rsvg_viewport_percentage (ctx->dpi_x, ctx->dpi_y);
    } else if (in->factor == 'l') {
        /* todo: "larger" */
    } else if (in->factor == 's') {
        /* todo: "smaller" */
    }

    return 0;
}

double
_rsvg_css_hand_normalize_length (const RsvgLength * in, gdouble pixels_per_inch,
                                 gdouble width_or_height, gdouble font_size)
{
    if (in->factor == '\0')
        return in->length;
    else if (in->factor == 'p')
        return in->length * width_or_height;
    else if (in->factor == 'm')
        return in->length * font_size;
    else if (in->factor == 'x')
        return in->length * font_size / 2.;
    else if (in->factor == 'i')
        return in->length * pixels_per_inch;

    return 0;
}

static gint
rsvg_css_clip_rgb_percent (gdouble in_percent)
{
    /* spec says to clip these values */
    if (in_percent > 100.)
        return 255;
    else if (in_percent <= 0.)
        return 0;
    return (gint) floor (255. * in_percent / 100. + 0.5);
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

/* pack 3 [0,255] ints into one 32 bit one */
#define PACK_RGB(r,g,b) (((r) << 16) | ((g) << 8) | (b))

/**
 * Parse a CSS2 color specifier, return RGB value
 */
guint32
rsvg_css_parse_color (const char *str, gboolean * inherit)
{
    gint val = 0;

    SETINHERIT ();

    if (str[0] == '#') {
        int i;
        for (i = 1; str[i]; i++) {
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
        if (i == 4) {
            val = ((val & 0xf00) << 8) | ((val & 0x0f0) << 4) | (val & 0x00f);
            val |= val << 4;
        }
    }
    /* i want to use g_str_has_prefix but it isn't in my gstrfuncs.h?? */
    else if (strstr (str, "rgb") != NULL) {
        gint r, g, b;
        r = g = b = 0;

        if (strstr (str, "%") != 0) {
            guint i, nb_toks;
            char **toks;

            /* assume rgb (9%, 100%, 23%) */
            for (i = 0; str[i] != '('; i++);

            i++;

            toks = rsvg_css_parse_list (str + i, &nb_toks);

            if (toks) {
                if (nb_toks == 3) {
                    r = rsvg_css_clip_rgb_percent (g_ascii_strtod (toks[0], NULL));
                    g = rsvg_css_clip_rgb_percent (g_ascii_strtod (toks[1], NULL));
                    b = rsvg_css_clip_rgb_percent (g_ascii_strtod (toks[2], NULL));
                }

                g_strfreev (toks);
            }
        } else {
            /* assume "rgb (r, g, b)" */
            if (3 == sscanf (str, " rgb ( %d , %d , %d ) ", &r, &g, &b)) {
                r = rsvg_css_clip_rgb (r);
                g = rsvg_css_clip_rgb (g);
                b = rsvg_css_clip_rgb (b);
            } else
                r = g = b = 0;
        }

        val = PACK_RGB (r, g, b);
    } else if (!strcmp (str, "inherit"))
        UNSETINHERIT ();
    else {
        CRRgb rgb;

        if (cr_rgb_set_from_name (&rgb, (const guchar *) str) == CR_OK) {
            val = PACK_RGB (rgb.red, rgb.green, rgb.blue);
        } else {
            /* default to black on failed lookup */
            UNSETINHERIT ();
            val = 0;
        }
    }

    return val;
}

#undef PACK_RGB

guint
rsvg_css_parse_opacity (const char *str)
{
    char *end_ptr = NULL;
    double opacity;

    opacity = g_ascii_strtod (str, &end_ptr);

    if (((opacity == -HUGE_VAL || opacity == HUGE_VAL) && (ERANGE == errno)) ||
        *end_ptr != '\0')
        opacity = 1.;

    opacity = CLAMP (opacity, 0., 1.);

    return (guint) floor (opacity * 255. + 0.5);
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
rsvg_css_parse_angle (const char *str)
{
    double degrees;
    char *end_ptr;

    degrees = g_ascii_strtod (str, &end_ptr);

    /* todo: error condition - figure out how to best represent it */
    if ((degrees == -HUGE_VAL || degrees == HUGE_VAL) && (ERANGE == errno))
        return 0.0;

    if (end_ptr) {
        if (!strcmp (end_ptr, "rad"))
            return degrees * 180. / G_PI;
        else if (!strcmp (end_ptr, "grad"))
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
rsvg_css_parse_frequency (const char *str)
{
    double f_hz;
    char *end_ptr;

    f_hz = g_ascii_strtod (str, &end_ptr);

    /* todo: error condition - figure out how to best represent it */
    if ((f_hz == -HUGE_VAL || f_hz == HUGE_VAL) && (ERANGE == errno))
        return 0.0;

    if (end_ptr && !strcmp (end_ptr, "kHz"))
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
rsvg_css_parse_time (const char *str)
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
rsvg_css_parse_font_style (const char *str, gboolean * inherit)
{
    SETINHERIT ();

    if (str) {
        if (!strcmp (str, "oblique"))
            return PANGO_STYLE_OBLIQUE;
        if (!strcmp (str, "italic"))
            return PANGO_STYLE_ITALIC;
        else if (!strcmp (str, "inherit")) {
            UNSETINHERIT ();
            return PANGO_STYLE_NORMAL;
        }
    }
    UNSETINHERIT ();
    return PANGO_STYLE_NORMAL;
}

PangoVariant
rsvg_css_parse_font_variant (const char *str, gboolean * inherit)
{
    SETINHERIT ();

    if (str) {
        if (!strcmp (str, "small-caps"))
            return PANGO_VARIANT_SMALL_CAPS;
        else if (!strcmp (str, "inherit")) {
            UNSETINHERIT ();
            return PANGO_VARIANT_NORMAL;
        }
    }
    UNSETINHERIT ();
    return PANGO_VARIANT_NORMAL;
}

PangoWeight
rsvg_css_parse_font_weight (const char *str, gboolean * inherit)
{
    SETINHERIT ();
    if (str) {
        if (!strcmp (str, "lighter"))
            return PANGO_WEIGHT_LIGHT;
        else if (!strcmp (str, "bold"))
            return PANGO_WEIGHT_BOLD;
        else if (!strcmp (str, "bolder"))
            return PANGO_WEIGHT_ULTRABOLD;
        else if (!strcmp (str, "100"))
            return (PangoWeight) 100;
        else if (!strcmp (str, "200"))
            return (PangoWeight) 200;
        else if (!strcmp (str, "300"))
            return (PangoWeight) 300;
        else if (!strcmp (str, "400"))
            return (PangoWeight) 400;
        else if (!strcmp (str, "500"))
            return (PangoWeight) 500;
        else if (!strcmp (str, "600"))
            return (PangoWeight) 600;
        else if (!strcmp (str, "700"))
            return (PangoWeight) 700;
        else if (!strcmp (str, "800"))
            return (PangoWeight) 800;
        else if (!strcmp (str, "900"))
            return (PangoWeight) 900;
        else if (!strcmp (str, "inherit")) {
            UNSETINHERIT ();
            return PANGO_WEIGHT_NORMAL;
        }
    }

    UNSETINHERIT ();
    return PANGO_WEIGHT_NORMAL;
}

PangoStretch
rsvg_css_parse_font_stretch (const char *str, gboolean * inherit)
{
    SETINHERIT ();

    if (str) {
        if (!strcmp (str, "ultra-condensed"))
            return PANGO_STRETCH_ULTRA_CONDENSED;
        else if (!strcmp (str, "extra-condensed"))
            return PANGO_STRETCH_EXTRA_CONDENSED;
        else if (!strcmp (str, "condensed") || !strcmp (str, "narrower"))       /* narrower not quite correct */
            return PANGO_STRETCH_CONDENSED;
        else if (!strcmp (str, "semi-condensed"))
            return PANGO_STRETCH_SEMI_CONDENSED;
        else if (!strcmp (str, "semi-expanded"))
            return PANGO_STRETCH_SEMI_EXPANDED;
        else if (!strcmp (str, "expanded") || !strcmp (str, "wider"))   /* wider not quite correct */
            return PANGO_STRETCH_EXPANDED;
        else if (!strcmp (str, "extra-expanded"))
            return PANGO_STRETCH_EXTRA_EXPANDED;
        else if (!strcmp (str, "ultra-expanded"))
            return PANGO_STRETCH_ULTRA_EXPANDED;
        else if (!strcmp (str, "inherit")) {
            UNSETINHERIT ();
            return PANGO_STRETCH_NORMAL;
        }
    }
    UNSETINHERIT ();
    return PANGO_STRETCH_NORMAL;
}

const char *
rsvg_css_parse_font_family (const char *str, gboolean * inherit)
{
    SETINHERIT ();

    if (!str)
        return NULL;
    else if (!strcmp (str, "inherit")) {
        UNSETINHERIT ();
        return NULL;
    } else
        return str;
}

#if !defined(HAVE_STRTOK_R)

static char *
strtok_r (char *s, const char *delim, char **last)
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

#endif                          /* !HAVE_STRTOK_R */

gchar **
rsvg_css_parse_list (const char *in_str, guint * out_list_len)
{
    char *ptr, *tok;
    char *str;

    guint n = 0;
    GSList *string_list = NULL;
    gchar **string_array = NULL;

    str = g_strdup (in_str);
    tok = strtok_r (str, ", \t", &ptr);
    if (tok != NULL) {
        if (strcmp (tok, " ") != 0) {
            string_list = g_slist_prepend (string_list, g_strdup (tok));
            n++;
        }

        while ((tok = strtok_r (NULL, ", \t", &ptr)) != NULL) {
            if (strcmp (tok, " ") != 0) {
                string_list = g_slist_prepend (string_list, g_strdup (tok));
                n++;
            }
        }
    }
    g_free (str);

    if (out_list_len)
        *out_list_len = n;

    if (string_list) {
        GSList *slist;

        string_array = g_new (gchar *, n + 1);

        string_array[n--] = NULL;
        for (slist = string_list; slist; slist = slist->next)
            string_array[n--] = (gchar *) slist->data;

        g_slist_free (string_list);
    }

    return string_array;
}

gdouble *
rsvg_css_parse_number_list (const char *in_str, guint * out_list_len)
{
    gchar **string_array;
    gdouble *output;
    guint len, i;

    if (out_list_len)
        *out_list_len = 0;

    string_array = rsvg_css_parse_list (in_str, &len);

    if (!(string_array && len))
        return NULL;

    output = g_new (gdouble, len);

    /* TODO: some error checking */
    for (i = 0; i < len; i++)
        output[i] = g_ascii_strtod (string_array[i], NULL);

    g_strfreev (string_array);

    if (out_list_len != NULL)
        *out_list_len = len;

    return output;
}

void
rsvg_css_parse_number_optional_number (const char *str, double *x, double *y)
{
    char *endptr;

    /* TODO: some error checking */

    *x = g_ascii_strtod (str, &endptr);

    if (endptr && *endptr != '\0')
        while (g_ascii_isspace (*endptr) && *endptr)
            endptr++;

    if (endptr && *endptr)
        *y = g_ascii_strtod (endptr, NULL);
    else
        *y = *x;
}

int
rsvg_css_parse_aspect_ratio (const char *str)
{
    char **elems;
    guint nb_elems;

    int ratio = RSVG_ASPECT_RATIO_NONE;

    elems = rsvg_css_parse_list (str, &nb_elems);

    if (elems && nb_elems) {
        guint i;

        for (i = 0; i < nb_elems; i++) {
            if (!strcmp (elems[i], "xMinYMin"))
                ratio = RSVG_ASPECT_RATIO_XMIN_YMIN;
            else if (!strcmp (elems[i], "xMidYMin"))
                ratio = RSVG_ASPECT_RATIO_XMID_YMIN;
            else if (!strcmp (elems[i], "xMaxYMin"))
                ratio = RSVG_ASPECT_RATIO_XMAX_YMIN;
            else if (!strcmp (elems[i], "xMinYMid"))
                ratio = RSVG_ASPECT_RATIO_XMIN_YMID;
            else if (!strcmp (elems[i], "xMidYMid"))
                ratio = RSVG_ASPECT_RATIO_XMID_YMID;
            else if (!strcmp (elems[i], "xMaxYMid"))
                ratio = RSVG_ASPECT_RATIO_XMAX_YMID;
            else if (!strcmp (elems[i], "xMinYMax"))
                ratio = RSVG_ASPECT_RATIO_XMIN_YMAX;
            else if (!strcmp (elems[i], "xMidYMax"))
                ratio = RSVG_ASPECT_RATIO_XMID_YMAX;
            else if (!strcmp (elems[i], "xMaxYMax"))
                ratio = RSVG_ASPECT_RATIO_XMAX_YMAX;
            else if (!strcmp (elems[i], "slice"))
                ratio |= RSVG_ASPECT_RATIO_SLICE;
        }

        g_strfreev (elems);
    }

    return ratio;
}

gboolean
rsvg_css_parse_overflow (const char *str, gboolean * inherit)
{
    SETINHERIT ();
    if (!strcmp (str, "visible") || !strcmp (str, "auto"))
        return 1;
    if (!strcmp (str, "hidden") || !strcmp (str, "scroll"))
        return 0;
    UNSETINHERIT ();
    return 0;
}

static void
rsvg_xml_noerror (void *data, xmlErrorPtr error)
{
}

/* This is quite hacky and not entirely correct, but apparently 
 * libxml2 has NO support for parsing pseudo attributes as defined 
 * by the xml-styleheet spec.
 */
char **
rsvg_css_parse_xml_attribute_string (const char *attribute_string)
{
    xmlSAXHandler handler;
    xmlParserCtxtPtr parser;
    xmlDocPtr doc;
    xmlNodePtr node;
    xmlAttrPtr attr;
    char *tag;
    GPtrArray *attributes;
    char **retval = NULL;

    tag = g_strdup_printf ("<rsvg-hack %s />\n", attribute_string);

    memset (&handler, 0, sizeof (handler));
    xmlSAX2InitDefaultSAXHandler (&handler, 0);
    handler.serror = rsvg_xml_noerror;
    parser = xmlCreatePushParserCtxt (&handler, NULL, tag, strlen (tag) + 1, NULL);
    if (xmlParseDocument (parser) != 0)
        goto done;

    if ((doc = parser->myDoc) == NULL ||
        (node = doc->children) == NULL ||
        strcmp (node->name, "rsvg-hack") != 0 ||
        node->next != NULL ||
        node->properties == NULL)
          goto done;

    attributes = g_ptr_array_new ();
    for (attr = node->properties; attr; attr = attr->next) {
        xmlNodePtr content = attr->children;

        g_ptr_array_add (attributes, g_strdup ((char *) attr->name));
        if (content)
          g_ptr_array_add (attributes, g_strdup ((char *) content->content));
        else
          g_ptr_array_add (attributes, g_strdup (""));
    }

    g_ptr_array_add (attributes, NULL);
    retval = (char **) g_ptr_array_free (attributes, FALSE);

  done:
    if (parser->myDoc)
      xmlFreeDoc (parser->myDoc);
    xmlFreeParserCtxt (parser);
    g_free (tag);

    return retval;
}
