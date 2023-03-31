/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#include "config.h"
#include "test-utils.h"

#include <string.h>
#include <pango/pango.h>
#include <pango/pangocairo.h>

#if !PANGO_VERSION_CHECK (1, 44, 0)
# include <hb.h>
#endif

#include <ft2build.h>
#include FT_FREETYPE_H


/* Compare two buffers, returning the number of pixels that are
 * different and the maximum difference of any single color channel in
 * result_ret.
 *
 * This function should be rewritten to compare all formats supported by
 * cairo_format_t instead of taking a mask as a parameter.
 */
static void
buffer_diff_core (unsigned char             *_buf_a,
                  unsigned char             *_buf_b,
                  unsigned char             *_buf_diff,
                  int                        width,
                  int                        height,
                  int                        stride,
                  guint32                    mask,
                  TestUtilsBufferDiffResult *result_ret)
{
    int x, y;
    guint32 *row_a, *row_b, *row;
    TestUtilsBufferDiffResult result = {0, 0};
    guint32 *buf_a = (guint32 *) _buf_a;
    guint32 *buf_b = (guint32 *) _buf_b;
    guint32 *buf_diff = (guint32 *) _buf_diff;

    stride /= sizeof(guint32);
    for (y = 0; y < height; y++)
    {
        row_a = buf_a + y * stride;
        row_b = buf_b + y * stride;
        row = buf_diff + y * stride;
        for (x = 0; x < width; x++)
        {
            /* check if the pixels are the same */
            if ((row_a[x] & mask) != (row_b[x] & mask)) {
                int channel;
                guint32 diff_pixel = 0;

                /* calculate a difference value for all 4 channels */
                for (channel = 0; channel < 4; channel++) {
                    int value_a = (row_a[x] >> (channel*8)) & 0xff;
                    int value_b = (row_b[x] >> (channel*8)) & 0xff;
                    unsigned int diff;
                    diff = abs (value_a - value_b);
                    if (diff > result.max_diff)
                        result.max_diff = diff;
                    diff *= 4;  /* emphasize */
                    if (diff)
                        diff += 128; /* make sure it's visible */
                    if (diff > 255)
                        diff = 255;
                    diff_pixel |= diff << (channel*8);
                }

                result.pixels_changed++;
                if ((diff_pixel & 0x00ffffff) == 0) {
                    /* alpha only difference, convert to luminance */
                    guint8 alpha = diff_pixel >> 24;
                    diff_pixel = alpha * 0x010101;
                }
                row[x] = diff_pixel;
            } else {
                row[x] = 0;
            }
            row[x] |= 0xff000000; /* Set ALPHA to 100% (opaque) */
        }
    }

    *result_ret = result;
}

void
test_utils_compare_surfaces (cairo_surface_t           *surface_a,
                             cairo_surface_t           *surface_b,
                             cairo_surface_t           *surface_diff,
                             TestUtilsBufferDiffResult *result)
{
    /* Here, we run cairo's old buffer_diff algorithm which looks for
     * pixel-perfect images.
     */
    buffer_diff_core (cairo_image_surface_get_data (surface_a),
                      cairo_image_surface_get_data (surface_b),
                      cairo_image_surface_get_data (surface_diff),
                      cairo_image_surface_get_width (surface_a),
                      cairo_image_surface_get_height (surface_a),
                      cairo_image_surface_get_stride (surface_a),
                      0xffffffff,
                      result);
    if (result->pixels_changed == 0)
        return;

    g_test_message ("%d pixels differ (with maximum difference of %d) from reference image\n",
                    result->pixels_changed, result->max_diff);
}

/* Copied from gdk_cairo_surface_paint_pixbuf in gdkcairo.c,
 * we do not want to depend on GDK
 */
static void
test_utils_cairo_surface_paint_pixbuf (cairo_surface_t *surface,
                                       const GdkPixbuf *pixbuf)
{
    gint width, height;
    guchar *gdk_pixels, *cairo_pixels;
    int gdk_rowstride, cairo_stride;
    int n_channels;
    int j;

    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS)
        return;

    /* This function can't just copy any pixbuf to any surface, be
     * sure to read the invariants here before calling it */

    g_assert (cairo_surface_get_type (surface) == CAIRO_SURFACE_TYPE_IMAGE);
    g_assert (cairo_image_surface_get_format (surface) == CAIRO_FORMAT_RGB24 ||
              cairo_image_surface_get_format (surface) == CAIRO_FORMAT_ARGB32);
    g_assert (cairo_image_surface_get_width (surface) == gdk_pixbuf_get_width (pixbuf));
    g_assert (cairo_image_surface_get_height (surface) == gdk_pixbuf_get_height (pixbuf));

    cairo_surface_flush (surface);

    width = gdk_pixbuf_get_width (pixbuf);
    height = gdk_pixbuf_get_height (pixbuf);
    gdk_pixels = gdk_pixbuf_get_pixels (pixbuf);
    gdk_rowstride = gdk_pixbuf_get_rowstride (pixbuf);
    n_channels = gdk_pixbuf_get_n_channels (pixbuf);
    cairo_stride = cairo_image_surface_get_stride (surface);
    cairo_pixels = cairo_image_surface_get_data (surface);

    for (j = height; j; j--)
    {
        guchar *p = gdk_pixels;
        guchar *q = cairo_pixels;

        if (n_channels == 3)
        {
            guchar *end = p + 3 * width;

            while (p < end)
            {
#if G_BYTE_ORDER == G_LITTLE_ENDIAN
                q[0] = p[2];
                q[1] = p[1];
                q[2] = p[0];
#else
                q[1] = p[0];
                q[2] = p[1];
                q[3] = p[2];
#endif
                p += 3;
                q += 4;
            }
        }
        else
        {
            guchar *end = p + 4 * width;
            guint t1,t2,t3;

#define MULT(d,c,a,t) G_STMT_START { t = c * a + 0x80; d = ((t >> 8) + t) >> 8; } G_STMT_END

            while (p < end)
            {
#if G_BYTE_ORDER == G_LITTLE_ENDIAN
                MULT(q[0], p[2], p[3], t1);
                MULT(q[1], p[1], p[3], t2);
                MULT(q[2], p[0], p[3], t3);
                q[3] = p[3];
#else
                q[0] = p[3];
                MULT(q[1], p[0], p[3], t1);
                MULT(q[2], p[1], p[3], t2);
                MULT(q[3], p[2], p[3], t3);
#endif

                p += 4;
                q += 4;
            }

#undef MULT
        }

        gdk_pixels += gdk_rowstride;
        cairo_pixels += cairo_stride;
    }

    cairo_surface_mark_dirty (surface);
}

cairo_surface_t *
test_utils_cairo_surface_from_pixbuf (const GdkPixbuf *pixbuf)
{
    cairo_surface_t *surface;

    g_return_val_if_fail (GDK_IS_PIXBUF (pixbuf), NULL);
    g_return_val_if_fail (gdk_pixbuf_get_n_channels (pixbuf) == 4, NULL);

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
                                          gdk_pixbuf_get_width (pixbuf),
                                          gdk_pixbuf_get_height (pixbuf));

    test_utils_cairo_surface_paint_pixbuf (surface, pixbuf);

    return surface;
}

static gchar *data_path = NULL;

const gchar *
test_utils_get_test_data_path (void)
{
    if (data_path)
        return data_path;

    data_path = g_test_build_filename (G_TEST_DIST, "fixtures", NULL);

    return data_path;
}

void
test_utils_print_dependency_versions (void)
{
    FT_Library ft_lib;
    FT_Int ft_major = 0;
    FT_Int ft_minor = 0;
    FT_Int ft_patch = 0;

    FT_Init_FreeType (&ft_lib);
    FT_Library_Version (ft_lib, &ft_major, &ft_minor, &ft_patch);
    FT_Done_FreeType (ft_lib);

    g_test_message ("Cairo version:    %s", cairo_version_string ());
    g_test_message ("Pango version:    %s", pango_version_string ());
    g_test_message ("Freetype version: %d.%d.%d", ft_major, ft_minor, ft_patch);
#if PANGO_VERSION_CHECK (1, 44, 0)
    g_test_message ("Harfbuzz version: %s", hb_version_string ());
#else
    g_test_message ("Not printing Harfbuzz version since Pango is older than 1.44");
#endif
}
