/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/* 
   rsvg-path.c: Parse SVG path element data into bezier path.
 
   Copyright (C) 2000 Eazel, Inc.
   Copyright © 2011 Christian Persch
   Copyright (C) 2016 Federico Mena Quintero <federico@gnome.org>
  
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
  
   Authors: Raph Levien <raph@artofcode.com>
            F. Wang <fred.wang@free.fr> - fix drawing of arc
            Federico Mena Quintero <federico@gnome.org>
*/

#include "config.h"
#include "rsvg-path.h"

#include <glib.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "rsvg-private.h"

/* This module provides a general-purpose RsvgPathBuilder to build Cairo paths
   gradually.  It also provides a high-level utility function to parses an SVG path element
   into an cairo_path_t.

   Reference: https://www.w3.org/TR/SVG/paths.html#PathDataBNF
*/

typedef struct _RsvgPathBuilder RsvgPathBuilder; /* This is in rust/src/path_builder.rs */

typedef struct {
    RsvgPathBuilder *builder;

    double cp_x, cp_y;          /* current point */
    double rp_x, rp_y;          /* reflection point (for 's' and 't' commands) */
    double last_moveto_x, last_moveto_y; /* Location of last move_to command, so close_path can go back to it */
    char cmd;                   /* current command (lowercase) */
    int param;                  /* parameter number */
    gboolean rel;               /* true if relative coords */
    double params[7];           /* parameters that have been parsed */
} RSVGParsePathCtx;

static void
rsvg_path_arc_segment (RsvgPathBuilder *builder,
                       double xc, double yc,
                       double th0, double th1, double rx, double ry,
                       double x_axis_rotation)
{
    double x1, y1, x2, y2, x3, y3;
    double t;
    double th_half;
    double f, sinf, cosf;

    f = x_axis_rotation * M_PI / 180.0;
    sinf = sin(f);
    cosf = cos(f);

    th_half = 0.5 * (th1 - th0);
    t = (8.0 / 3.0) * sin (th_half * 0.5) * sin (th_half * 0.5) / sin (th_half);
    x1 = rx*(cos (th0) - t * sin (th0));
    y1 = ry*(sin (th0) + t * cos (th0));
    x3 = rx*cos (th1);
    y3 = ry*sin (th1);
    x2 = x3 + rx*(t * sin (th1));
    y2 = y3 + ry*(-t * cos (th1));

    rsvg_path_builder_curve_to (builder,
                                xc + cosf*x1 - sinf*y1,
                                yc + sinf*x1 + cosf*y1,
                                xc + cosf*x2 - sinf*y2,
                                yc + sinf*x2 + cosf*y2,
                                xc + cosf*x3 - sinf*y3,
                                yc + sinf*x3 + cosf*y3);
}

/**
 * rsvg_path_builder_arc:
 * @builder: Path builder.
 * @x1: Starting x coordinate
 * @y1: Starting y coordinate
 * @rx: Radius in x direction (before rotation).
 * @ry: Radius in y direction (before rotation).
 * @x_axis_rotation: Rotation angle for axes.
 * @large_arc_flag: 0 for arc length <= 180, 1 for arc >= 180.
 * @sweep_flag: 0 for "negative angle", 1 for "positive angle".
 * @x2: Ending x coordinate
 * @y2: Ending y coordinate
 *
 * Add an RSVG arc to the path context.
 **/
void
rsvg_path_builder_arc (RsvgPathBuilder *builder,
                       double x1, double y1,
                       double rx, double ry,
                       double x_axis_rotation,
                       gboolean large_arc_flag, gboolean sweep_flag,
                       double x2, double y2)
{

    /* See Appendix F.6 Elliptical arc implementation notes
       http://www.w3.org/TR/SVG/implnote.html#ArcImplementationNotes */

    double f, sinf, cosf;
    double x1_, y1_;
    double cx_, cy_, cx, cy;
    double gamma;
    double theta1, delta_theta;
    double k1, k2, k3, k4, k5;

    int i, n_segs;

    if (x1 == x2 && y1 == y2)
        return;

    /* X-axis */
    f = x_axis_rotation * M_PI / 180.0;
    sinf = sin (f);
    cosf = cos (f);

    rx = fabs (rx);
    ry = fabs (ry);

    /* Check the radius against floading point underflow.
       See http://bugs.debian.org/508443 */
    if ((rx < DBL_EPSILON) || (ry < DBL_EPSILON)) {
        rsvg_path_builder_line_to (builder, x2, y2);
        return;
    }

    k1 = (x1 - x2) / 2;
    k2 = (y1 - y2) / 2;

    x1_ = cosf * k1 + sinf * k2;
    y1_ = -sinf * k1 + cosf * k2;

    gamma = (x1_ * x1_) / (rx * rx) + (y1_ * y1_) / (ry * ry);
    if (gamma > 1) {
        rx *= sqrt (gamma);
        ry *= sqrt (gamma);
    }

    /* Compute the center */

    k1 = rx * rx * y1_ * y1_ + ry * ry * x1_ * x1_;
    if (k1 == 0)
        return;

    k1 = sqrt (fabs ((rx * rx * ry * ry) / k1 - 1));
    if (sweep_flag == large_arc_flag)
        k1 = -k1;

    cx_ = k1 * rx * y1_ / ry;
    cy_ = -k1 * ry * x1_ / rx;
    
    cx = cosf * cx_ - sinf * cy_ + (x1 + x2) / 2;
    cy = sinf * cx_ + cosf * cy_ + (y1 + y2) / 2;

    /* Compute start angle */

    k1 = (x1_ - cx_) / rx;
    k2 = (y1_ - cy_) / ry;
    k3 = (-x1_ - cx_) / rx;
    k4 = (-y1_ - cy_) / ry;

    k5 = sqrt (fabs (k1 * k1 + k2 * k2));
    if (k5 == 0)
        return;

    k5 = k1 / k5;
    k5 = CLAMP (k5, -1, 1);
    theta1 = acos (k5);
    if (k2 < 0)
        theta1 = -theta1;

    /* Compute delta_theta */

    k5 = sqrt (fabs ((k1 * k1 + k2 * k2) * (k3 * k3 + k4 * k4)));
    if (k5 == 0)
        return;

    k5 = (k1 * k3 + k2 * k4) / k5;
    k5 = CLAMP (k5, -1, 1);
    delta_theta = acos (k5);
    if (k1 * k4 - k3 * k2 < 0)
        delta_theta = -delta_theta;

    if (sweep_flag && delta_theta < 0)
        delta_theta += M_PI * 2;
    else if (!sweep_flag && delta_theta > 0)
        delta_theta -= M_PI * 2;
   
    /* Now draw the arc */

    n_segs = ceil (fabs (delta_theta / (M_PI * 0.5 + 0.001)));

    for (i = 0; i < n_segs; i++)
        rsvg_path_arc_segment (builder, cx, cy,
                               theta1 + i * delta_theta / n_segs,
                               theta1 + (i + 1) * delta_theta / n_segs,
                               rx, ry, x_axis_rotation);
}


/* supply defaults for missing parameters, assuming relative coordinates
   are to be interpreted as x,y */
static void
rsvg_parse_path_default_xy (RSVGParsePathCtx * ctx, int n_params)
{
    int i;

    if (ctx->rel) {
        for (i = ctx->param; i < n_params; i++) {
            /* we shouldn't get 0 here (usually ctx->param > 0 as
               precondition) */
            if (i == 0)
                ctx->params[i] = ctx->cp_x;
            else if (i == 1)
                ctx->params[i] = ctx->cp_y;
            else
                ctx->params[i] = ctx->params[i - 2];
        }
    } else {
        for (i = ctx->param; i < n_params; i++)
            ctx->params[i] = 0.0;
    }
}

static void
rsvg_parse_path_do_cmd (RSVGParsePathCtx * ctx, gboolean final)
{
    double x1, y1, x2, y2, x3, y3;

    switch (ctx->cmd) {
    case 'm':
        /* moveto */
        if (ctx->param == 2 || final) {
            rsvg_parse_path_default_xy (ctx, 2);
            rsvg_path_builder_move_to (ctx->builder, ctx->params[0], ctx->params[1]);
            ctx->cp_x = ctx->rp_x = ctx->params[0];
            ctx->cp_y = ctx->rp_y = ctx->params[1];
            ctx->last_moveto_x = ctx->cp_x;
            ctx->last_moveto_y = ctx->cp_y;
            ctx->param = 0;
            ctx->cmd = 'l'; /* implicit linetos after a moveto */
        }
        break;
    case 'l':
        /* lineto */
        if (ctx->param == 2 || final) {
            rsvg_parse_path_default_xy (ctx, 2);
            rsvg_path_builder_line_to (ctx->builder, ctx->params[0], ctx->params[1]);
            ctx->cp_x = ctx->rp_x = ctx->params[0];
            ctx->cp_y = ctx->rp_y = ctx->params[1];
            ctx->param = 0;
        }
        break;
    case 'c':
        /* curveto */
        if (ctx->param == 6 || final) {
            rsvg_parse_path_default_xy (ctx, 6);
            x1 = ctx->params[0];
            y1 = ctx->params[1];
            x2 = ctx->params[2];
            y2 = ctx->params[3];
            x3 = ctx->params[4];
            y3 = ctx->params[5];
            rsvg_path_builder_curve_to (ctx->builder, x1, y1, x2, y2, x3, y3);
            ctx->rp_x = x2;
            ctx->rp_y = y2;
            ctx->cp_x = x3;
            ctx->cp_y = y3;
            ctx->param = 0;
        }
        break;
    case 's':
        /* smooth curveto */
        if (ctx->param == 4 || final) {
            rsvg_parse_path_default_xy (ctx, 4);
            x1 = 2 * ctx->cp_x - ctx->rp_x;
            y1 = 2 * ctx->cp_y - ctx->rp_y;
            x2 = ctx->params[0];
            y2 = ctx->params[1];
            x3 = ctx->params[2];
            y3 = ctx->params[3];
            rsvg_path_builder_curve_to (ctx->builder, x1, y1, x2, y2, x3, y3);
            ctx->rp_x = x2;
            ctx->rp_y = y2;
            ctx->cp_x = x3;
            ctx->cp_y = y3;
            ctx->param = 0;
        }
        break;
    case 'h':
        /* horizontal lineto */
        if (ctx->param == 1) {
            rsvg_path_builder_line_to (ctx->builder, ctx->params[0], ctx->cp_y);
            ctx->cp_x = ctx->rp_x = ctx->params[0];
            ctx->rp_y = ctx->cp_y;
            ctx->param = 0;
        }
        break;
    case 'v':
        /* vertical lineto */
        if (ctx->param == 1) {
            rsvg_path_builder_line_to (ctx->builder, ctx->cp_x, ctx->params[0]);
            ctx->rp_x = ctx->cp_x;
            ctx->cp_y = ctx->rp_y = ctx->params[0];
            ctx->param = 0;
        }
        break;
    case 'q':
        /* quadratic bezier curveto */

        /* non-normative reference:
           http://www.icce.rug.nl/erikjan/bluefuzz/beziers/beziers/beziers.html
         */
        if (ctx->param == 4 || final) {
            rsvg_parse_path_default_xy (ctx, 4);
            /* raise quadratic bezier to cubic */
            x1 = (ctx->cp_x + 2 * ctx->params[0]) * (1.0 / 3.0);
            y1 = (ctx->cp_y + 2 * ctx->params[1]) * (1.0 / 3.0);
            x3 = ctx->params[2];
            y3 = ctx->params[3];
            x2 = (x3 + 2 * ctx->params[0]) * (1.0 / 3.0);
            y2 = (y3 + 2 * ctx->params[1]) * (1.0 / 3.0);
            rsvg_path_builder_curve_to (ctx->builder, x1, y1, x2, y2, x3, y3);
            ctx->rp_x = ctx->params[0];
            ctx->rp_y = ctx->params[1];
            ctx->cp_x = x3;
            ctx->cp_y = y3;
            ctx->param = 0;
        }
        break;
    case 't':
        /* Truetype quadratic bezier curveto */
        if (ctx->param == 2 || final) {
            double xc, yc;      /* quadratic control point */

            xc = 2 * ctx->cp_x - ctx->rp_x;
            yc = 2 * ctx->cp_y - ctx->rp_y;
            /* generate a quadratic bezier with control point = xc, yc */
            x1 = (ctx->cp_x + 2 * xc) * (1.0 / 3.0);
            y1 = (ctx->cp_y + 2 * yc) * (1.0 / 3.0);
            x3 = ctx->params[0];
            y3 = ctx->params[1];
            x2 = (x3 + 2 * xc) * (1.0 / 3.0);
            y2 = (y3 + 2 * yc) * (1.0 / 3.0);
            rsvg_path_builder_curve_to (ctx->builder, x1, y1, x2, y2, x3, y3);
            ctx->rp_x = xc;
            ctx->rp_y = yc;
            ctx->cp_x = x3;
            ctx->cp_y = y3;
            ctx->param = 0;
        } else if (final) {
            if (ctx->param > 2) {
                rsvg_parse_path_default_xy (ctx, 4);
                /* raise quadratic bezier to cubic */
                x1 = (ctx->cp_x + 2 * ctx->params[0]) * (1.0 / 3.0);
                y1 = (ctx->cp_y + 2 * ctx->params[1]) * (1.0 / 3.0);
                x3 = ctx->params[2];
                y3 = ctx->params[3];
                x2 = (x3 + 2 * ctx->params[0]) * (1.0 / 3.0);
                y2 = (y3 + 2 * ctx->params[1]) * (1.0 / 3.0);
                rsvg_path_builder_curve_to (ctx->builder, x1, y1, x2, y2, x3, y3);
                ctx->rp_x = ctx->params[0];
                ctx->rp_y = ctx->params[1];
                ctx->cp_x = x3;
                ctx->cp_y = y3;
            } else {
                rsvg_parse_path_default_xy (ctx, 2);
                rsvg_path_builder_line_to (ctx->builder, ctx->params[0], ctx->params[1]);
                ctx->cp_x = ctx->rp_x = ctx->params[0];
                ctx->cp_y = ctx->rp_y = ctx->params[1];
            }
            ctx->param = 0;
        }
        break;
    case 'a':
        if (ctx->param == 7 || final) {
            double x1, y1;
            double rx, ry;
            double x_axis_rotation;
            gboolean large_arc_flag;
            gboolean sweep_flag;
            double x2, y2;

            x1 = ctx->cp_x;
            y1 = ctx->cp_y;

            rx = ctx->params[0];
            ry = ctx->params[1];

            x_axis_rotation = ctx->params[2];

            large_arc_flag = (ctx->params[3] == 0 ? FALSE : TRUE);
            sweep_flag = (ctx->params[4] == 0 ? FALSE : TRUE);

            x2 = ctx->params[5];
            y2 = ctx->params[6];

            rsvg_path_builder_arc (ctx->builder,
                                   x1, y1,
                                   rx, ry,
                                   x_axis_rotation,
                                   large_arc_flag,
                                   sweep_flag,
                                   x2, y2);

            ctx->rp_x = ctx->cp_x = x2;
            ctx->rp_y = ctx->cp_y = y2;

            ctx->param = 0;
        }
        break;
    default:
        ctx->param = 0;
    }
}

static void
rsvg_path_end_of_number (RSVGParsePathCtx * ctx, double val, int sign, int exp_sign, int exp)
{
    val *= sign * pow (10, exp_sign * exp);
    if (ctx->rel) {
        /* Handle relative coordinates. This switch statement attempts
           to determine _what_ the coords are relative to. This is
           underspecified in the 12 Apr working draft. */
        switch (ctx->cmd) {
        case 'l':
        case 'm':
        case 'c':
        case 's':
        case 'q':
        case 't':
            /* rule: even-numbered params are x-relative, odd-numbered
               are y-relative */
            if ((ctx->param & 1) == 0)
                val += ctx->cp_x;
            else if ((ctx->param & 1) == 1)
                val += ctx->cp_y;
            break;
        case 'a':
            /* rule: sixth and seventh are x and y, rest are not
               relative */
            if (ctx->param == 5)
                val += ctx->cp_x;
            else if (ctx->param == 6)
                val += ctx->cp_y;
            break;
        case 'h':
            /* rule: x-relative */
            val += ctx->cp_x;
            break;
        case 'v':
            /* rule: y-relative */
            val += ctx->cp_y;
            break;
        }
    }
    ctx->params[ctx->param++] = val;
    rsvg_parse_path_do_cmd (ctx, FALSE);    
}

#define RSVGN_IN_PREINTEGER  0
#define RSVGN_IN_INTEGER     1
#define RSVGN_IN_FRACTION    2
#define RSVGN_IN_PREEXPONENT 3
#define RSVGN_IN_EXPONENT    4

#define RSVGN_GOT_SIGN          0x1
#define RSVGN_GOT_EXPONENT_SIGN 0x2

/* Returns the length of the number parsed, so it can be skipped
 * in rsvg_parse_path_data. Calls rsvg_path_end_number to have the number
 * processed in its command.
 */
static int
rsvg_parse_number (RSVGParsePathCtx * ctx, const char *data)
{
    int length = 0;
    int in = RSVGN_IN_PREINTEGER; /* Current location within the number */
    int got = 0x0; /* [bitfield] Having 2 of each of these is an error */
    gboolean end = FALSE; /* Set to true if the number should end after a char */
    gboolean error = FALSE; /* Set to true if the number ended due to an error */

    double value = 0.0;
    double fraction = 1.0;
    int sign = +1; /* Presume the INTEGER is positive if it has no sign */
    int exponent = 0;
    int exponent_sign = +1; /* Presume the EXPONENT is positive if it has no sign */

    while (data[length] != '\0' && !end && !error) {
        char c = data[length];
        switch (in) {
            case RSVGN_IN_PREINTEGER: /* No numbers yet, we're just starting out */
                /* LEGAL: + - .->FRACTION DIGIT->INTEGER */
                if (c == '+' || c == '-') {
                    if (got & RSVGN_GOT_SIGN) {
                        error = TRUE; /* Two signs: not allowed */
                    } else {
                        sign = c == '+' ? +1 : -1;
                        got |= RSVGN_GOT_SIGN;
                    }
                } else if (c == '.') {
                    in = RSVGN_IN_FRACTION;
                } else if (c >= '0' && c <= '9') {
                    value = c - '0';
                    in = RSVGN_IN_INTEGER;
                }
                break;
            case RSVGN_IN_INTEGER: /* Previous character(s) was/were digit(s) */
                /* LEGAL: DIGIT .->FRACTION E->PREEXPONENT */
                if (c >= '0' && c <= '9') {
                    value = value * 10 + (c - '0');
                }
                else if (c == '.') {
                    in = RSVGN_IN_FRACTION;
                }
                else if (c == 'e' || c == 'E') {
                    in = RSVGN_IN_PREEXPONENT;
                }
                else {
                    end = TRUE;
                }
                break;
            case RSVGN_IN_FRACTION: /* Previously, digit(s) in the fractional part */
                /* LEGAL: DIGIT E->PREEXPONENT */
                if (c >= '0' && c <= '9') {
                    fraction *= 0.1;
                    value += fraction * (c - '0');
                }
                else if (c == 'e' || c == 'E') {
                    in = RSVGN_IN_PREEXPONENT;
                }
                else {
                    end = TRUE;
                }
                break;
            case RSVGN_IN_PREEXPONENT: /* Right after E */
                /* LEGAL: + - DIGIT->EXPONENT */
                if (c == '+' || c == '-') {
                    if (got & RSVGN_GOT_EXPONENT_SIGN) {
                        error = TRUE; /* Two signs: not allowed */
                    } else {
                        exponent_sign = c == '+' ? +1 : -1;
                        got |= RSVGN_GOT_EXPONENT_SIGN;
                    }
                } else if (c >= '0' && c <= '9') {
                    exponent = c - '0';
                    in = RSVGN_IN_EXPONENT;
                }
                break;
            case RSVGN_IN_EXPONENT: /* After E and the sign, if applicable */
                /* LEGAL: DIGIT */
                if (c >= '0' && c <= '9') {
                    exponent = exponent * 10 + (c - '0');
                } else {
                    end = TRUE;
                }
                break;
        }
        length++;
    }

    /* TODO? if (error) report_the_error_somehow(); */
    rsvg_path_end_of_number(ctx, value, sign, exponent_sign, exponent);
    return end /* && !error */ ? length - 1 : length;
}

static void
rsvg_parse_path_data (RSVGParsePathCtx * ctx, const char *data)
{
    int i = 0;
    char c = 0;

    for (i = 0; data[i] != '\0'; i++) {
        c = data[i];
        if ((c >= '0' && c <= '9') || c == '+' || c == '-' || c == '.') {
            /* digit */
            i += rsvg_parse_number(ctx, data + i) - 1;
        } else if (c == 'z' || c == 'Z') {
            if (ctx->param)
                rsvg_parse_path_do_cmd (ctx, TRUE);
            rsvg_path_builder_close_path (ctx->builder);

            ctx->cp_x = ctx->last_moveto_x;
            ctx->cp_y = ctx->last_moveto_y;
            ctx->rp_x = ctx->cp_x;
            ctx->rp_y = ctx->cp_y;
        } else if (c >= 'A' && c < 'Z' && c != 'E') {
            if (ctx->param)
                rsvg_parse_path_do_cmd (ctx, TRUE);
            ctx->cmd = c + 'a' - 'A';
            ctx->rel = FALSE;
        } else if (c >= 'a' && c < 'z' && c != 'e') {
            if (ctx->param)
                rsvg_parse_path_do_cmd (ctx, TRUE);
            ctx->cmd = c;
            ctx->rel = TRUE;
        }
        /* else c _should_ be whitespace or , */
    }
}

RsvgPathBuilder *
rsvg_path_builder_parse_path (const char *path_str)
{
    RSVGParsePathCtx ctx;

    ctx.builder = rsvg_path_builder_new ();

    ctx.cp_x = 0.0;
    ctx.cp_y = 0.0;
    ctx.rp_x = 0.0;
    ctx.rp_y = 0.0;
    ctx.last_moveto_x = 0.0;
    ctx.last_moveto_y = 0.0;
    ctx.cmd = 0;
    ctx.param = 0;

    rsvg_parse_path_data (&ctx, path_str);

    if (ctx.param)
        rsvg_parse_path_do_cmd (&ctx, TRUE);

    return ctx.builder;
}
