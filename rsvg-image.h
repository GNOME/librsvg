/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg-image.h: Image loading and displaying

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#ifndef RSVG_IMAGE_H
#define RSVG_IMAGE_H

#include "rsvg-structure.h"

G_BEGIN_DECLS 

RsvgNode *rsvg_new_image (void);

typedef struct _RsvgNodeImage RsvgNodeImage;

struct _RsvgNodeImage {
    RsvgNode super;
    gint preserve_aspect_ratio;
    RsvgLength x, y, w, h;
    GdkPixbuf *img;
};

void rsvg_preserve_aspect_ratio (unsigned int aspect_ratio, double width,
                                 double height, double *w, double *h, double *x, double *y);


gchar *rsvg_get_file_path (const gchar * filename, const gchar * basedir);

GdkPixbuf *rsvg_pixbuf_new_from_href (const char *href, const char *base_uri, GError ** error);

G_END_DECLS

#endif                          /* RSVG_IMAGE_H */
