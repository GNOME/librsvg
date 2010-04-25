/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-cond.c: Handle SVG conditionals

   Copyright (C) 2004-2005 Dom Lachowicz <cinamod@hotmail.com>

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

   Author: Dom Lachowicz
*/

#include "config.h"

#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"

#include <string.h>
#include <stdlib.h>
#include <locale.h>

static const char *implemented_features[] = {
    "http://www.w3.org/TR/SVG11/feature#BasicText",
    "http://www.w3.org/TR/SVG11/feature#BasicFilter",
    "http://www.w3.org/TR/SVG11/feature#BasicGraphicsAttribute",
    "http://www.w3.org/TR/SVG11/feature#BasicPaintAttribute",
    "http://www.w3.org/TR/SVG11/feature#BasicStructure",
    "http://www.w3.org/TR/SVG11/feature#ConditionalProcessing",
    "http://www.w3.org/TR/SVG11/feature#ContainerAttribute",
    "http://www.w3.org/TR/SVG11/feature#Filter",
    "http://www.w3.org/TR/SVG11/feature#Gradient",
    "http://www.w3.org/TR/SVG11/feature#Image",
    "http://www.w3.org/TR/SVG11/feature#Marker",
    "http://www.w3.org/TR/SVG11/feature#Mask",
    "http://www.w3.org/TR/SVG11/feature#OpacityAttribute",
    "http://www.w3.org/TR/SVG11/feature#Pattern",
    "http://www.w3.org/TR/SVG11/feature#Shape",
    "http://www.w3.org/TR/SVG11/feature#Structure",
    "http://www.w3.org/TR/SVG11/feature#Style",
    "http://www.w3.org/TR/SVG11/feature#SVG",
    "http://www.w3.org/TR/SVG11/feature#SVG-static",
    "http://www.w3.org/TR/SVG11/feature#View",
    "org.w3c.svg.static"        /* deprecated SVG 1.0 feature string */
};
static const guint nb_implemented_features = G_N_ELEMENTS (implemented_features);

static const char **implemented_extensions = NULL;
static const guint nb_implemented_extensions = 0;

static int
rsvg_feature_compare (const void *a, const void *b)
{
    return strcmp ((const char *) a, *(const char **) b);
}

/* http://www.w3.org/TR/SVG/struct.html#RequiredFeaturesAttribute */
static gboolean
rsvg_cond_fulfills_requirement (const char *value, const char **features, guint nb_features)
{
    guint nb_elems = 0;
    char **elems;
    gboolean permitted = TRUE;

    elems = rsvg_css_parse_list (value, &nb_elems);

    if (elems && nb_elems) {
        guint i;

        for (i = 0; (i < nb_elems) && permitted; i++)
            if (!bsearch (elems[i], features, nb_features, sizeof (char *), rsvg_feature_compare))
                permitted = FALSE;

        g_strfreev (elems);
    } else
        permitted = FALSE;

    return permitted;
}

/* http://www.w3.org/TR/SVG/struct.html#SystemLanguageAttribute */
static gboolean
rsvg_locale_compare (const char *a, const char *b)
{
    const char *hyphen;

    /* check for an exact-ish match first */
    if (!g_ascii_strncasecmp (a, b, strlen (b)))
        return TRUE;

    /* check to see if there's a hyphen */
    hyphen = strstr (b, "-");
    if (!hyphen)
        return FALSE;

    /* compare up to the hyphen */
    return !g_ascii_strncasecmp (a, b, (hyphen - b));
}

/* http://www.w3.org/TR/SVG/struct.html#SystemLanguageAttribute */
static gboolean
rsvg_cond_parse_system_language (const char *value)
{
    guint nb_elems = 0;
    char **elems;
    gboolean permitted = TRUE;

    elems = rsvg_css_parse_list (value, &nb_elems);

    if (elems && nb_elems) {
        guint i;
        gchar *locale = NULL;

        /* we're required to be pessimistic until we hit a language we recognize */
        permitted = FALSE;

#if defined(G_OS_WIN32)
        if (!locale)
            locale = g_win32_getlocale ();
#endif

        if (!locale)
            locale = g_strdup (g_getenv ("LANG"));

#if defined(HAVE_LC_MESSAGES)
        if (!locale)
            locale = g_strdup (setlocale (LC_MESSAGES, NULL));
#endif

        if (!locale)
            locale = g_strdup (setlocale (LC_ALL, NULL));

        if (!locale || strcmp (locale, "C") == 0) {
            g_free (locale);
            locale = g_strdup ("en");
        }

        if (locale) {
            for (i = 0; (i < nb_elems) && !permitted; i++) {
                if (rsvg_locale_compare (locale, elems[i]))
                    permitted = TRUE;
            }

            g_free (locale);
        }

        g_strfreev (elems);
    } else
        permitted = FALSE;

    return permitted;
}

/* returns TRUE if this element should be processed according to <switch> semantics
   http://www.w3.org/TR/SVG/struct.html#SwitchElement */
gboolean
rsvg_eval_switch_attributes (RsvgPropertyBag * atts, gboolean * p_has_cond)
{
    gboolean permitted = TRUE;
    gboolean has_cond = FALSE;

    if (atts && rsvg_property_bag_size (atts)) {
        const char *value;

        if ((value = rsvg_property_bag_lookup (atts, "requiredFeatures"))) {
            permitted =
                rsvg_cond_fulfills_requirement (value, implemented_features,
                                                nb_implemented_features);
            has_cond = TRUE;
        }

        if (permitted && (value = rsvg_property_bag_lookup (atts, "requiredExtensions"))) {
            permitted =
                rsvg_cond_fulfills_requirement (value, implemented_extensions,
                                                nb_implemented_extensions);
            has_cond = TRUE;
        }

        if (permitted && (value = rsvg_property_bag_lookup (atts, "systemLanguage"))) {
            permitted = rsvg_cond_parse_system_language (value);
            has_cond = TRUE;
        }
    }

    if (p_has_cond)
        *p_has_cond = has_cond;

    return permitted;
}
