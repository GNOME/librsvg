/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#include "config.h"
#include "test-utils.h"

static gchar *data_path = NULL;
const gchar *
test_utils_get_test_data_path (void)
{
    if (data_path)
        return data_path;

    data_path = g_build_filename (TEST_SRC_DIR,
                                  TOP_SRC_DIR,
                                  "tests",
                                  "fixtures", NULL);

    return data_path;
}

