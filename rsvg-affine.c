/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/* Libart_LGPL - library of basic graphic primitives
 * Copyright (C) 1998 Raph Levien
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Library General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Library General Public License for more details.
 *
 * You should have received a copy of the GNU Library General Public
 * License along with this library; if not, write to the
 * Free Software Foundation, Inc., 59 Temple Place - Suite 330,
 * Boston, MA 02111-1307, USA.
 */

/* Simple manipulations with affine transformations */

#include "config.h"
#include "rsvg-private.h"

#include <math.h>
#include <stdio.h>
#include <string.h>

/**
 * _rsvg_affine_invert: Find the inverse of an affine transformation.
 * @dst: Where the resulting affine is stored.
 * @src: The original affine transformation.
 *
 * All non-degenerate affine transforms are invertible. If the original
 * affine is degenerate or nearly so, expect numerical instability and
 * very likely core dumps on Alpha and other fp-picky architectures.
 * Otherwise, @dst multiplied with @src, or @src multiplied with @dst
 * will be (to within roundoff error) the identity affine.
 **/
void
_rsvg_affine_invert (double dst[6], const double src[6])
{
    double r_det;

    r_det = 1.0 / (src[0] * src[3] - src[1] * src[2]);
    dst[0] = src[3] * r_det;
    dst[1] = -src[1] * r_det;
    dst[2] = -src[2] * r_det;
    dst[3] = src[0] * r_det;
    dst[4] = -src[4] * dst[0] - src[5] * dst[2];
    dst[5] = -src[4] * dst[1] - src[5] * dst[3];
}

/**
 * _rsvg_affine_flip: Flip an affine transformation horizontally and/or vertically.
 * @dst_affine: Where the resulting affine is stored.
 * @src_affine: The original affine transformation.
 * @horiz: Whether or not to flip horizontally.
 * @vert: Whether or not to flip horizontally.
 *
 * Flips the affine transform. FALSE for both @horiz and @vert implements
 * a simple copy operation. TRUE for both @horiz and @vert is a
 * 180 degree rotation. It is ok for @src_affine and @dst_affine to
 * be equal pointers.
 **/
void
_rsvg_affine_flip (double dst_affine[6], const double src_affine[6], int horz, int vert)
{
    dst_affine[0] = horz ? -src_affine[0] : src_affine[0];
    dst_affine[1] = horz ? -src_affine[1] : src_affine[1];
    dst_affine[2] = vert ? -src_affine[2] : src_affine[2];
    dst_affine[3] = vert ? -src_affine[3] : src_affine[3];
    dst_affine[4] = horz ? -src_affine[4] : src_affine[4];
    dst_affine[5] = vert ? -src_affine[5] : src_affine[5];
}

#define EPSILON 1e-6

/**
 * _rsvg_affine_multiply: Multiply two affine transformation matrices.
 * @dst: Where to store the result.
 * @src1: The first affine transform to multiply.
 * @src2: The second affine transform to multiply.
 *
 * Multiplies two affine transforms together, i.e. the resulting @dst
 * is equivalent to doing first @src1 then @src2. Note that the
 * PostScript concat operator multiplies on the left, i.e.  "M concat"
 * is equivalent to "CTM = multiply (M, CTM)";
 *
 * It is safe to call this function with @dst equal to @src1 or @src2.
 **/
void
_rsvg_affine_multiply (double dst[6], const double src1[6], const double src2[6])
{
    double d0, d1, d2, d3, d4, d5;

    d0 = src1[0] * src2[0] + src1[1] * src2[2];
    d1 = src1[0] * src2[1] + src1[1] * src2[3];
    d2 = src1[2] * src2[0] + src1[3] * src2[2];
    d3 = src1[2] * src2[1] + src1[3] * src2[3];
    d4 = src1[4] * src2[0] + src1[5] * src2[2] + src2[4];
    d5 = src1[4] * src2[1] + src1[5] * src2[3] + src2[5];
    dst[0] = d0;
    dst[1] = d1;
    dst[2] = d2;
    dst[3] = d3;
    dst[4] = d4;
    dst[5] = d5;
}

/**
 * _rsvg_affine_identity: Set up the identity matrix.
 * @dst: Where to store the resulting affine transform.
 *
 * Sets up an identity matrix.
 **/
void
_rsvg_affine_identity (double dst[6])
{
    dst[0] = 1;
    dst[1] = 0;
    dst[2] = 0;
    dst[3] = 1;
    dst[4] = 0;
    dst[5] = 0;
}


/**
 * _rsvg_affine_scale: Set up a scaling matrix.
 * @dst: Where to store the resulting affine transform.
 * @sx: X scale factor.
 * @sy: Y scale factor.
 *
 * Sets up a scaling matrix.
 **/
void
_rsvg_affine_scale (double dst[6], double sx, double sy)
{
    dst[0] = sx;
    dst[1] = 0;
    dst[2] = 0;
    dst[3] = sy;
    dst[4] = 0;
    dst[5] = 0;
}

/**
 * _rsvg_affine_rotate: Set up a rotation affine transform.
 * @dst: Where to store the resulting affine transform.
 * @theta: Rotation angle in degrees.
 *
 * Sets up a rotation matrix. In the standard libart coordinate
 * system, in which increasing y moves downward, this is a
 * counterclockwise rotation. In the standard PostScript coordinate
 * system, which is reversed in the y direction, it is a clockwise
 * rotation.
 **/
void
_rsvg_affine_rotate (double dst[6], double theta)
{
    double s, c;

    s = sin (theta * M_PI / 180.0);
    c = cos (theta * M_PI / 180.0);
    dst[0] = c;
    dst[1] = s;
    dst[2] = -s;
    dst[3] = c;
    dst[4] = 0;
    dst[5] = 0;
}

/**
 * _rsvg_affine_shear: Set up a shearing matrix.
 * @dst: Where to store the resulting affine transform.
 * @theta: Shear angle in degrees.
 *
 * Sets up a shearing matrix. In the standard libart coordinate system
 * and a small value for theta, || becomes \\. Horizontal lines remain
 * unchanged.
 **/
void
_rsvg_affine_shear (double dst[6], double theta)
{
    double t;

    t = tan (theta * M_PI / 180.0);
    dst[0] = 1;
    dst[1] = 0;
    dst[2] = t;
    dst[3] = 1;
    dst[4] = 0;
    dst[5] = 0;
}

/**
 * _rsvg_affine_translate: Set up a translation matrix.
 * @dst: Where to store the resulting affine transform.
 * @tx: X translation amount.
 * @tx: Y translation amount.
 *
 * Sets up a translation matrix.
 **/
void
_rsvg_affine_translate (double dst[6], double tx, double ty)
{
    dst[0] = 1;
    dst[1] = 0;
    dst[2] = 0;
    dst[3] = 1;
    dst[4] = tx;
    dst[5] = ty;
}

/**
 * _rsvg_affine_expansion: Find the affine's expansion factor.
 * @src: The affine transformation.
 *
 * Finds the expansion factor, i.e. the square root of the factor
 * by which the affine transform affects area. In an affine transform
 * composed of scaling, rotation, shearing, and translation, returns
 * the amount of scaling.
 *
 * Return value: the expansion factor.
 **/
double
_rsvg_affine_expansion (const double src[6])
{
    return sqrt (fabs (src[0] * src[3] - src[1] * src[2]));
}

/**
 * _rsvg_affine_rectilinear: Determine whether the affine transformation is rectilinear.
 * @src: The original affine transformation.
 *
 * Determines whether @src is rectilinear, i.e.  grid-aligned
 * rectangles are transformed to other grid-aligned rectangles.  The
 * implementation has epsilon-tolerance for roundoff errors.
 *
 * Return value: TRUE if @src is rectilinear.
 **/
int
_rsvg_affine_rectilinear (const double src[6])
{
    return ((fabs (src[1]) < EPSILON && fabs (src[2]) < EPSILON) ||
            (fabs (src[0]) < EPSILON && fabs (src[3]) < EPSILON));
}

/**
 * _rsvg_affine_equal: Determine whether two affine transformations are equal.
 * @matrix1: An affine transformation.
 * @matrix2: Another affine transformation.
 *
 * Determines whether @matrix1 and @matrix2 are equal, with
 * epsilon-tolerance for roundoff errors.
 *
 * Return value: TRUE if @matrix1 and @matrix2 are equal.
 **/
int
_rsvg_affine_equal (double matrix1[6], double matrix2[6])
{
    return (fabs (matrix1[0] - matrix2[0]) < EPSILON &&
            fabs (matrix1[1] - matrix2[1]) < EPSILON &&
            fabs (matrix1[2] - matrix2[2]) < EPSILON &&
            fabs (matrix1[3] - matrix2[3]) < EPSILON &&
            fabs (matrix1[4] - matrix2[4]) < EPSILON && fabs (matrix1[5] - matrix2[5]) < EPSILON);
}
