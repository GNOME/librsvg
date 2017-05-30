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
#include "rsvg-image.h"

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

static void
rsvg_stop_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgGradientStop *stop = impl;
    const char *value;
    RsvgState *state;
    RsvgState *inherited_state;
    int opacity;
    guint32 color;

    state = rsvg_node_get_state (node);

    if ((value = rsvg_property_bag_lookup (atts, "offset"))) {
        /* either a number [0,1] or a percentage */
        RsvgLength length = rsvg_length_parse (value, LENGTH_DIR_BOTH);

        if (length.unit == LENGTH_UNIT_DEFAULT || length.unit == LENGTH_UNIT_PERCENT) {
            double offset;

            offset = length.length;

            if (offset < 0.0)
                offset = 0.0;
            else if (offset > 1.0)
                offset = 1.0;

            stop->offset = offset;
            stop->is_valid = TRUE;
        } else {
            /* Only default and percent values are allowed */
            stop->is_valid = FALSE;
        }
    }
    if ((value = rsvg_property_bag_lookup (atts, "style")))
        rsvg_parse_style (handle, state, value);

    rsvg_parse_style_pairs (state, atts);

    inherited_state = rsvg_state_new ();
    rsvg_state_reconstruct (inherited_state, node);

    if (state->has_stop_color) {
        switch (state->stop_color.kind) {
        case RSVG_CSS_COLOR_SPEC_INHERIT:
            switch (inherited_state->stop_color.kind) {
            case RSVG_CSS_COLOR_SPEC_INHERIT:
                color = 0;
                break;

            case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
                color = inherited_state->current_color;
                break;

            case RSVG_CSS_COLOR_SPEC_ARGB:
                color = inherited_state->stop_color.argb;
                break;

            case RSVG_CSS_COLOR_PARSE_ERROR:
                color = 0;
                break;

            default:
                g_assert_not_reached ();
                return;
            }
            break;

        case RSVG_CSS_COLOR_SPEC_CURRENT_COLOR:
            color = inherited_state->current_color;
            break;

        case RSVG_CSS_COLOR_SPEC_ARGB:
            color = state->stop_color.argb & 0x00ffffff;
            break;

        case RSVG_CSS_COLOR_PARSE_ERROR:
            color = 0;
            break;

        default:
            g_assert_not_reached ();
            return;
        }
    } else {
        color = 0;
    }

    if (state->has_stop_opacity) {
        switch (state->stop_opacity.kind) {
        case STOP_OPACITY_SPECIFIED:
            opacity = state->stop_opacity.opacity;
            break;

        case STOP_OPACITY_INHERIT:
            switch (inherited_state->stop_opacity.kind) {
            case STOP_OPACITY_SPECIFIED:
                opacity = inherited_state->stop_opacity.opacity;
                break;

            case STOP_OPACITY_INHERIT:
                opacity = 0xff;
                break;

            default:
                g_assert_not_reached ();
                return;
            }
            break;

        default:
            g_assert_not_reached ();
            return;
        }
    } else {
        opacity = 0xff;
    }

    stop->rgba = (color << 8) | opacity;

    rsvg_state_free (inherited_state);
}

static void
rsvg_paint_server_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing; paint servers are handled specially */
}

RsvgNode *
rsvg_new_stop (const char *element_name, RsvgNode *parent)
{
    RsvgGradientStop *stop = g_new0 (RsvgGradientStop, 1);

    stop->offset = 0;
    stop->rgba = 0xff000000;
    stop->is_valid = FALSE;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_STOP,
                                parent,
                                rsvg_state_new (),
                                stop,
                                rsvg_stop_set_atts,
                                rsvg_paint_server_draw,
                                g_free);
}

static void
rsvg_linear_gradient_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgLinearGradient *grad = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "x1"))) {
        grad->x1 = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        grad->hasx1 = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "y1"))) {
        grad->y1 = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        grad->hasy1 = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "x2"))) {
        grad->x2 = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        grad->hasx2 = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "y2"))) {
        grad->y2 = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        grad->hasy2 = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "spreadMethod"))) {
        if (!strcmp (value, "pad")) {
            grad->spread = CAIRO_EXTEND_PAD;
        } else if (!strcmp (value, "reflect")) {
            grad->spread = CAIRO_EXTEND_REFLECT;
        } else if (!strcmp (value, "repeat")) {
            grad->spread = CAIRO_EXTEND_REPEAT;
        }
        grad->hasspread = TRUE;
    }
    g_free (grad->fallback);
    grad->fallback = g_strdup (rsvg_property_bag_lookup (atts, "xlink:href"));
    if ((value = rsvg_property_bag_lookup (atts, "gradientTransform"))) {
        if (rsvg_parse_transform (&grad->affine, value)) {
            grad->hastransform = TRUE;
        } else {
            rsvg_node_set_attribute_parse_error (node,
                                                 "gradientTransform",
                                                 "Invalid transformation");
        }
    }
    if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            grad->obj_bbox = FALSE;
        else if (!strcmp (value, "objectBoundingBox"))
            grad->obj_bbox = TRUE;
        grad->hasbbox = TRUE;
    }
}

static void
rsvg_linear_gradient_free (gpointer impl)
{
    RsvgLinearGradient *self = impl;

    g_free (self->fallback);
    g_free (self);
}

RsvgNode *
rsvg_new_linear_gradient (const char *element_name, RsvgNode *parent)
{
    RsvgLinearGradient *grad = NULL;

    grad = g_new0 (RsvgLinearGradient, 1);
    cairo_matrix_init_identity (&grad->affine);
    grad->x1 = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    grad->y1 = grad->y2 = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    grad->x2 = rsvg_length_parse ("1", LENGTH_DIR_HORIZONTAL);
    grad->fallback = NULL;
    grad->obj_bbox = TRUE;
    grad->spread = CAIRO_EXTEND_PAD;
    grad->hasx1 = grad->hasy1 = grad->hasx2 = grad->hasy2 = grad->hasbbox = grad->hasspread =
        grad->hastransform = FALSE;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_LINEAR_GRADIENT,
                                parent,
                                rsvg_state_new (),
                                grad,
                                rsvg_linear_gradient_set_atts,
                                rsvg_paint_server_draw,
                                rsvg_linear_gradient_free);
}

static void
rsvg_radial_gradient_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgRadialGradient *grad = impl;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "cx"))) {
        grad->cx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        grad->hascx = TRUE;
        if (!grad->hasfx)
            grad->fx = grad->cx;
    }
    if ((value = rsvg_property_bag_lookup (atts, "cy"))) {
        grad->cy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        grad->hascy = TRUE;
        if (!grad->hasfy)
            grad->fy = grad->cy;
    }
    if ((value = rsvg_property_bag_lookup (atts, "r"))) {
        grad->r = rsvg_length_parse (value, LENGTH_DIR_BOTH);
        grad->hasr = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "fx"))) {
        grad->fx = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        grad->hasfx = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "fy"))) {
        grad->fy = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        grad->hasfy = TRUE;
    }
    g_free (grad->fallback);
    grad->fallback = g_strdup (rsvg_property_bag_lookup (atts, "xlink:href"));
    if ((value = rsvg_property_bag_lookup (atts, "gradientTransform"))) {
        if (rsvg_parse_transform (&grad->affine, value)) {
            grad->hastransform = TRUE;
        } else {
            rsvg_node_set_attribute_parse_error (node,
                                                 "gradientTransform",
                                                 "Invalid transformation");
        }
    }
    if ((value = rsvg_property_bag_lookup (atts, "spreadMethod"))) {
        if (!strcmp (value, "pad"))
            grad->spread = CAIRO_EXTEND_PAD;
        else if (!strcmp (value, "reflect"))
            grad->spread = CAIRO_EXTEND_REFLECT;
        else if (!strcmp (value, "repeat"))
            grad->spread = CAIRO_EXTEND_REPEAT;
        grad->hasspread = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "gradientUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            grad->obj_bbox = FALSE;
        else if (!strcmp (value, "objectBoundingBox"))
            grad->obj_bbox = TRUE;
        grad->hasbbox = TRUE;
    }
}

static void
rsvg_radial_gradient_free (gpointer impl)
{
    RsvgRadialGradient *self = impl;

    g_free (self->fallback);
    g_free (self);
}

RsvgNode *
rsvg_new_radial_gradient (const char *element_name, RsvgNode *parent)
{
    RsvgRadialGradient *grad = g_new0 (RsvgRadialGradient, 1);
    cairo_matrix_init_identity (&grad->affine);
    grad->obj_bbox = TRUE;
    grad->spread = CAIRO_EXTEND_PAD;
    grad->fallback = NULL;
    grad->cx = grad->cy = grad->r = grad->fx = grad->fy = rsvg_length_parse ("0.5", LENGTH_DIR_BOTH);
    grad->hascx = grad->hascy = grad->hasfx = grad->hasfy = grad->hasr = grad->hasbbox =
        grad->hasspread = grad->hastransform = FALSE;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_RADIAL_GRADIENT,
                                parent,
                                rsvg_state_new (),
                                grad,
                                rsvg_radial_gradient_set_atts,
                                rsvg_paint_server_draw,
                                rsvg_radial_gradient_free);
}
