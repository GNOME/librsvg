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
#include "rsvg-private.h"
#include "rsvg-css.h"
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
 * rsvg_css_parse_vbox:
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

/* Recursive evaluation of all parent elements regarding absolute font size */
static double
normalize_font_size (RsvgState * state, RsvgDrawingCtx * ctx)
{
    RsvgState *parent;

    switch (state->font_size.unit) {
    case LENGTH_UNIT_PERCENT:
    case LENGTH_UNIT_FONT_EM:
    case LENGTH_UNIT_FONT_EX:
        parent = rsvg_state_parent (state);
        if (parent) {
            double parent_size;
            parent_size = normalize_font_size (parent, ctx);
            return state->font_size.length * parent_size;
        }
        break;
    default:
        return rsvg_length_normalize (&state->font_size, ctx);
        break;
    }

    return 12.;
}

double
rsvg_drawing_ctx_get_normalized_font_size (RsvgDrawingCtx *ctx)
{
    return normalize_font_size (rsvg_current_state (ctx), ctx);
}

/* Recursive evaluation of all parent elements regarding basline-shift */
double
_rsvg_css_accumulate_baseline_shift (RsvgState * state, RsvgDrawingCtx * ctx)
{
    RsvgState *parent;
    double shift = 0.;

    parent = rsvg_state_parent (state);
    if (parent) {
        if (state->has_baseline_shift) {
            double parent_font_size;
            parent_font_size = normalize_font_size (parent, ctx); /* font size from here */
            shift = parent_font_size * state->baseline_shift;
        }
        shift += _rsvg_css_accumulate_baseline_shift (parent, ctx); /* baseline-shift for parent element */
    }

    return shift;
}

static gint
rsvg_css_clip_rgb_percent (const char *s, double max)
{
    double value;
    char *end;

    value = g_ascii_strtod (s, &end);

    if (*end == '%') {
        value = CLAMP (value, 0, 100) / 100.0;
    }
    else {
        value = CLAMP (value, 0, max) / max;
    }
    
    return (gint) floor (value * 255 + 0.5);
}

/* pack 3 [0,255] ints into one 32 bit one */
#define PACK_RGBA(r,g,b,a) (((a) << 24) | ((r) << 16) | ((g) << 8) | (b))
#define PACK_RGB(r,g,b) PACK_RGBA(r, g, b, 255)

/**
 * rsvg_css_parse_color:
 * @str: string to parse
 * @inherit: whether to inherit
 *
 * Parse a CSS2 color specifier, return RGB value
 *
 * Returns: and RGB value
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

        val |= 0xff000000; /* opaque */
    }
    else if (g_str_has_prefix (str, "rgb")) {
        gint r, g, b, a;
        gboolean has_alpha;
        guint nb_toks;
        char **toks;

        r = g = b = 0;
        a = 255;

        if (str[3] == 'a') {
            /* "rgba" */
            has_alpha = TRUE;
            str += 4;
        }
        else {
            /* "rgb" */
            has_alpha = FALSE;
            str += 3;
        }

        str = strchr (str, '(');
        if (str == NULL)
          return val;

        toks = rsvg_css_parse_list (str + 1, &nb_toks);

        if (toks) {
            if (nb_toks == (has_alpha ? 4 : 3)) {
                r = rsvg_css_clip_rgb_percent (toks[0], 255.0);
                g = rsvg_css_clip_rgb_percent (toks[1], 255.0);
                b = rsvg_css_clip_rgb_percent (toks[2], 255.0);
                if (has_alpha)
                    a = rsvg_css_clip_rgb_percent (toks[3], 1.0);
                else
                    a = 255;
            }

            g_strfreev (toks);
        }

        val = PACK_RGBA (r, g, b, a);
    } else if (!strcmp (str, "inherit"))
        UNSETINHERIT ();
    else {
        CRRgb rgb;

        if (cr_rgb_set_from_name (&rgb, (const guchar *) str) == CR_OK) {
            val = PACK_RGB (rgb.red, rgb.green, rgb.blue);
        } else {
            /* default to opaque black on failed lookup */
            UNSETINHERIT ();
            val = PACK_RGB (0, 0, 0);
        }
    }

    return val;
}

#undef PACK_RGB
#undef PACK_RGBA

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

PangoStyle
rsvg_css_parse_font_style (const char *str, gboolean * inherit)
{
    SETINHERIT ();

    if (str) {
        if (!strcmp (str, "oblique"))
            return PANGO_STYLE_OBLIQUE;
        if (!strcmp (str, "italic"))
            return PANGO_STYLE_ITALIC;
        if (!strcmp (str, "normal"))
            return PANGO_STYLE_NORMAL;
        if (!strcmp (str, "inherit")) {
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
    parser->options |= XML_PARSE_NONET;

    if (xmlParseDocument (parser) != 0)
        goto done;

    if ((doc = parser->myDoc) == NULL ||
        (node = doc->children) == NULL ||
        strcmp ((const char *) node->name, "rsvg-hack") != 0 ||
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
