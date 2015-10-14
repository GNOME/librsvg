/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */

#ifndef TEST_UTILS_H
#define TEST_UTILS_H

#include <gio/gio.h>

G_BEGIN_DECLS 

typedef gboolean (* AddTestFunc) (GFile *file);

const gchar *test_utils_get_test_data_path      (void);

void         test_utils_add_test_for_all_files  (const gchar    *prefix,
                                                 GFile          *base,
                                                 GFile          *file,
                                                 GTestDataFunc   test_func,
                                                 AddTestFunc     add_test_func);
G_END_DECLS

#endif /* TEST_UTILS_H */
