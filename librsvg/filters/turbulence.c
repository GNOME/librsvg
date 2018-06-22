/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.c: Provides filters

   Copyright (C) 2004 Caleb Moore

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

   Author: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "../rsvg-private.h"
#include "../rsvg-styles.h"
#include "../rsvg-css.h"
#include "common.h"

/* Produces results in the range [1, 2**31 - 2].
   Algorithm is: r = (a * r) mod m
   where a = 16807 and m = 2**31 - 1 = 2147483647
   See [Park & Miller], CACM vol. 31 no. 10 p. 1195, Oct. 1988
   To test: the algorithm should produce the result 1043618065
   as the 10,000th generated number if the original seed is 1.
*/
#define feTurbulence_RAND_m 2147483647  /* 2**31 - 1 */
#define feTurbulence_RAND_a 16807       /* 7**5; primitive root of m */
#define feTurbulence_RAND_q 127773      /* m / a */
#define feTurbulence_RAND_r 2836        /* m % a */
#define feTurbulence_BSize 0x100
#define feTurbulence_BM 0xff
#define feTurbulence_PerlinN 0x1000
#define feTurbulence_NP 12      /* 2^PerlinN */
#define feTurbulence_NM 0xfff

typedef struct _RsvgFilterPrimitiveTurbulence RsvgFilterPrimitiveTurbulence;
struct _RsvgFilterPrimitiveTurbulence {
    RsvgFilterPrimitive super;

    int uLatticeSelector[feTurbulence_BSize + feTurbulence_BSize + 2];
    double fGradient[4][feTurbulence_BSize + feTurbulence_BSize + 2][2];

    int seed;

    double fBaseFreqX;
    double fBaseFreqY;

    int nNumOctaves;
    gboolean bFractalSum;
    gboolean bDoStitching;
};

struct feTurbulence_StitchInfo {
    int nWidth;                 /* How much to subtract to wrap for stitching. */
    int nHeight;
    int nWrapX;                 /* Minimum value to wrap. */
    int nWrapY;
};

static long
feTurbulence_setup_seed (int lSeed)
{
    if (lSeed <= 0)
        lSeed = -(lSeed % (feTurbulence_RAND_m - 1)) + 1;
    if (lSeed > feTurbulence_RAND_m - 1)
        lSeed = feTurbulence_RAND_m - 1;
    return lSeed;
}

static long
feTurbulence_random (int lSeed)
{
    long result;

    result =
        feTurbulence_RAND_a * (lSeed % feTurbulence_RAND_q) -
        feTurbulence_RAND_r * (lSeed / feTurbulence_RAND_q);
    if (result <= 0)
        result += feTurbulence_RAND_m;
    return result;
}

static void
feTurbulence_init (RsvgFilterPrimitiveTurbulence * filter)
{
    double s;
    int i, j, k, lSeed;

    lSeed = feTurbulence_setup_seed (filter->seed);
    for (k = 0; k < 4; k++) {
        for (i = 0; i < feTurbulence_BSize; i++) {
            filter->uLatticeSelector[i] = i;
            for (j = 0; j < 2; j++)
                filter->fGradient[k][i][j] =
                    (double) (((lSeed =
                                feTurbulence_random (lSeed)) % (feTurbulence_BSize +
                                                                feTurbulence_BSize)) -
                              feTurbulence_BSize) / feTurbulence_BSize;
            s = (double) (sqrt
                          (filter->fGradient[k][i][0] * filter->fGradient[k][i][0] +
                           filter->fGradient[k][i][1] * filter->fGradient[k][i][1]));
            filter->fGradient[k][i][0] /= s;
            filter->fGradient[k][i][1] /= s;
        }
    }

    while (--i) {
        k = filter->uLatticeSelector[i];
        filter->uLatticeSelector[i] = filter->uLatticeSelector[j =
                                                               (lSeed =
                                                                feTurbulence_random (lSeed)) %
                                                               feTurbulence_BSize];
        filter->uLatticeSelector[j] = k;
    }

    for (i = 0; i < feTurbulence_BSize + 2; i++) {
        filter->uLatticeSelector[feTurbulence_BSize + i] = filter->uLatticeSelector[i];
        for (k = 0; k < 4; k++)
            for (j = 0; j < 2; j++)
                filter->fGradient[k][feTurbulence_BSize + i][j] = filter->fGradient[k][i][j];
    }
}

#define feTurbulence_s_curve(t) ( t * t * (3. - 2. * t) )
#define feTurbulence_lerp(t, a, b) ( a + t * (b - a) )

static double
feTurbulence_noise2 (RsvgFilterPrimitiveTurbulence * filter,
                     int nColorChannel, double vec[2], struct feTurbulence_StitchInfo *pStitchInfo)
{
    int bx0, bx1, by0, by1, b00, b10, b01, b11;
    double rx0, rx1, ry0, ry1, *q, sx, sy, a, b, t, u, v;
    register int i, j;

    t = vec[0] + feTurbulence_PerlinN;
    bx0 = (int) t;
    bx1 = bx0 + 1;
    rx0 = t - (int) t;
    rx1 = rx0 - 1.0f;
    t = vec[1] + feTurbulence_PerlinN;
    by0 = (int) t;
    by1 = by0 + 1;
    ry0 = t - (int) t;
    ry1 = ry0 - 1.0f;

    /* If stitching, adjust lattice points accordingly. */
    if (pStitchInfo != NULL) {
        if (bx0 >= pStitchInfo->nWrapX)
            bx0 -= pStitchInfo->nWidth;
        if (bx1 >= pStitchInfo->nWrapX)
            bx1 -= pStitchInfo->nWidth;
        if (by0 >= pStitchInfo->nWrapY)
            by0 -= pStitchInfo->nHeight;
        if (by1 >= pStitchInfo->nWrapY)
            by1 -= pStitchInfo->nHeight;
    }

    bx0 &= feTurbulence_BM;
    bx1 &= feTurbulence_BM;
    by0 &= feTurbulence_BM;
    by1 &= feTurbulence_BM;
    i = filter->uLatticeSelector[bx0];
    j = filter->uLatticeSelector[bx1];
    b00 = filter->uLatticeSelector[i + by0];
    b10 = filter->uLatticeSelector[j + by0];
    b01 = filter->uLatticeSelector[i + by1];
    b11 = filter->uLatticeSelector[j + by1];
    sx = (double) (feTurbulence_s_curve (rx0));
    sy = (double) (feTurbulence_s_curve (ry0));
    q = filter->fGradient[nColorChannel][b00];
    u = rx0 * q[0] + ry0 * q[1];
    q = filter->fGradient[nColorChannel][b10];
    v = rx1 * q[0] + ry0 * q[1];
    a = feTurbulence_lerp (sx, u, v);
    q = filter->fGradient[nColorChannel][b01];
    u = rx0 * q[0] + ry1 * q[1];
    q = filter->fGradient[nColorChannel][b11];
    v = rx1 * q[0] + ry1 * q[1];
    b = feTurbulence_lerp (sx, u, v);

    return feTurbulence_lerp (sy, a, b);
}

static double
feTurbulence_turbulence (RsvgFilterPrimitiveTurbulence * filter,
                         int nColorChannel, double *point,
                         double fTileX, double fTileY, double fTileWidth, double fTileHeight)
{
    struct feTurbulence_StitchInfo stitch;
    struct feTurbulence_StitchInfo *pStitchInfo = NULL; /* Not stitching when NULL. */

    double fSum = 0.0f, vec[2], ratio = 1.;
    int nOctave;

    /* Adjust the base frequencies if necessary for stitching. */
    if (filter->bDoStitching) {
        /* When stitching tiled turbulence, the frequencies must be adjusted
           so that the tile borders will be continuous. */
        if (filter->fBaseFreqX != 0.0) {
            double fLoFreq = (double) (floor (fTileWidth * filter->fBaseFreqX)) / fTileWidth;
            double fHiFreq = (double) (ceil (fTileWidth * filter->fBaseFreqX)) / fTileWidth;
            if (filter->fBaseFreqX / fLoFreq < fHiFreq / filter->fBaseFreqX)
                filter->fBaseFreqX = fLoFreq;
            else
                filter->fBaseFreqX = fHiFreq;
        }

        if (filter->fBaseFreqY != 0.0) {
            double fLoFreq = (double) (floor (fTileHeight * filter->fBaseFreqY)) / fTileHeight;
            double fHiFreq = (double) (ceil (fTileHeight * filter->fBaseFreqY)) / fTileHeight;
            if (filter->fBaseFreqY / fLoFreq < fHiFreq / filter->fBaseFreqY)
                filter->fBaseFreqY = fLoFreq;
            else
                filter->fBaseFreqY = fHiFreq;
        }

        /* Set up initial stitch values. */
        pStitchInfo = &stitch;
        stitch.nWidth = (int) (fTileWidth * filter->fBaseFreqX + 0.5f);
        stitch.nWrapX = fTileX * filter->fBaseFreqX + feTurbulence_PerlinN + stitch.nWidth;
        stitch.nHeight = (int) (fTileHeight * filter->fBaseFreqY + 0.5f);
        stitch.nWrapY = fTileY * filter->fBaseFreqY + feTurbulence_PerlinN + stitch.nHeight;
    }

    vec[0] = point[0] * filter->fBaseFreqX;
    vec[1] = point[1] * filter->fBaseFreqY;

    for (nOctave = 0; nOctave < filter->nNumOctaves; nOctave++) {
        if (filter->bFractalSum)
            fSum +=
                (double) (feTurbulence_noise2 (filter, nColorChannel, vec, pStitchInfo) / ratio);
        else
            fSum +=
                (double) (fabs (feTurbulence_noise2 (filter, nColorChannel, vec, pStitchInfo)) /
                          ratio);

        vec[0] *= 2;
        vec[1] *= 2;
        ratio *= 2;

        if (pStitchInfo != NULL) {
            /* Update stitch values. Subtracting PerlinN before the multiplication and
               adding it afterward simplifies to subtracting it once. */
            stitch.nWidth *= 2;
            stitch.nWrapX = 2 * stitch.nWrapX - feTurbulence_PerlinN;
            stitch.nHeight *= 2;
            stitch.nWrapY = 2 * stitch.nWrapY - feTurbulence_PerlinN;
        }
    }

    return fSum;
}

static void
rsvg_filter_primitive_turbulence_render (RsvgNode *node,
                                         RsvgComputedValues *values,
                                         RsvgFilterPrimitive *primitive,
                                         RsvgFilterContext *ctx,
                                         RsvgDrawingCtx *draw_ctx)
{
    RsvgFilterPrimitiveTurbulence *turbulence = (RsvgFilterPrimitiveTurbulence *) primitive;

    gint x, y, tileWidth, tileHeight, rowstride, width, height;
    RsvgIRect boundarys;
    guchar *output_pixels;
    cairo_surface_t *output, *in;
    cairo_matrix_t affine;

    cairo_matrix_t ctx_paffine = rsvg_filter_context_get_paffine(ctx);
    affine = ctx_paffine;
    if (cairo_matrix_invert (&affine) != CAIRO_STATUS_SUCCESS)
      return;

    in = rsvg_filter_get_in (primitive->in, ctx, draw_ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);
    rowstride = cairo_image_surface_get_stride (in);

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx, draw_ctx);

    tileWidth = (boundarys.x1 - boundarys.x0);
    tileHeight = (boundarys.y1 - boundarys.y0);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    const int *ctx_channelmap = rsvg_filter_context_get_channelmap(ctx);

    for (y = 0; y < tileHeight; y++) {
        for (x = 0; x < tileWidth; x++) {
            gint i;
            double point[2];
            guchar *pixel;
            point[0] = affine.xx * (x + boundarys.x0) + affine.xy * (y + boundarys.y0) + affine.x0;
            point[1] = affine.yx * (x + boundarys.x0) + affine.yy * (y + boundarys.y0) + affine.y0;

            pixel = output_pixels + 4 * (x + boundarys.x0) + (y + boundarys.y0) * rowstride;

            for (i = 0; i < 4; i++) {
                double cr;

                cr = feTurbulence_turbulence (turbulence, i, point, (double) x, (double) y,
                                              (double) tileWidth, (double) tileHeight);

                if (turbulence->bFractalSum)
                    cr = ((cr * 255.) + 255.) / 2.;
                else
                    cr = (cr * 255.);

                cr = CLAMP (cr, 0., 255.);

                pixel[ctx_channelmap[i]] = (guchar) cr;
            }
            for (i = 0; i < 3; i++)
                pixel[ctx_channelmap[i]] =
                    pixel[ctx_channelmap[i]] * pixel[ctx_channelmap[3]] / 255;

        }
    }

    cairo_surface_mark_dirty (output);

    RsvgFilterPrimitiveOutput op;
    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output(primitive->result, op, ctx);
    /* rsvg_filter_store_result (primitive->result, output, ctx); */

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_turbulence_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveTurbulence *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_IN:
            g_string_assign (filter->super.in, value);
            break;

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        case RSVG_ATTRIBUTE_BASE_FREQUENCY:
            if (!rsvg_css_parse_number_optional_number (value, &filter->fBaseFreqX, &filter->fBaseFreqY)) {
                rsvg_node_set_attribute_parse_error (node, "baseFrequency", "expected number-optional-number");
                goto out;
            }
            break;

        case RSVG_ATTRIBUTE_NUM_OCTAVES:
            filter->nNumOctaves = atoi (value);
            break;

        case RSVG_ATTRIBUTE_SEED:
            filter->seed = atoi (value);
            break;

        case RSVG_ATTRIBUTE_STITCH_TILES:
            filter->bDoStitching = (!strcmp (value, "stitch"));
            break;

        case RSVG_ATTRIBUTE_TYPE:
            filter->bFractalSum = (!strcmp (value, "fractalNoise"));
            break;

        default:
            break;
        }
    }

out:
    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_turbulence (const char *element_name, RsvgNode *parent, const char *id, const char *klass)
{
    RsvgFilterPrimitiveTurbulence *filter;

    filter = g_new0 (RsvgFilterPrimitiveTurbulence, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->fBaseFreqX = 0;
    filter->fBaseFreqY = 0;
    filter->nNumOctaves = 1;
    filter->seed = 0;
    filter->bDoStitching = 0;
    filter->bFractalSum = 0;

    feTurbulence_init (filter);

    filter->super.render = rsvg_filter_primitive_turbulence_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_TURBULENCE,
                                parent,
                                id,
                                klass,
                                filter,
                                rsvg_filter_primitive_turbulence_set_atts,
                                rsvg_filter_primitive_free);
}
