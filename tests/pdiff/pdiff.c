/*
  Metric
  Copyright (C) 2006 Yangli Hector Yee

  This program is free software; you can redistribute it and/or modify it under the terms of the
  GNU General Public License as published by the Free Software Foundation; either version 2 of the License,
  or (at your option) any later version.

  This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY;
  without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
  See the GNU General Public License for more details.

  You should have received a copy of the GNU General Public License along with this program;
  if not, write to the Free Software Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307 USA
*/

#define _GNU_SOURCE

#include "config.h"

#ifdef HAVE_STDINT_H
#include <stdint.h>
#else
#include <glib.h>
typedef gint8 int8_t;
typedef guint8 uint8_t;
typedef gint16 int16_t;
typedef guint16 uint16_t;
typedef gint32 int32_t;
typedef guint32 uint32_t;
#endif

#include "lpyramid.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include "pdiff.h"

#ifndef M_PI
#define M_PI 3.14159265f
#endif

#ifndef __USE_ISOC99
#define expf	exp
#define powf	pow
#define fabsf	fabs
#define sqrtf	sqrt
#define log10f	log10
#endif

/*
 * Given the adaptation luminance, this function returns the
 * threshold of visibility in cd per m^2
 * TVI means Threshold vs Intensity function
 * This version comes from Ward Larson Siggraph 1997
 */
static float
tvi (float adaptation_luminance)
{
    /* returns the threshold luminance given the adaptation luminance
       units are candelas per meter squared
    */
    float log_a, r, result;
    log_a = log10f(adaptation_luminance);

    if (log_a < -3.94f) {
	r = -2.86f;
    } else if (log_a < -1.44f) {
	r = powf(0.405f * log_a + 1.6f , 2.18f) - 2.86f;
    } else if (log_a < -0.0184f) {
	r = log_a - 0.395f;
    } else if (log_a < 1.9f) {
	r = powf(0.249f * log_a + 0.65f, 2.7f) - 0.72f;
    } else {
	r = log_a - 1.255f;
    }

    result = powf(10.0f , r);

    return result;
}

/* computes the contrast sensitivity function (Barten SPIE 1989)
 * given the cycles per degree (cpd) and luminance (lum)
 */
static float
csf (float cpd, float lum)
{
    float a, b, result;

    a = 440.0f * powf((1.0f + 0.7f / lum), -0.2f);
    b = 0.3f * powf((1.0f + 100.0f / lum), 0.15f);

    result = a * cpd * expf(-b * cpd) * sqrtf(1.0f + 0.06f * expf(b * cpd));

    return result;
}

/*
 * Visual Masking Function
 * from Daly 1993
 */
static float
mask (float contrast)
{
    float a, b, result;
    a = powf(392.498f * contrast,  0.7f);
    b = powf(0.0153f * a, 4.0f);
    result = powf(1.0f + b, 0.25f);

    return result;
}

/* convert Adobe RGB (1998) with reference white D65 to XYZ */
static void
AdobeRGBToXYZ (float r, float g, float b, float *x, float *y, float *z)
{
    /* matrix is from http://www.brucelindbloom.com/ */
    *x = r * 0.576700f + g * 0.185556f + b * 0.188212f;
    *y = r * 0.297361f + g * 0.627355f + b * 0.0752847f;
    *z = r * 0.0270328f + g * 0.0706879f + b * 0.991248f;
}

static void
XYZToLAB (float x, float y, float z, float *L, float *A, float *B)
{
    static float xw = -1;
    static float yw;
    static float zw;
    const float epsilon  = 216.0f / 24389.0f;
    const float kappa = 24389.0f / 27.0f;
    float f[3];
    float r[3];
    int i;

    /* reference white */
    if (xw < 0) {
	AdobeRGBToXYZ(1, 1, 1, &xw, &yw, &zw);
    }
    r[0] = x / xw;
    r[1] = y / yw;
    r[2] = z / zw;
    for (i = 0; i < 3; i++) {
	if (r[i] > epsilon) {
	    f[i] = powf(r[i], 1.0f / 3.0f);
	} else {
	    f[i] = (kappa * r[i] + 16.0f) / 116.0f;
	}
    }
    *L = 116.0f * f[1] - 16.0f;
    *A = 500.0f * (f[0] - f[1]);
    *B = 200.0f * (f[1] - f[2]);
}

static uint32_t
_get_pixel (cairo_surface_t *surface, int i)
{
    uint32_t *data;

    data = (uint32_t *) cairo_image_surface_get_data (surface);
    return data[i];
}

static unsigned char
_get_red (cairo_surface_t *surface, int i)
{
    uint32_t pixel;
    uint8_t alpha;

    pixel = _get_pixel (surface, i);

    alpha = (pixel & 0xff000000) >> 24;

    if (alpha == 0)
	return 0;
    else
	return (((pixel & 0x00ff0000) >> 16) * 255 + alpha / 2) / alpha;
}

static unsigned char
_get_green (cairo_surface_t *surface, int i)
{
    uint32_t pixel;
    uint8_t alpha;

    pixel = _get_pixel (surface, i);

    alpha = (pixel & 0xff000000) >> 24;

    if (alpha == 0)
	return 0;
    else
	return (((pixel & 0x0000ff00) >> 8) * 255 + alpha / 2) / alpha;
}

static unsigned char
_get_blue (cairo_surface_t *surface, int i)
{
    uint32_t pixel;
    uint8_t alpha;

    pixel = _get_pixel (surface, i);

    alpha = (pixel & 0xff000000) >> 24;

    if (alpha == 0)
	return 0;
    else
	return (((pixel & 0x000000ff) >> 0) * 255 + alpha / 2) / alpha;
}

static void *
xmalloc (size_t size)
{
    void *buf;

    buf = malloc (size);
    if (buf == NULL) {
	fprintf (stderr, "Out of memory.\n");
	exit (1);
    }

    return buf;
}

int
pdiff_compare (cairo_surface_t *surface_a,
	       cairo_surface_t *surface_b,
	       double gamma,
	       double luminance,
	       double field_of_view)
{
    unsigned int dim = (cairo_image_surface_get_width (surface_a)
			* cairo_image_surface_get_height (surface_a));
    unsigned int i;

    /* assuming colorspaces are in Adobe RGB (1998) convert to XYZ */
    float *aX = xmalloc (dim * sizeof (float));
    float *aY = xmalloc (dim * sizeof (float));
    float *aZ = xmalloc (dim * sizeof (float));
    float *bX = xmalloc (dim * sizeof (float));
    float *bY = xmalloc (dim * sizeof (float));
    float *bZ = xmalloc (dim * sizeof (float));
    float *aLum = xmalloc (dim * sizeof (float));
    float *bLum = xmalloc (dim * sizeof (float));

    float *aA = xmalloc (dim * sizeof (float));
    float *bA = xmalloc (dim * sizeof (float));
    float *aB = xmalloc (dim * sizeof (float));
    float *bB = xmalloc (dim * sizeof (float));

    unsigned int x, y, w, h;

    lpyramid_t *la, *lb;

    float num_one_degree_pixels, pixels_per_degree, num_pixels;
    unsigned int adaptation_level;

    float cpd[MAX_PYR_LEVELS];
    float F_freq[MAX_PYR_LEVELS - 2];
    float csf_max;

    unsigned int pixels_failed;

    w = cairo_image_surface_get_width (surface_a);
    h = cairo_image_surface_get_height (surface_a);
    for (y = 0; y < h; y++) {
	for (x = 0; x < w; x++) {
	    float r, g, b, l;
	    i = x + y * w;
	    r = powf(_get_red (surface_a, i) / 255.0f, gamma);
	    g = powf(_get_green (surface_a, i) / 255.0f, gamma);
	    b = powf(_get_blue (surface_a, i) / 255.0f, gamma);

	    AdobeRGBToXYZ(r,g,b,&aX[i],&aY[i],&aZ[i]);
	    XYZToLAB(aX[i], aY[i], aZ[i], &l, &aA[i], &aB[i]);
	    r = powf(_get_red (surface_b, i) / 255.0f, gamma);
	    g = powf(_get_green (surface_b, i) / 255.0f, gamma);
	    b = powf(_get_blue (surface_b, i) / 255.0f, gamma);

	    AdobeRGBToXYZ(r,g,b,&bX[i],&bY[i],&bZ[i]);
	    XYZToLAB(bX[i], bY[i], bZ[i], &l, &bA[i], &bB[i]);
	    aLum[i] = aY[i] * luminance;
	    bLum[i] = bY[i] * luminance;
	}
    }

    la = lpyramid_create (aLum, w, h);
    lb = lpyramid_create (bLum, w, h);

    num_one_degree_pixels = (float) (2 * tan(field_of_view * 0.5 * M_PI / 180) * 180 / M_PI);
    pixels_per_degree = w / num_one_degree_pixels;

    num_pixels = 1;
    adaptation_level = 0;
    for (i = 0; i < MAX_PYR_LEVELS; i++) {
	adaptation_level = i;
	if (num_pixels > num_one_degree_pixels) break;
	num_pixels *= 2;
    }

    cpd[0] = 0.5f * pixels_per_degree;
    for (i = 1; i < MAX_PYR_LEVELS; i++) cpd[i] = 0.5f * cpd[i - 1];
    csf_max = csf(3.248f, 100.0f);

    for (i = 0; i < MAX_PYR_LEVELS - 2; i++) F_freq[i] = csf_max / csf( cpd[i], 100.0f);

    pixels_failed = 0;
    for (y = 0; y < h; y++) {
	for (x = 0; x < w; x++) {
	    int index = x + y * w;
	    float contrast[MAX_PYR_LEVELS - 2];
	    float F_mask[MAX_PYR_LEVELS - 2];
	    float factor;
	    float delta;
	    float adapt;
	    bool pass;
	    float sum_contrast = 0;
	    for (i = 0; i < MAX_PYR_LEVELS - 2; i++) {
		float n1 = fabsf(lpyramid_get_value (la,x,y,i) - lpyramid_get_value (la,x,y,i + 1));
		float n2 = fabsf(lpyramid_get_value (lb,x,y,i) - lpyramid_get_value (lb,x,y,i + 1));
		float numerator = (n1 > n2) ? n1 : n2;
		float d1 = fabsf(lpyramid_get_value(la,x,y,i+2));
		float d2 = fabsf(lpyramid_get_value(lb,x,y,i+2));
		float denominator = (d1 > d2) ? d1 : d2;
		if (denominator < 1e-5f) denominator = 1e-5f;
		contrast[i] = numerator / denominator;
		sum_contrast += contrast[i];
	    }
	    if (sum_contrast < 1e-5) sum_contrast = 1e-5f;
	    adapt = lpyramid_get_value(la,x,y,adaptation_level) + lpyramid_get_value(lb,x,y,adaptation_level);
	    adapt *= 0.5f;
	    if (adapt < 1e-5) adapt = 1e-5f;
	    for (i = 0; i < MAX_PYR_LEVELS - 2; i++) {
		F_mask[i] = mask(contrast[i] * csf(cpd[i], adapt));
	    }
	    factor = 0;
	    for (i = 0; i < MAX_PYR_LEVELS - 2; i++) {
		factor += contrast[i] * F_freq[i] * F_mask[i] / sum_contrast;
	    }
	    if (factor < 1) factor = 1;
	    if (factor > 10) factor = 10;
	    delta = fabsf(lpyramid_get_value(la,x,y,0) - lpyramid_get_value(lb,x,y,0));
	    pass = true;
	    /* pure luminance test */
	    if (delta > factor * tvi(adapt)) {
		pass = false;
	    } else {
		/* CIE delta E test with modifications */
		float color_scale = 1.0f;
		float da = aA[index] - bA[index];
		float db = aB[index] - bB[index];
		float delta_e;
		/* ramp down the color test in scotopic regions */
		if (adapt < 10.0f) {
		    color_scale = 1.0f - (10.0f - color_scale) / 10.0f;
		    color_scale = color_scale * color_scale;
		}
		da = da * da;
		db = db * db;
		delta_e = (da + db) * color_scale;
		if (delta_e > factor) {
		    pass = false;
		}
	    }
	    if (!pass)
		pixels_failed++;
	}
    }

    free (aX);
    free (aY);
    free (aZ);
    free (bX);
    free (bY);
    free (bZ);
    free (aLum);
    free (bLum);
    lpyramid_destroy (la);
    lpyramid_destroy (lb);
    free (aA);
    free (bA);
    free (aB);
    free (bB);

    return pixels_failed;
}
