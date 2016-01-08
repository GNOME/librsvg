/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-marker.c: Marker loading and rendering

   Copyright (C) 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "rsvg-marker.h"
#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-css.h"
#include "rsvg-defs.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-image.h"
#include "rsvg-path.h"

#include <string.h>
#include <math.h>
#include <errno.h>

static void
rsvg_node_marker_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgMarker *marker;
    marker = (RsvgMarker *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &marker->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
            marker->vbox = rsvg_css_parse_vbox (value);
        if ((value = rsvg_property_bag_lookup (atts, "refX")))
            marker->refX = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "refY")))
            marker->refY = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "markerWidth")))
            marker->width = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "markerHeight")))
            marker->height = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "orient"))) {
            if (!strcmp (value, "auto"))
                marker->orientAuto = TRUE;
            else
                marker->orient = rsvg_css_parse_angle (value);
        }
        if ((value = rsvg_property_bag_lookup (atts, "markerUnits"))) {
            if (!strcmp (value, "userSpaceOnUse"))
                marker->bbox = FALSE;
            if (!strcmp (value, "strokeWidth"))
                marker->bbox = TRUE;
        }
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            marker->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
        rsvg_parse_style_attrs (ctx, self->state, "marker", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_marker (void)
{
    RsvgMarker *marker;
    marker = g_new (RsvgMarker, 1);
    _rsvg_node_init (&marker->super, RSVG_NODE_TYPE_MARKER);
    marker->orient = 0;
    marker->orientAuto = FALSE;
    marker->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    marker->refX = marker->refY = _rsvg_css_parse_length ("0");
    marker->width = marker->height = _rsvg_css_parse_length ("3");
    marker->bbox = TRUE;
    marker->vbox.active = FALSE;
    marker->super.set_atts = rsvg_node_marker_set_atts;
    return &marker->super;
}

static void
rsvg_marker_render (const char * marker_name, gdouble xpos, gdouble ypos, gdouble orient, gdouble linewidth,
                    RsvgDrawingCtx * ctx)
{
    RsvgMarker *self;
    cairo_matrix_t affine, taffine;
    unsigned int i;
    gdouble rotation;
    RsvgState *state = rsvg_current_state (ctx);

    if (marker_name == NULL)
        return; /* to avoid the caller having to check for nonexistent markers on every call */

    self = (RsvgMarker *) rsvg_acquire_node (ctx, marker_name);
    if (self == NULL || RSVG_NODE_TYPE (&self->super) != RSVG_NODE_TYPE_MARKER)
      {
        rsvg_release_node (ctx, &self->super);
        return;
      }

    cairo_matrix_init_translate (&taffine, xpos, ypos);
    cairo_matrix_multiply (&affine, &taffine, &state->affine);

    if (self->orientAuto)
        rotation = orient;
    else
        rotation = self->orient * M_PI / 180.;

    cairo_matrix_init_rotate (&taffine, rotation);
    cairo_matrix_multiply (&affine, &taffine, &affine);

    if (self->bbox) {
        cairo_matrix_init_scale (&taffine, linewidth, linewidth);
        cairo_matrix_multiply (&affine, &taffine, &affine);
    }

    if (self->vbox.active) {
        double w, h, x, y;
        w = _rsvg_css_normalize_length (&self->width, ctx, 'h');
        h = _rsvg_css_normalize_length (&self->height, ctx, 'v');
        x = 0;
        y = 0;

        rsvg_preserve_aspect_ratio (self->preserve_aspect_ratio,
                                    self->vbox.rect.width,
                                    self->vbox.rect.height,
                                    &w, &h, &x, &y);

        cairo_matrix_init_scale (&taffine, w / self->vbox.rect.width, h / self->vbox.rect.height);
        cairo_matrix_multiply (&affine, &taffine, &affine);

        _rsvg_push_view_box (ctx, self->vbox.rect.width, self->vbox.rect.height);
    }

    cairo_matrix_init_translate (&taffine,
                                 -_rsvg_css_normalize_length (&self->refX, ctx, 'h'),
                                 -_rsvg_css_normalize_length (&self->refY, ctx, 'v'));
    cairo_matrix_multiply (&affine, &taffine, &affine);

    rsvg_state_push (ctx);
    state = rsvg_current_state (ctx);

    rsvg_state_reinit (state);

    rsvg_state_reconstruct (state, &self->super);

    state->affine = affine;

    rsvg_push_discrete_layer (ctx);

    state = rsvg_current_state (ctx);

    if (!state->overflow) {
        if (self->vbox.active)
            rsvg_add_clipping_rect (ctx, self->vbox.rect.x, self->vbox.rect.y,
                                    self->vbox.rect.width, self->vbox.rect.height);
        else
            rsvg_add_clipping_rect (ctx, 0, 0,
                                    _rsvg_css_normalize_length (&self->width, ctx, 'h'),
                                    _rsvg_css_normalize_length (&self->height, ctx, 'v'));
    }

    for (i = 0; i < self->super.children->len; i++) {
        rsvg_state_push (ctx);

        rsvg_node_draw (g_ptr_array_index (self->super.children, i), ctx, 0);

        rsvg_state_pop (ctx);
    }
    rsvg_pop_discrete_layer (ctx);

    rsvg_state_pop (ctx);
    if (self->vbox.active)
        _rsvg_pop_view_box (ctx);

    rsvg_release_node (ctx, (RsvgNode *) self);
}

typedef struct {
    gboolean is_degenerate; /* If true, only (p1x, p1y) are valid.  If false, all are valid */
    double p1x, p1y;
    double p2x, p2y;
    double p3x, p3y;
    double p4x, p4y;
} Segment;

typedef enum {
    SEGMENT_START,
    SEGMENT_END,
} SegmentState;

/* This converts a cairo_path_t into a list of curveto-like segments.  Each segment can be:
 *
 * 1. segment->is_degenerate = TRUE => the segment is actually a single point (segment->p1x, segment->p1y)
 *
 * 2. segment->is_degenerate = FALSE => either a lineto or a curveto (or the effective lineto that results from a closepath).
 *    We have the following points:
 *       P1 = (p1x, p1y)
 *       P2 = (p2x, p2y)
 *       P3 = (p3x, p3y)
 *       P4 = (p4x, p4y)
 *
 *    The start and end points are P1 and P4, respectively.
 *    The tangent at the start point is given by the vector (P2 - P1).
 *    The tangent at the end point is given by the vector (P4 - P3).
 *    The tangents also work if the segment refers to a lineto (they will both just point in the same direction).
 */

#define EPSILON 1e-10
#define DOUBLE_EQUALS(a, b) (fabs ((a) - (b)) < EPSILON)

static void
path_to_segments (const cairo_path_t *path,
                  Segment **out_segments,
                  int *num_segments)
{
    int i;
    double last_x, last_y;
    double cur_x, cur_y;
    double subpath_start_x, subpath_start_y;
    int max_segments;
    int segment_num;
    Segment *segments;
    SegmentState state;

    max_segments = path->num_data; /* We'll generate maximum this many segments */
    segments = g_new (Segment, max_segments);
    *out_segments = segments;

    last_x = last_y = cur_x = cur_y = subpath_start_x = subpath_start_y = 0.0;

    segment_num = -1;
    state = SEGMENT_END;

    for (i = 0; i < path->num_data; i += path->data[i].header.length) {
        last_x = cur_x;
        last_y = cur_y;

        switch (path->data[i].header.type) {
        case CAIRO_PATH_MOVE_TO:
            segment_num++;
            g_assert (segment_num < max_segments);

            g_assert (i + 1 < path->num_data);
            cur_x = path->data[i + 1].point.x;
            cur_y = path->data[i + 1].point.y;

            subpath_start_x = cur_x;
            subpath_start_y = cur_y;

            segments[segment_num].is_degenerate = TRUE;

            segments[segment_num].p1x = cur_x;
            segments[segment_num].p1y = cur_y;

            state = SEGMENT_START;

            break;

        case CAIRO_PATH_LINE_TO:
            g_assert (i + 1 < path->num_data);
            cur_x = path->data[i + 1].point.x;
            cur_y = path->data[i + 1].point.y;

            if (state == SEGMENT_START) {
                segments[segment_num].is_degenerate = FALSE;
                state = SEGMENT_END;
            } else {
                segment_num++;
                g_assert (segment_num < max_segments);

                segments[segment_num].is_degenerate = FALSE;

                segments[segment_num].p1x = last_x;
                segments[segment_num].p1y = last_y;
            }

            segments[segment_num].p2x = cur_x;
            segments[segment_num].p2y = cur_y;

            segments[segment_num].p3x = last_x;
            segments[segment_num].p3y = last_y;

            segments[segment_num].p4x = cur_x;
            segments[segment_num].p4y = cur_y;

            break;

        case CAIRO_PATH_CURVE_TO:
            g_assert (i + 3 < path->num_data);
            cur_x = path->data[i + 3].point.x;
            cur_y = path->data[i + 3].point.y;

            if (state == SEGMENT_START) {
                segments[segment_num].is_degenerate = FALSE;
                state = SEGMENT_END;
            } else {
                segment_num++;
                g_assert (segment_num < max_segments);

                segments[segment_num].is_degenerate = FALSE;

                segments[segment_num].p1x = last_x;
                segments[segment_num].p1y = last_y;
            }

            segments[segment_num].p2x = path->data[i + 1].point.x;
            segments[segment_num].p2y = path->data[i + 1].point.y;

            segments[segment_num].p3x = path->data[i + 2].point.x;
            segments[segment_num].p3y = path->data[i + 2].point.y;

            segments[segment_num].p4x = cur_x;
            segments[segment_num].p4y = cur_y;

            /* Fix the tangents for when the middle control points coincide with their respective endpoints */

            if (DOUBLE_EQUALS (segments[segment_num].p2x, segments[segment_num].p1x)
                && DOUBLE_EQUALS (segments[segment_num].p2y, segments[segment_num].p1y)) {
                segments[segment_num].p2x = segments[segment_num].p3x;
                segments[segment_num].p2y = segments[segment_num].p3y;
            }

            if (DOUBLE_EQUALS (segments[segment_num].p3x, segments[segment_num].p4x)
                && DOUBLE_EQUALS (segments[segment_num].p3y, segments[segment_num].p4y)) {
                segments[segment_num].p3x = segments[segment_num].p2x;
                segments[segment_num].p3y = segments[segment_num].p2y;
            }

            break;

        case CAIRO_PATH_CLOSE_PATH:
            cur_x = subpath_start_x;
            cur_y = subpath_start_y;

            if (state == SEGMENT_START) {
                segments[segment_num].is_degenerate = FALSE;

                segments[segment_num].p2x = cur_x;
                segments[segment_num].p2y = cur_y;

                segments[segment_num].p3x = last_x;
                segments[segment_num].p3y = last_y;

                segments[segment_num].p4x = cur_x;
                segments[segment_num].p4y = cur_y;

                state = SEGMENT_END;
            } else {
                /* nothing; closepath after moveto (or a single lone closepath) does nothing */
            }

            break;

        default:
            g_assert_not_reached ();
        }
    }

    *num_segments = segment_num + 1;
    g_assert (*num_segments <= max_segments);
}

static gboolean
points_equal (double x1, double y1, double x2, double y2)
{
    return DOUBLE_EQUALS (x1, x2) && DOUBLE_EQUALS (y1, y2);
}

/* A segment is zero length if it is degenerate, or if all four control points
 * coincide (the first and last control points may coincide, but the others may
 * define a loop - thus nonzero length)
 */
static gboolean
is_zero_length_segment (Segment *segment)
{
    double p1x, p1y;
    double p2x, p2y;
    double p3x, p3y;
    double p4x, p4y;

    if (segment->is_degenerate)
        return TRUE;

    p1x = segment->p1x;
    p1y = segment->p1y;

    p2x = segment->p2x;
    p2y = segment->p2y;

    p3x = segment->p3x;
    p3y = segment->p3y;

    p4x = segment->p4x;
    p4y = segment->p4y;

    return (points_equal (p1x, p1y, p2x, p2y)
            && points_equal (p1x, p1y, p3x, p3y)
            && points_equal (p1x, p1y, p4x, p4y));
}

/* The SVG spec 1.1 says http://www.w3.org/TR/SVG/implnote.html#PathElementImplementationNotes
 *
 * Certain line-capping and line-joining situations and markers
 * require that a path segment have directionality at its start and
 * end points. Zero-length path segments have no directionality. In
 * these cases, the following algorithm is used to establish
 * directionality:  to determine the directionality of the start
 * point of a zero-length path segment, go backwards in the path
 * data specification within the current subpath until you find a
 * segment which has directionality at its end point (e.g., a path
 * segment with non-zero length) and use its ending direction;
 * otherwise, temporarily consider the start point to lack
 * directionality. Similarly, to determine the directionality of the
 * end point of a zero-length path segment, go forwards in the path
 * data specification within the current subpath until you find a
 * segment which has directionality at its start point (e.g., a path
 * segment with non-zero length) and use its starting direction;
 * otherwise, temporarily consider the end point to lack
 * directionality. If the start point has directionality but the end
 * point doesn't, then the end point uses the start point's
 * directionality. If the end point has directionality but the start
 * point doesn't, then the start point uses the end point's
 * directionality. Otherwise, set the directionality for the path
 * segment's start and end points to align with the positive x-axis
 * in user space.
 */
static gboolean
find_incoming_directionality_backwards (Segment *segments, int num_segments, int start_index, double *vx, double *vy)
{
    int j;
    gboolean found;

    /* "go backwards ... within the current subpath until ... segment which has directionality at its end point" */

    found = FALSE;

    for (j = start_index; j >= 0; j--) {
        if (segments[j].is_degenerate)
            break; /* reached the beginning of the subpath as we ran into a standalone point */
        else {
            if (is_zero_length_segment (&segments[j]))
                continue;
            else {
                found = TRUE;
                break;
            }
        }
    }

    if (found) {
        g_assert (j >= 0);
        *vx = segments[j].p4x - segments[j].p3x;
        *vy = segments[j].p4y - segments[j].p3y;
        return TRUE;
    } else {
        *vx = 0.0;
        *vy = 0.0;
        return FALSE;
    }
}

static gboolean
find_outgoing_directionality_forwards (Segment *segments, int num_segments, int start_index, double *vx, double *vy)
{
    int j;
    gboolean found;

    /* "go forwards ... within the current subpath until ... segment which has directionality at its start point" */

    found = FALSE;

    for (j = start_index; j < num_segments; j++) {
        if (segments[j].is_degenerate)
            break; /* reached the end of a subpath as we ran into a standalone point */
        else {
            if (is_zero_length_segment (&segments[j]))
                continue;
            else {
                found = TRUE;
                break;
            }
        }
    }

    if (found) {
        g_assert (j < num_segments);
        *vx = segments[j].p2x - segments[j].p1x;
        *vy = segments[j].p2y - segments[j].p1y;
        return TRUE;
    } else {
        *vx = 0.0;
        *vy = 0.0;
        return FALSE;
    }
}

static double
angle_from_vector (double vx, double vy)
{
    double angle;

    angle = atan2 (vy, vx);

    if (isnan (angle))
        return 0.0;
    else
        return angle;
}

typedef enum {
    NO_SUBPATH,
    IN_SUBPATH,
} SubpathState;

void
rsvg_render_markers (RsvgDrawingCtx * ctx,
                     const cairo_path_t *path)
{
    RsvgState *state;
    double linewidth;
    const char *startmarker;
    const char *middlemarker;
    const char *endmarker;

    int i;
    double incoming_vx, incoming_vy;
    double outgoing_vx, outgoing_vy;

    Segment *segments;
    int num_segments;

    SubpathState subpath_state;

    state = rsvg_current_state (ctx);

    linewidth = _rsvg_css_normalize_length (&state->stroke_width, ctx, 'o');
    startmarker = state->startMarker;
    middlemarker = state->middleMarker;
    endmarker = state->endMarker;

    if (linewidth == 0)
        return;

    if (!startmarker && !middlemarker && !endmarker)
        return;

    if (path->num_data <= 0)
        return;

    /* Convert the path to a list of segments and bare points (i.e. degenerate segments) */
    path_to_segments (path, &segments, &num_segments);

    subpath_state = NO_SUBPATH;

    for (i = 0; i < num_segments; i++) {
        incoming_vx = incoming_vy = outgoing_vx = outgoing_vy = 0.0;

        if (segments[i].is_degenerate) {
            if (subpath_state == IN_SUBPATH) {
                g_assert (i > 0);

                /* Got a lone point after a subpath; render the subpath's end marker first */

                find_incoming_directionality_backwards (segments, num_segments, i - 1, &incoming_vx, &incoming_vy);
                rsvg_marker_render (endmarker, segments[i - 1].p4x, segments[i - 1].p4y, angle_from_vector (incoming_vx, incoming_vy), linewidth, ctx);
            }

            /* Render marker for the lone point; no directionality */
            rsvg_marker_render (middlemarker, segments[i].p1x, segments[i].p1y, 0.0, linewidth, ctx);

            subpath_state = NO_SUBPATH;
        } else {
            /* Not a degenerate segment */

            if (subpath_state == NO_SUBPATH) {
                find_outgoing_directionality_forwards (segments, num_segments, i, &outgoing_vx, &outgoing_vy);
                rsvg_marker_render (startmarker, segments[i].p1x, segments[i].p1y, angle_from_vector (outgoing_vx, outgoing_vy), linewidth, ctx);

                subpath_state = IN_SUBPATH;
            } else {
                /* subpath_state == IN_SUBPATH */

                gboolean has_incoming, has_outgoing;
                double incoming, outgoing;
                double angle;

                g_assert (i > 0);

                has_incoming = find_incoming_directionality_backwards (segments, num_segments, i - 1, &incoming_vx, &incoming_vy);
                has_outgoing = find_outgoing_directionality_forwards (segments, num_segments, i, &outgoing_vx, &outgoing_vy);

                if (has_incoming)
                    incoming = angle_from_vector (incoming_vx, incoming_vy);

                if (has_outgoing)
                    outgoing = angle_from_vector (outgoing_vx, outgoing_vy);

                if (has_incoming && has_outgoing)
                    angle = (incoming + outgoing) / 2;
                else if (has_incoming)
                    angle = incoming;
                else if (has_outgoing)
                    angle = outgoing;
                else
                    angle = 0.0;

                rsvg_marker_render (middlemarker, segments[i].p1x, segments[i].p1y, angle, linewidth, ctx);
            }
        }
    }

    /* Finally, render the last point */

    if (num_segments > 0) {
        if (!segments[num_segments - 1].is_degenerate) {
            find_incoming_directionality_backwards (segments, num_segments, num_segments - 1, &incoming_vx, &incoming_vy);

            rsvg_marker_render (endmarker, segments[num_segments - 1].p4x, segments[num_segments - 1].p4y, angle_from_vector (incoming_vx, incoming_vy), linewidth, ctx);
        }
    }

    g_free (segments);
}
