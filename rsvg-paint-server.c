/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-paint-server.c: Implement the SVG paint server abstraction.

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
#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "rsvg-paint-server.h"
#include "rsvg-styles.h"

#include <glib.h>
#include <string.h>
#include <math.h>

#include "rsvg-css.h"

static RsvgPaintServer *
rsvg_paint_server_solid (guint32 argb)
{
    RsvgPaintServer *result = g_new0 (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_SOLID;
    result->core.color = g_new0 (RsvgSolidColor, 1);
    result->core.color->argb = argb;
    result->core.color->currentcolor = FALSE;

    return result;
}

static RsvgPaintServer *
rsvg_paint_server_solid_current_color (void)
{
    RsvgPaintServer *result = g_new0 (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_SOLID;
    result->core.color = g_new0 (RsvgSolidColor, 1);
    result->core.color->currentcolor = TRUE;

    return result;
}

static RsvgPaintServer *
rsvg_paint_server_iri (char *iri, gboolean has_alternate, RsvgSolidColor alternate)
{
    RsvgPaintServer *result = g_new0 (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_IRI;
    result->core.iri = g_new0 (RsvgPaintServerIri, 1);
    result->core.iri->iri_str = iri;
    result->core.iri->has_alternate = has_alternate;
    result->core.iri->alternate = alternate;

    return result;
}

static gboolean
parse_current_color_or_argb (const char *str, RsvgSolidColor *dest)
{
    if (!strcmp (str, "none")) {
        dest->currentcolor = FALSE;
        dest->argb = 0;
        return FALSE;
    } else {
        RsvgCssColorSpec spec;

        spec = rsvg_css_parse_color (str, ALLOW_INHERIT_NO, ALLOW_CURRENT_COLOR_YES);
        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            dest->currentcolor = TRUE;
            dest->argb = 0;
            return TRUE;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            dest->currentcolor = FALSE;
            dest->argb = spec.argb;
            return TRUE;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            dest->currentcolor = FALSE;
            dest->argb = 0;
            return FALSE;

        default:
            g_assert_not_reached ();
            return FALSE;
        }
    }
}

/**
 * rsvg_paint_server_parse:
 * @str: The SVG paint specification string to parse.
 *
 * Parses the paint specification @str, creating a new paint server
 * object.
 *
 * Return value: (nullable): The newly created paint server, or %NULL
 *   on error.
 **/
RsvgPaintServer *
rsvg_paint_server_parse (gboolean *inherit, const char *str)
{
    char *name;
    const char *rest;

    if (inherit != NULL)
        *inherit = TRUE;

    if (str == NULL || !strcmp (str, "none"))
        return NULL;

    name = rsvg_get_url_string (str, &rest);
    if (name) {
        RsvgSolidColor alternate;
        gboolean has_alternate;

        while (*rest && g_ascii_isspace (*rest)) {
            rest++;
        }

        has_alternate = parse_current_color_or_argb (rest, &alternate);

        return rsvg_paint_server_iri (name, has_alternate, alternate);
    } else {
        RsvgCssColorSpec spec;

        spec = rsvg_css_parse_color (str, ALLOW_INHERIT_YES, ALLOW_CURRENT_COLOR_YES);
        switch (spec.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            /* FIXME: this is incorrect; we should inherit the paint server */
            if (inherit != NULL)
                *inherit = FALSE;
            return rsvg_paint_server_solid (0);

        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            return rsvg_paint_server_solid_current_color ();

        case RSVG_CSS_COLOR_SPEC_ARGB:
            return rsvg_paint_server_solid (spec.argb);

        case RSVG_CSS_COLOR_PARSE_ERROR:
            return NULL;

        default:
            g_assert_not_reached ();
            return NULL;
        }
    }
}

/**
 * rsvg_paint_server_ref:
 * @ps: The paint server object to reference.
 *
 * Reference a paint server object.
 **/
void
rsvg_paint_server_ref (RsvgPaintServer * ps)
{
    if (ps == NULL)
        return;
    ps->refcnt++;
}

/**
 * rsvg_paint_server_unref:
 * @ps: The paint server object to unreference.
 *
 * Unreference a paint server object.
 **/
void
rsvg_paint_server_unref (RsvgPaintServer * ps)
{
    if (ps == NULL)
        return;
    if (--ps->refcnt == 0) {
        if (ps->type == RSVG_PAINT_SERVER_SOLID)
            g_free (ps->core.color);
        else if (ps->type == RSVG_PAINT_SERVER_IRI) {
            g_free (ps->core.iri->iri_str);
            g_free (ps->core.iri);
        }
        g_free (ps);
    }
}
