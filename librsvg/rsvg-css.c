/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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

/* This is defined like this so that we can export the Rust function... just for
 * the benefit of rsvg-convert.c
 */
RsvgCssColorSpec rsvg_css_parse_color_ (const char       *str,
                                        AllowInherit      allow_inherit,
                                        AllowCurrentColor allow_current_color)
{
    return rsvg_css_parse_color (str, allow_inherit, allow_current_color);
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
