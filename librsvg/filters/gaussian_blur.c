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

typedef struct _RsvgFilterPrimitiveGaussianBlur
 RsvgFilterPrimitiveGaussianBlur;

struct _RsvgFilterPrimitiveGaussianBlur {
    RsvgFilterPrimitive super;
    double sdx, sdy;
};

static void
box_blur_line (gint box_width, gint even_offset,
               guchar *src, guchar *dest,
               gint len, gint bpp)
{
    gint  i;
    gint  lead;    /* This marks the leading edge of the kernel              */
    gint  output;  /* This marks the center of the kernel                    */
    gint  trail;   /* This marks the pixel BEHIND the last 1 in the
                      kernel; it's the pixel to remove from the accumulator. */
    gint  *ac;     /* Accumulator for each channel                           */

    g_assert (box_width > 0);

    ac = g_new0 (gint, bpp);

    /* The algorithm differs for even and odd-sized kernels.
     * With the output at the center,
     * If odd, the kernel might look like this: 0011100
     * If even, the kernel will either be centered on the boundary between
     * the output and its left neighbor, or on the boundary between the
     * output and its right neighbor, depending on even_lr.
     * So it might be 0111100 or 0011110, where output is on the center
     * of these arrays.
     */
    lead = 0;

    if (box_width % 2 != 0) {
        /* Odd-width kernel */
        output = lead - (box_width - 1) / 2;
        trail  = lead - box_width;
    } else {
        /* Even-width kernel. */
        if (even_offset == 1) {
            /* Right offset */
            output = lead + 1 - box_width / 2;
            trail  = lead - box_width;
        } else if (even_offset == -1) {
            /* Left offset */
            output = lead - box_width / 2;
            trail  = lead - box_width;
        } else {
            /* If even_offset isn't 1 or -1, there's some error. */
            g_assert_not_reached ();
        }
    }

    /* Initialize accumulator */
    for (i = 0; i < bpp; i++)
        ac[i] = 0;

    /* As the kernel moves across the image, it has a leading edge and a
     * trailing edge, and the output is in the middle. */
    while (output < len) {
        /* The number of pixels that are both in the image and
         * currently covered by the kernel. This is necessary to
         * handle edge cases. */
        guint coverage = (lead < len ? lead : len - 1) - (trail >= 0 ? trail : -1);

#ifdef READABLE_BOXBLUR_CODE
/* The code here does the same as the code below, but the code below
 * has been optimized by moving the if statements out of the tight for
 * loop, and is harder to understand.
 * Don't use both this code and the code below. */
        for (i = 0; i < bpp; i++) {
            /* If the leading edge of the kernel is still on the image,
             * add the value there to the accumulator. */
            if (lead < len)
                ac[i] += src[bpp * lead + i];

            /* If the trailing edge of the kernel is on the image,
             * subtract the value there from the accumulator. */
            if (trail >= 0)
                ac[i] -= src[bpp * trail + i];

            /* Take the averaged value in the accumulator and store
             * that value in the output. The number of pixels currently
             * stored in the accumulator can be less than the nominal
             * width of the kernel because the kernel can go "over the edge"
             * of the image. */
            if (output >= 0)
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
        }
#endif

        /* If the leading edge of the kernel is still on the image... */
        if (lead < len) {
            if (trail >= 0) {
                /* If the trailing edge of the kernel is on the image. (Since
                 * the output is in between the lead and trail, it must be on
                 * the image. */
                for (i = 0; i < bpp; i++) {
                    ac[i] += src[bpp * lead + i];
                    ac[i] -= src[bpp * trail + i];
                    dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
                }
            } else if (output >= 0) {
                /* If the output is on the image, but the trailing edge isn't yet
                 * on the image. */

                for (i = 0; i < bpp; i++) {
                    ac[i] += src[bpp * lead + i];
                    dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
                }
            } else {
                /* If leading edge is on the image, but the output and trailing
                 * edge aren't yet on the image. */
                for (i = 0; i < bpp; i++)
                    ac[i] += src[bpp * lead + i];
            }
        } else if (trail >= 0) {
            /* If the leading edge has gone off the image, but the output and
             * trailing edge are on the image. (The big loop exits when the
             * output goes off the image. */
            for (i = 0; i < bpp; i++) {
                ac[i] -= src[bpp * trail + i];
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
            }
        } else if (output >= 0) {
            /* Leading has gone off the image and trailing isn't yet in it
             * (small image) */
            for (i = 0; i < bpp; i++)
                dest[bpp * output + i] = (ac[i] + (coverage >> 1)) / coverage;
        }

        lead++;
        output++;
        trail++;
    }

    g_free (ac);
}

static gint
compute_box_blur_width (double radius)
{
    double width;

    width = radius * 3 * sqrt (2 * G_PI) / 4;
    return (gint) (width + 0.5);
}

#define SQR(x) ((x) * (x))

static void
make_gaussian_convolution_matrix (gdouble radius, gdouble **out_matrix, gint *out_matrix_len)
{
    gdouble *matrix;
    gdouble std_dev;
    gdouble sum;
    gint matrix_len;
    gint i, j;

    std_dev = radius + 1.0;
    radius = std_dev * 2;

    matrix_len = 2 * ceil (radius - 0.5) + 1;
    if (matrix_len <= 0)
        matrix_len = 1;

    matrix = g_new0 (gdouble, matrix_len);

    /* Fill the matrix by doing numerical integration approximation
     * from -2*std_dev to 2*std_dev, sampling 50 points per pixel.
     * We do the bottom half, mirror it to the top half, then compute the
     * center point.  Otherwise asymmetric quantization errors will occur.
     * The formula to integrate is e^-(x^2/2s^2).
     */

    for (i = matrix_len / 2 + 1; i < matrix_len; i++)
    {
        gdouble base_x = i - (matrix_len / 2) - 0.5;

        sum = 0;
        for (j = 1; j <= 50; j++)
        {
            gdouble r = base_x + 0.02 * j;

            if (r <= radius)
                sum += exp (- SQR (r) / (2 * SQR (std_dev)));
        }

        matrix[i] = sum / 50;
    }

    /* mirror to the bottom half */
    for (i = 0; i <= matrix_len / 2; i++)
        matrix[i] = matrix[matrix_len - 1 - i];

    /* find center val -- calculate an odd number of quanta to make it
     * symmetric, even if the center point is weighted slightly higher
     * than others.
     */
    sum = 0;
    for (j = 0; j <= 50; j++)
        sum += exp (- SQR (- 0.5 + 0.02 * j) / (2 * SQR (std_dev)));

    matrix[matrix_len / 2] = sum / 51;

    /* normalize the distribution by scaling the total sum to one */
    sum = 0;
    for (i = 0; i < matrix_len; i++)
        sum += matrix[i];

    for (i = 0; i < matrix_len; i++)
        matrix[i] = matrix[i] / sum;

    *out_matrix = matrix;
    *out_matrix_len = matrix_len;
}

static void
gaussian_blur_line (gdouble *matrix,
                    gint matrix_len,
                    guchar *src,
                    guchar *dest,
                    gint len,
                    gint bpp)
{
    guchar *src_p;
    guchar *src_p1;
    gint matrix_middle;
    gint row;
    gint i, j;

    matrix_middle = matrix_len / 2;

    /* picture smaller than the matrix? */
    if (matrix_len > len) {
        for (row = 0; row < len; row++) {
            /* find the scale factor */
            gdouble scale = 0;

            for (j = 0; j < len; j++) {
                /* if the index is in bounds, add it to the scale counter */
                if (j + matrix_middle - row >= 0 &&
                    j + matrix_middle - row < matrix_len)
                    scale += matrix[j];
            }

            src_p = src;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < len; j++) {
                    if (j + matrix_middle - row >= 0 &&
                        j + matrix_middle - row < matrix_len)
                        sum += *src_p1 * matrix[j];

                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }
    } else {
        /* left edge */

        for (row = 0; row < matrix_middle; row++) {
            /* find scale factor */
            gdouble scale = 0;

            for (j = matrix_middle - row; j < matrix_len; j++)
                scale += matrix[j];

            src_p = src;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = matrix_middle - row; j < matrix_len; j++) {
                    sum += *src_p1 * matrix[j];
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }

        /* go through each pixel in each col */
        for (; row < len - matrix_middle; row++) {
            src_p = src + (row - matrix_middle) * bpp;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < matrix_len; j++) {
                    sum += matrix[j] * *src_p1;
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum + 0.5);
            }
        }

        /* for the edge condition, we only use available info and scale to one */
        for (; row < len; row++) {
            /* find scale factor */
            gdouble scale = 0;

            for (j = 0; j < len - row + matrix_middle; j++)
                scale += matrix[j];

            src_p = src + (row - matrix_middle) * bpp;

            for (i = 0; i < bpp; i++) {
                gdouble sum = 0;

                src_p1 = src_p++;

                for (j = 0; j < len - row + matrix_middle; j++) {
                    sum += *src_p1 * matrix[j];
                    src_p1 += bpp;
                }

                *dest++ = (guchar) (sum / scale + 0.5);
            }
        }
    }
}

static void
get_column (guchar *column_data,
            guchar *src_data,
            gint src_stride,
            gint bpp,
            gint height,
            gint x)
{
    gint y;
    guchar *src = src_data + (x * bpp);
    for (y = 0; y < height; y++) {
        memcpy (column_data, src, bpp * sizeof (guchar));

        column_data += bpp;
        src += src_stride;
    }
}

static void
put_column (guchar *column_data, guchar *dest_data, gint dest_stride, gint bpp, gint height, gint x)
{
    gint y;
    guchar *dst = dest_data + (x * bpp);
    for (y = 0; y < height; y++) {
        memcpy (dst, column_data, bpp * sizeof(guchar));

        column_data += bpp;
        dst += dest_stride;
    }
}

static void
gaussian_blur_surface (cairo_surface_t *in,
                       cairo_surface_t *out,
                       gdouble sx,
                       gdouble sy)
{
    gint width, height;
    cairo_format_t in_format, out_format;
    gint in_stride;
    gint out_stride;
    guchar *in_data, *out_data;
    gint bpp;
    gboolean out_has_data;

    cairo_surface_flush (in);

    width = cairo_image_surface_get_width (in);
    height = cairo_image_surface_get_height (in);

    g_assert (width == cairo_image_surface_get_width (out)
              && height == cairo_image_surface_get_height (out));

    in_format = cairo_image_surface_get_format (in);
    out_format = cairo_image_surface_get_format (out);
    g_assert (in_format == out_format);
    g_assert (in_format == CAIRO_FORMAT_ARGB32
              || in_format == CAIRO_FORMAT_A8);

    if (in_format == CAIRO_FORMAT_ARGB32)
        bpp = 4;
    else if (in_format == CAIRO_FORMAT_A8)
        bpp = 1;
    else {
        g_assert_not_reached ();
        return;
    }

    in_stride = cairo_image_surface_get_stride (in);
    out_stride = cairo_image_surface_get_stride (out);

    in_data = cairo_image_surface_get_data (in);
    out_data = cairo_image_surface_get_data (out);

    if (sx < 0.0)
        sx = 0.0;

    if (sy < 0.0)
        sy = 0.0;

    /* Bail out by just copying? */
    if ((sx == 0.0 && sy == 0.0)
        || sx > 1000 || sy > 1000) {
        cairo_t *cr;

        cr = cairo_create (out);
        cairo_set_source_surface (cr, in, 0, 0);
        cairo_paint (cr);
        cairo_destroy (cr);
        return;
    }

    if (sx != 0.0) {
        gint box_width;
        gdouble *gaussian_matrix;
        gint gaussian_matrix_len;
        int y;
        guchar *row_buffer = NULL;
        guchar *row1, *row2;
        gboolean use_box_blur;

        /* For small radiuses, use a true gaussian kernel; otherwise use three box blurs with
         * clever offsets.
         */
        if (sx < 10.0)
            use_box_blur = FALSE;
        else
            use_box_blur = TRUE;

        if (use_box_blur) {
            box_width = compute_box_blur_width (sx);

            /* twice the size so we can have "two" scratch rows */
            row_buffer = g_new0 (guchar, width * bpp * 2);
            row1 = row_buffer;
            row2 = row_buffer + width * bpp;
        } else
            make_gaussian_convolution_matrix (sx, &gaussian_matrix, &gaussian_matrix_len);

        for (y = 0; y < height; y++) {
            guchar *in_row, *out_row;

            in_row = in_data + in_stride * y;
            out_row = out_data + out_stride * y;

            if (use_box_blur) {
                if (box_width % 2 != 0) {
                    /* Odd-width box blur: repeat 3 times, centered on output pixel */

                    box_blur_line (box_width, 0, in_row, row1,    width, bpp);
                    box_blur_line (box_width, 0, row1,   row2,    width, bpp);
                    box_blur_line (box_width, 0, row2,   out_row, width, bpp);
                } else {
                    /* Even-width box blur:
                     * This method is suggested by the specification for SVG.
                     * One pass with width n, centered between output and right pixel
                     * One pass with width n, centered between output and left pixel
                     * One pass with width n+1, centered on output pixel
                     */
                    box_blur_line (box_width,     -1, in_row, row1,    width, bpp);
                    box_blur_line (box_width,      1, row1,   row2,    width, bpp);
                    box_blur_line (box_width + 1,  0, row2,   out_row, width, bpp);
                }
            } else
                gaussian_blur_line (gaussian_matrix, gaussian_matrix_len, in_row, out_row, width, bpp);
        }

        if (!use_box_blur)
            g_free (gaussian_matrix);

        g_free (row_buffer);

        out_has_data = TRUE;
    } else
        out_has_data = FALSE;

    if (sy != 0.0) {
        gint box_height;
        gdouble *gaussian_matrix = NULL;
        gint gaussian_matrix_len;
        guchar *col_buffer;
        guchar *col1, *col2;
        int x;
        gboolean use_box_blur;

        /* For small radiuses, use a true gaussian kernel; otherwise use three box blurs with
         * clever offsets.
         */
        if (sy < 10.0)
            use_box_blur = FALSE;
        else
            use_box_blur = TRUE;

        /* twice the size so we can have the source pixels and the blurred pixels */
        col_buffer = g_new0 (guchar, height * bpp * 2);
        col1 = col_buffer;
        col2 = col_buffer + height * bpp;

        if (use_box_blur) {
            box_height = compute_box_blur_width (sy);
        } else
            make_gaussian_convolution_matrix (sy, &gaussian_matrix, &gaussian_matrix_len);

        for (x = 0; x < width; x++) {
            if (out_has_data)
                get_column (col1, out_data, out_stride, bpp, height, x);
            else
                get_column (col1, in_data, in_stride, bpp, height, x);

            if (use_box_blur) {
                if (box_height % 2 != 0) {
                    /* Odd-width box blur */
                    box_blur_line (box_height, 0, col1, col2, height, bpp);
                    box_blur_line (box_height, 0, col2, col1, height, bpp);
                    box_blur_line (box_height, 0, col1, col2, height, bpp);
                } else {
                    /* Even-width box blur */
                    box_blur_line (box_height,     -1, col1, col2, height, bpp);
                    box_blur_line (box_height,      1, col2, col1, height, bpp);
                    box_blur_line (box_height + 1,  0, col1, col2, height, bpp);
                }
            } else
                gaussian_blur_line (gaussian_matrix, gaussian_matrix_len, col1, col2, height, bpp);

            put_column (col2, out_data, out_stride, bpp, height, x);
        }

        g_free (gaussian_matrix);
        g_free (col_buffer);
    }

    cairo_surface_mark_dirty (out);
}

static void
rsvg_filter_primitive_gaussian_blur_render (RsvgNode *node, RsvgComputedValues *values, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    RsvgFilterPrimitiveGaussianBlur *gaussian = (RsvgFilterPrimitiveGaussianBlur *) primitive;

    int width, height;
    cairo_surface_t *output, *in;
    RsvgIRect boundarys;
    gdouble sdx, sdy;
    RsvgFilterPrimitiveOutput op;
    cairo_t *cr;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL) {
        return;
    }

    width = cairo_image_surface_get_width (in);
    height = cairo_image_surface_get_height (in);

    output = _rsvg_image_surface_new (width, height);

    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    /* scale the SD values */
    cairo_matrix_t ctx_paffine = rsvg_filter_context_get_paffine(ctx);
    sdx = fabs (gaussian->sdx * ctx_paffine.xx);
    sdy = fabs (gaussian->sdy * ctx_paffine.yy);

    gaussian_blur_surface (in, output, sdx, sdy);

    /* Hard-clip to the filter area */
    if (!(boundarys.x0 == 0
          && boundarys.y0 == 0
          && boundarys.x1 == width
          && boundarys.y1 == height)) {
        cr = cairo_create (output);
        cairo_set_operator (cr, CAIRO_OPERATOR_CLEAR);
        cairo_set_fill_rule (cr, CAIRO_FILL_RULE_EVEN_ODD);
        cairo_rectangle (cr, 0, 0, width, height);
        cairo_rectangle (cr,
                         boundarys.x0, boundarys.y0,
                         boundarys.x1 - boundarys.x0, boundarys.y1 - boundarys.y0);
        cairo_fill (cr);
        cairo_destroy (cr);
    }

    op.surface = output;
    op.bounds = boundarys;
    rsvg_filter_store_output (primitive->result, op, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_gaussian_blur_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveGaussianBlur *filter = impl;
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

        case RSVG_ATTRIBUTE_STD_DEVIATION:
            if (!rsvg_css_parse_number_optional_number (value, &filter->sdx, &filter->sdy)) {
                rsvg_node_set_attribute_parse_error (node, "stdDeviation", "expected number-optional-number");
                goto out;
            }
            break;

        default:
            break;
        }
    }

out:
    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_gaussian_blur (const char *element_name, RsvgNode *parent, const char *id, const char *klass)
{
    RsvgFilterPrimitiveGaussianBlur *filter;

    filter = g_new0 (RsvgFilterPrimitiveGaussianBlur, 1);
    filter->super.in = g_string_new ("none");
    filter->super.result = g_string_new ("none");
    filter->sdx = 0;
    filter->sdy = 0;
    filter->super.render = rsvg_filter_primitive_gaussian_blur_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_GAUSSIAN_BLUR,
                                parent,
                                id,
                                klass,
                                filter,
                                rsvg_filter_primitive_gaussian_blur_set_atts,
                                rsvg_filter_primitive_free);
}
