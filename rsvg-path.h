/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */

#ifndef RSVG_PATH_H
#define RSVG_PATH_H

#include "rsvg-bpath-util.h"

G_BEGIN_DECLS

RsvgBpathDef *
rsvg_parse_path (const char *path_str);

G_END_DECLS

#endif /* RSVG_PATH_H */
