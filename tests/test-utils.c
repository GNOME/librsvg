/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#include "config.h"
#include "test-utils.h"

static gchar *data_path = NULL;
const gchar *
test_utils_get_test_data_path (void)
{
    gchar *prgname;
    gchar *dirname;

    if (data_path)
        return data_path;

    prgname = g_get_prgname ();
    dirname = g_path_get_dirname (prgname);

#ifdef LT_OBJDIR
    {
        gchar *another_dirname;
        another_dirname = g_path_get_dirname (dirname);
        g_free (dirname);
        dirname = another_dirname;
    }
#endif
    data_path = g_build_filename (dirname, "fixtures", NULL);
    g_free (dirname);

    return data_path;
}

