/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */

#ifndef RSVG_BPATH_UTIL_H
#define RSVG_BPATH_UTIL_H

#include <glib/gtypes.h>
#include <libart_lgpl/art_bpath.h>

G_BEGIN_DECLS

typedef struct _RsvgBpathDef RsvgBpathDef;

struct _RsvgBpathDef {
	ArtBpath *bpath;
	int n_bpath;
	int n_bpath_max;
	int moveto_idx;
};

RsvgBpathDef *rsvg_bpath_def_new (void);
RsvgBpathDef *rsvg_bpath_def_new_from (ArtBpath *bpath);

void rsvg_bpath_def_free       (RsvgBpathDef *bpd);

void rsvg_bpath_def_moveto     (RsvgBpathDef *bpd,
								double x, double y);
void rsvg_bpath_def_lineto     (RsvgBpathDef *bpd,
								double x, double y);
void rsvg_bpath_def_curveto    (RsvgBpathDef *bpd,
								double x1, double y1,
								double x2, double y2,
								double x3, double y3);
void rsvg_bpath_def_closepath  (RsvgBpathDef *bpd);

void rsvg_bpath_def_art_finish (RsvgBpathDef *bpd);

G_END_DECLS

#endif

