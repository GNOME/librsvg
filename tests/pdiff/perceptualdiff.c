/*
  PerceptualDiff - a program that compares two images using a perceptual metric
  based on the paper :
  A perceptual metric for production testing. Journal of graphics tools, 9(4):33-40, 2004, Hector Yee
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

#include "config.h"
#include <stdio.h>

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

#include <string.h>
#include <math.h>
#include "lpyramid.h"
#include "args.h"
#include "pdiff.h"

static bool Yee_Compare(args_t *args)
{
    int width_a, height_a, stride_a;
    unsigned char *data_a, *row_a;
    uint32_t *pixel_a;
    int width_b, height_b, stride_b;
    unsigned char *data_b, *row_b;
    uint32_t *pixel_b;
    int x, y;
    unsigned pixels_failed;
    bool identical = true;

    width_a = cairo_image_surface_get_width (args->surface_a);
    height_a = cairo_image_surface_get_height (args->surface_a);
    stride_a = cairo_image_surface_get_stride (args->surface_a);
    data_a = cairo_image_surface_get_data (args->surface_a);

    width_b = cairo_image_surface_get_width (args->surface_b);
    height_b = cairo_image_surface_get_height (args->surface_b);
    stride_b = cairo_image_surface_get_stride (args->surface_b);
    data_b = cairo_image_surface_get_data (args->surface_b);

    if ((width_a != width_b) || (height_a != height_b)) {
	printf ("FAIL: Image dimensions do not match\n");
	return false;
    }

    identical = true;

    for (y = 0; y < height_a; y++) {
	row_a = data_a + y * stride_a;
	row_b = data_b + y * stride_b;
	pixel_a = (uint32_t *) row_a;
	pixel_b = (uint32_t *) row_b;
	for (x = 0; x < width_a; x++) {
	    if (*pixel_a != *pixel_b) {
		identical = false;
	    }
	    pixel_a++;
	    pixel_b++;
	}
    }
    if (identical) {
	printf ("PASS: Images are binary identical\n");
	return true;
    }

    pixels_failed = pdiff_compare (args->surface_a, args->surface_b,
				   args->Gamma, args->Luminance,
				   args->FieldOfView);

    if (pixels_failed < args->ThresholdPixels) {
	printf ("PASS: Images are perceptually indistinguishable\n");
	return true;
    }

    printf("FAIL: Images are visibly different\n"
	   "%d pixels are different\n", pixels_failed);

    return false;
}

int main(int argc, char **argv)
{
    args_t args;

    args_init (&args);

    if (!args_parse (&args, argc, argv)) {
	return -1;
    } else {
	if (args.Verbose)
	    args_print (&args);
    }
    return ! Yee_Compare(&args);
}
