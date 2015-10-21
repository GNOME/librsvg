/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#include "config.h"
#include "test-utils.h"

#include <string.h>

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
      
      g_test_add_data_func_full (test_path, g_object_ref (file), test_func, g_object_unref);
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
