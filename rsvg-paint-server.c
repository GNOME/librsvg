/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
    RsvgPaintServer *result = g_new (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_SOLID;
    result->core.color = g_new (RsvgSolidColor, 1);
    result->core.color->argb = argb;
    result->core.color->currentcolor = FALSE;

    return result;
}

static RsvgPaintServer *
rsvg_paint_server_solid_current_color (void)
{
    RsvgPaintServer *result = g_new (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_SOLID;
    result->core.color = g_new (RsvgSolidColor, 1);
    result->core.color->currentcolor = TRUE;

    return result;
}

static RsvgPaintServer *
rsvg_paint_server_iri (char *iri)
{
    RsvgPaintServer *result = g_new (RsvgPaintServer, 1);

    result->refcnt = 1;
    result->type = RSVG_PAINT_SERVER_IRI;
    result->core.iri = iri;

    return result;
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
rsvg_paint_server_parse (gboolean * inherit, const char *str)
{
    char *name;
    guint32 argb;
    if (inherit != NULL)
        *inherit = 1;
    if (str == NULL || !strcmp (str, "none"))
        return NULL;

    name = rsvg_get_url_string (str);
    if (name) {
        return rsvg_paint_server_iri (name);
    } else if (!strcmp (str, "inherit")) {
        if (inherit != NULL)
            *inherit = 0;
        return rsvg_paint_server_solid (0);
    } else if (!strcmp (str, "currentColor")) {
        RsvgPaintServer *ps;
        ps = rsvg_paint_server_solid_current_color ();
        return ps;
    } else {
        argb = rsvg_css_parse_color (str, inherit);
        return rsvg_paint_server_solid (argb);
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
        else if (ps->type == RSVG_PAINT_SERVER_IRI)
            g_free (ps->core.iri);
        g_free (ps);
    }
}

static void
rsvg_stop_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    gboolean has_stop_color = FALSE;
    gboolean has_stop_opacity = FALSE;
    gboolean is_current_color = FALSE;
    const char *value;
    RsvgGradientStop *stop;
    RsvgState *state;
    int opacity;
    guint32 color;

    stop = (RsvgGradientStop *) self;

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
        rsvg_parse_style (ctx, rsvg_node_get_state (self), value);

    if ((value = rsvg_property_bag_lookup (atts, "stop-color"))) {
        has_stop_color = TRUE;

        if (!strcmp (value, "currentColor"))
            is_current_color = TRUE;
    }

    if ((value = rsvg_property_bag_lookup (atts, "stop-opacity"))) {
        has_stop_opacity = TRUE;
    }

    rsvg_parse_style_pairs (ctx, rsvg_node_get_state (self), atts);

    state = rsvg_state_new ();
    rsvg_state_reconstruct (state, self);

    if (has_stop_color) {
        if (is_current_color)
            color = state->current_color;
        else {
            color = state->stop_color & 0x00ffffff;
        }
    } else {
        color = 0x0;
    }

    if (has_stop_opacity) {
        opacity = state->stop_opacity;
    } else {
        opacity = 0xff;
    }

    stop->rgba = (color << 8) | opacity;

    rsvg_state_free (state);
}

RsvgNode *
rsvg_new_stop (const char *element_name)
{
    RsvgGradientStop *stop = g_new (RsvgGradientStop, 1);
    _rsvg_node_init (&stop->super, RSVG_NODE_TYPE_STOP);
    stop->super.set_atts = rsvg_stop_set_atts;
    stop->offset = 0;
    stop->rgba = 0xff000000;
    stop->is_valid = FALSE;
    return &stop->super;
}

static void
rsvg_linear_gradient_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgLinearGradient *grad = (RsvgLinearGradient *) self;
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
        rsvg_parse_transform (&grad->affine, value);
        grad->hastransform = TRUE;
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
rsvg_linear_gradient_free (RsvgNode * node)
{
    RsvgLinearGradient *self = (RsvgLinearGradient *) node;
    g_free (self->fallback);
    _rsvg_node_free (node);
}

RsvgNode *
rsvg_new_linear_gradient (const char *element_name)
{
    RsvgLinearGradient *grad = NULL;
    grad = g_new (RsvgLinearGradient, 1);
    _rsvg_node_init (&grad->super, RSVG_NODE_TYPE_LINEAR_GRADIENT);
    cairo_matrix_init_identity (&grad->affine);
    grad->x1 = rsvg_length_parse ("0", LENGTH_DIR_HORIZONTAL);
    grad->y1 = grad->y2 = rsvg_length_parse ("0", LENGTH_DIR_VERTICAL);
    grad->x2 = rsvg_length_parse ("1", LENGTH_DIR_HORIZONTAL);
    grad->fallback = NULL;
    grad->obj_bbox = TRUE;
    grad->spread = CAIRO_EXTEND_PAD;
    grad->super.free = rsvg_linear_gradient_free;
    grad->super.set_atts = rsvg_linear_gradient_set_atts;
    grad->hasx1 = grad->hasy1 = grad->hasx2 = grad->hasy2 = grad->hasbbox = grad->hasspread =
        grad->hastransform = FALSE;
    return &grad->super;
}

static void
rsvg_radial_gradient_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgRadialGradient *grad = (RsvgRadialGradient *) self;
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
        rsvg_parse_transform (&grad->affine, value);
        grad->hastransform = TRUE;
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
rsvg_radial_gradient_free (RsvgNode * node)
{
    RsvgRadialGradient *self = (RsvgRadialGradient *) node;
    g_free (self->fallback);
    _rsvg_node_free (node);
}

RsvgNode *
rsvg_new_radial_gradient (const char *element_name)
{

    RsvgRadialGradient *grad = g_new (RsvgRadialGradient, 1);
    _rsvg_node_init (&grad->super, RSVG_NODE_TYPE_RADIAL_GRADIENT);
    cairo_matrix_init_identity (&grad->affine);
    grad->obj_bbox = TRUE;
    grad->spread = CAIRO_EXTEND_PAD;
    grad->fallback = NULL;
    grad->cx = grad->cy = grad->r = grad->fx = grad->fy = rsvg_length_parse ("0.5", LENGTH_DIR_BOTH);
    grad->super.free = rsvg_radial_gradient_free;
    grad->super.set_atts = rsvg_radial_gradient_set_atts;
    grad->hascx = grad->hascy = grad->hasfx = grad->hasfy = grad->hasr = grad->hasbbox =
        grad->hasspread = grad->hastransform = FALSE;
    return &grad->super;
}

static void
rsvg_pattern_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgPattern *pattern = (RsvgPattern *) self;
    const char *value;

    if ((value = rsvg_property_bag_lookup (atts, "viewBox"))) {
        pattern->vbox = rsvg_css_parse_vbox (value);
        pattern->hasvbox = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "x"))) {
        pattern->x = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        pattern->hasx = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "y"))) {
        pattern->y = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        pattern->hasy = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "width"))) {
        pattern->width = rsvg_length_parse (value, LENGTH_DIR_HORIZONTAL);
        pattern->haswidth = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "height"))) {
        pattern->height = rsvg_length_parse (value, LENGTH_DIR_VERTICAL);
        pattern->hasheight = TRUE;
    }
    g_free (pattern->fallback);
    pattern->fallback = g_strdup (rsvg_property_bag_lookup (atts, "xlink:href"));
    if ((value = rsvg_property_bag_lookup (atts, "patternTransform"))) {
        rsvg_parse_transform (&pattern->affine, value);
        pattern->hastransform = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "patternUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            pattern->obj_bbox = FALSE;
        else if (!strcmp (value, "objectBoundingBox"))
            pattern->obj_bbox = TRUE;
        pattern->hasbbox = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "patternContentUnits"))) {
        if (!strcmp (value, "userSpaceOnUse"))
            pattern->obj_cbbox = FALSE;
        else if (!strcmp (value, "objectBoundingBox"))
            pattern->obj_cbbox = TRUE;
        pattern->hascbox = TRUE;
    }
    if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio"))) {
        pattern->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
        pattern->hasaspect = TRUE;
    }
}

static void
rsvg_pattern_free (RsvgNode * node)
{
    RsvgPattern *self = (RsvgPattern *) node;
    g_free (self->fallback);
    _rsvg_node_free (node);
}


RsvgNode *
rsvg_new_pattern (const char *element_name)
{
    RsvgPattern *pattern = g_new (RsvgPattern, 1);
    _rsvg_node_init (&pattern->super, RSVG_NODE_TYPE_PATTERN);
    cairo_matrix_init_identity (&pattern->affine);
    pattern->obj_bbox = TRUE;
    pattern->obj_cbbox = FALSE;
    pattern->x = pattern->y = pattern->width = pattern->height = rsvg_length_parse ("0", LENGTH_DIR_BOTH);
    pattern->fallback = NULL;
    pattern->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    pattern->vbox.active = FALSE;
    pattern->super.free = rsvg_pattern_free;
    pattern->super.set_atts = rsvg_pattern_set_atts;
    pattern->hasx = pattern->hasy = pattern->haswidth = pattern->hasheight = pattern->hasbbox =
        pattern->hascbox = pattern->hasvbox = pattern->hasaspect = pattern->hastransform = FALSE;
    return &pattern->super;
}

static int
hasstop (GPtrArray * lookin)
{
    unsigned int i;
    for (i = 0; i < lookin->len; i++) {
        RsvgNode *node = g_ptr_array_index (lookin, i);
        if (rsvg_node_type (node) == RSVG_NODE_TYPE_STOP)
            return 1;
    }
    return 0;
}

typedef const char * (* GetFallbackFn) (RsvgNode *node);
typedef void (* ApplyFallbackFn) (RsvgNode *node, RsvgNode *fallback_node);

/* Some SVG paint servers can reference a "parent" or "fallback" paint server
 * through the xlink:href attribute (for example,
 * http://www.w3.org/TR/SVG11/pservers.html#LinearGradientElementHrefAttribute )
 * This is used to define a chain of properties to be resolved from each
 * fallback.
 */
static void
resolve_fallbacks (RsvgDrawingCtx *ctx,
                   RsvgNode *data,
                   RsvgNode *last_fallback,
                   GetFallbackFn get_fallback,
                   ApplyFallbackFn apply_fallback)
{
    RsvgNode *fallback;
    const char *fallback_id;

    fallback_id = get_fallback (last_fallback);
    if (fallback_id == NULL)
        return;
    fallback = rsvg_acquire_node (ctx, fallback_id);
    if (fallback == NULL)
      return;

    apply_fallback (data, fallback);
    resolve_fallbacks (ctx, data, fallback, get_fallback, apply_fallback);

    rsvg_release_node (ctx, fallback);
}

static const char *
gradient_get_fallback (RsvgNode *node)
{
    if (rsvg_node_type (node) == RSVG_NODE_TYPE_LINEAR_GRADIENT) {
        RsvgLinearGradient *g = (RsvgLinearGradient *) node;
        return g->fallback;
    } else if (rsvg_node_type (node) == RSVG_NODE_TYPE_RADIAL_GRADIENT) {
        RsvgRadialGradient *g = (RsvgRadialGradient *) node;
        return g->fallback;
    } else
        return NULL;
}

static void
linear_gradient_apply_fallback (RsvgNode *node, RsvgNode *fallback_node)
{
    RsvgLinearGradient *grad;

    g_assert (rsvg_node_type (node) == RSVG_NODE_TYPE_LINEAR_GRADIENT);
    grad = (RsvgLinearGradient *) node;

    if (rsvg_node_type (fallback_node) == RSVG_NODE_TYPE_LINEAR_GRADIENT) {
        RsvgLinearGradient *fallback = (RsvgLinearGradient *) fallback_node;

        if (!grad->hasx1 && fallback->hasx1) {
            grad->hasx1 = TRUE;
            grad->x1 = fallback->x1;
        }
        if (!grad->hasy1 && fallback->hasy1) {
            grad->hasy1 = TRUE;
            grad->y1 = fallback->y1;
        }
        if (!grad->hasx2 && fallback->hasx2) {
            grad->hasx2 = TRUE;
            grad->x2 = fallback->x2;
        }
        if (!grad->hasy2 && fallback->hasy2) {
            grad->hasy2 = TRUE;
            grad->y2 = fallback->y2;
        }
        if (!grad->hastransform && fallback->hastransform) {
            grad->hastransform = TRUE;
            grad->affine = fallback->affine;
        }
        if (!grad->hasspread && fallback->hasspread) {
            grad->hasspread = TRUE;
            grad->spread = fallback->spread;
        }
        if (!grad->hasbbox && fallback->hasbbox) {
            grad->hasbbox = TRUE;
            grad->obj_bbox = fallback->obj_bbox;
        }
        if (!hasstop (grad->super.children) && hasstop (fallback->super.children)) {
            grad->super.children = fallback->super.children;
        }
    } else if (rsvg_node_type (fallback_node) == RSVG_NODE_TYPE_RADIAL_GRADIENT) {
        RsvgRadialGradient *fallback = (RsvgRadialGradient *) fallback_node;

        if (!grad->hastransform && fallback->hastransform) {
            grad->hastransform = TRUE;
            grad->affine = fallback->affine;
        }
        if (!grad->hasspread && fallback->hasspread) {
            grad->hasspread = TRUE;
            grad->spread = fallback->spread;
        }
        if (!grad->hasbbox && fallback->hasbbox) {
            grad->hasbbox = TRUE;
            grad->obj_bbox = fallback->obj_bbox;
        }
        if (!hasstop (grad->super.children) && hasstop (fallback->super.children)) {
            grad->super.children = fallback->super.children;
        }
    }
}

void
rsvg_linear_gradient_fix_fallback (RsvgDrawingCtx *ctx, RsvgLinearGradient * grad)
{
    resolve_fallbacks (ctx,
                       (RsvgNode *) grad,
                       (RsvgNode *) grad,
                       gradient_get_fallback,
                       linear_gradient_apply_fallback);
}

static void
radial_gradient_apply_fallback (RsvgNode *node, RsvgNode *fallback_node)
{
    RsvgRadialGradient *grad;

    g_assert (rsvg_node_type (node) == RSVG_NODE_TYPE_RADIAL_GRADIENT);
    grad = (RsvgRadialGradient *) node;

    if (rsvg_node_type (fallback_node) == RSVG_NODE_TYPE_RADIAL_GRADIENT) {
        RsvgRadialGradient *fallback = (RsvgRadialGradient *) fallback_node;

        if (!grad->hascx && fallback->hascx) {
            grad->hascx = TRUE;
            grad->cx = fallback->cx;
        }
        if (!grad->hascy && fallback->hascy) {
            grad->hascy = TRUE;
            grad->cy = fallback->cy;
        }
        if (!grad->hasfx && fallback->hasfx) {
            grad->hasfx = TRUE;
            grad->fx = fallback->fx;
        }
        if (!grad->hasfy && fallback->hasfy) {
            grad->hasfy = TRUE;
            grad->fy = fallback->fy;
        }
        if (!grad->hasr && fallback->hasr) {
            grad->hasr = TRUE;
            grad->r = fallback->r;
        }
        if (!grad->hastransform && fallback->hastransform) {
            grad->hastransform = TRUE;
            grad->affine = fallback->affine;
        }
        if (!grad->hasspread && fallback->hasspread) {
            grad->hasspread = TRUE;
            grad->spread = fallback->spread;
        }
        if (!grad->hasbbox && fallback->hasbbox) {
            grad->hasbbox = TRUE;
            grad->obj_bbox = fallback->obj_bbox;
        }
        if (!hasstop (grad->super.children) && hasstop (fallback->super.children)) {
            grad->super.children = fallback->super.children;
        }
    } else if (rsvg_node_type (fallback_node) == RSVG_NODE_TYPE_LINEAR_GRADIENT) {
        RsvgLinearGradient *fallback = (RsvgLinearGradient *) fallback_node;

        if (!grad->hastransform && fallback->hastransform) {
            grad->hastransform = TRUE;
            grad->affine = fallback->affine;
        }
        if (!grad->hasspread && fallback->hasspread) {
            grad->hasspread = TRUE;
            grad->spread = fallback->spread;
        }
        if (!grad->hasbbox && fallback->hasbbox) {
            grad->hasbbox = TRUE;
            grad->obj_bbox = fallback->obj_bbox;
        }
        if (!hasstop (grad->super.children) && hasstop (fallback->super.children)) {
            grad->super.children = fallback->super.children;
        }
    }
}

void
rsvg_radial_gradient_fix_fallback (RsvgDrawingCtx *ctx, RsvgRadialGradient * grad)
{
    resolve_fallbacks (ctx,
                       (RsvgNode *) grad,
                       (RsvgNode *) grad,
                       gradient_get_fallback,
                       radial_gradient_apply_fallback);
}

static const char *
pattern_get_fallback (RsvgNode *node)
{
    if (rsvg_node_type (node) == RSVG_NODE_TYPE_PATTERN) {
        RsvgPattern *pattern = (RsvgPattern *) node;

        return pattern->fallback;
    } else
        return NULL;
}

static void
pattern_apply_fallback (RsvgNode *pattern_node, RsvgNode *fallback_node)
{
    RsvgPattern *pattern;
    RsvgPattern *fallback;

    g_assert (rsvg_node_type (pattern_node) == RSVG_NODE_TYPE_PATTERN);

    if (rsvg_node_type (fallback_node) != RSVG_NODE_TYPE_PATTERN)
        return;

    pattern = (RsvgPattern *) pattern_node;
    fallback = (RsvgPattern *) fallback_node;

    if (!pattern->hasx && fallback->hasx) {
        pattern->hasx = TRUE;
        pattern->x = fallback->x;
    }
    if (!pattern->hasy && fallback->hasy) {
        pattern->hasy = TRUE;
        pattern->y = fallback->y;
    }
    if (!pattern->haswidth && fallback->haswidth) {
        pattern->haswidth = TRUE;
        pattern->width = fallback->width;
    }
    if (!pattern->hasheight && fallback->hasheight) {
        pattern->hasheight = TRUE;
        pattern->height = fallback->height;
    }
    if (!pattern->hastransform && fallback->hastransform) {
        pattern->hastransform = TRUE;
        pattern->affine = fallback->affine;
    }
    if (!pattern->hasvbox && fallback->hasvbox) {
        pattern->vbox = fallback->vbox;
    }
    if (!pattern->hasaspect && fallback->hasaspect) {
        pattern->hasaspect = TRUE;
        pattern->preserve_aspect_ratio = fallback->preserve_aspect_ratio;
    }
    if (!pattern->hasbbox && fallback->hasbbox) {
        pattern->hasbbox = TRUE;
        pattern->obj_bbox = fallback->obj_bbox;
    }
    if (!pattern->hascbox && fallback->hascbox) {
        pattern->hascbox = TRUE;
        pattern->obj_cbbox = fallback->obj_cbbox;
    }
    if (!pattern->super.children->len && fallback->super.children->len) {
        pattern->super.children = fallback->super.children;
    }
}

void
rsvg_pattern_fix_fallback (RsvgDrawingCtx *ctx, RsvgPattern * pattern)
{
    resolve_fallbacks (ctx,
                       (RsvgNode *) pattern,
                       (RsvgNode *) pattern,
                       pattern_get_fallback,
                       pattern_apply_fallback);
}
