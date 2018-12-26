/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#include "config.h"
#include "test-utils.h"

#include <string.h>

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

static gchar *data_path = NULL;

const gchar *
test_utils_get_test_data_path (void)
{
    if (data_path)
        return data_path;

    data_path = g_build_filename (g_test_get_dir (G_TEST_DIST),
                                  "fixtures",
                                  NULL);

    return data_path;
}

static int
compare_files (gconstpointer a, gconstpointer b)
{
  GFile *file1 = G_FILE (a);
  GFile *file2 = G_FILE (b);
  char *uri1, *uri2;
  int result;

  uri1 = g_file_get_uri (file1);
  uri2 = g_file_get_uri (file2);

  result = strcmp (uri1, uri2);

  g_free (uri1);
  g_free (uri2);

  return result;
}

void
test_utils_add_test_for_all_files (const gchar   *prefix,
                                   GFile         *base,
                                   GFile         *file,
                                   GTestDataFunc  test_func,
                                   AddTestFunc    add_test_func)
{
  GFileEnumerator *enumerator;
  GFileInfo *info;
  GList *l, *files;
  GError *error = NULL;


  if (g_file_query_file_type (file, 0, NULL) != G_FILE_TYPE_DIRECTORY)
    {
      gchar *test_path;
      gchar *relative_path;

      if (base)
        relative_path = g_file_get_relative_path (base, file);
      else
        relative_path = g_file_get_path (file);

      test_path = g_strconcat (prefix, "/", relative_path, NULL);
      g_free (relative_path);
      
      g_test_add_data_func_full (test_path, g_object_ref (file), test_func, g_object_unref);

      g_free (test_path);
      return;
    }


  enumerator = g_file_enumerate_children (file, G_FILE_ATTRIBUTE_STANDARD_NAME, 0, NULL, &error);
  g_assert_no_error (error);
  files = NULL;

  while ((info = g_file_enumerator_next_file (enumerator, NULL, &error)))
    {
      GFile *next_file = g_file_get_child (file, g_file_info_get_name (info));

      if (add_test_func == NULL || add_test_func (next_file))
        {
          files = g_list_prepend (files, g_object_ref (next_file));
        }

      g_object_unref (next_file);
      g_object_unref (info);
    }
  
  g_assert_no_error (error);
  g_object_unref (enumerator);

  files = g_list_sort (files, compare_files);

  for (l = files; l; l = l->next)
    {
      test_utils_add_test_for_all_files (prefix, base, l->data, test_func, add_test_func);
    }

  g_list_free_full (files, g_object_unref);
}
