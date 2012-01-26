/*
   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

   Author: Raph Levien <raph@artofcode.com>
*/

#ifndef RSVG_SIZE_CALLBACK_H
#define RSVG_SIZE_CALLBACK_H

#include <glib.h>

G_BEGIN_DECLS

typedef enum {
    RSVG_SIZE_ZOOM,
    RSVG_SIZE_WH,
    RSVG_SIZE_WH_MAX,
    RSVG_SIZE_ZOOM_MAX
} RsvgSizeType;

struct RsvgSizeCallbackData {
    RsvgSizeType type;
    double x_zoom;
    double y_zoom;
    gint width;
    gint height;

    gboolean keep_aspect_ratio;
};

G_GNUC_INTERNAL
void _rsvg_size_callback (int *width, int *height, gpointer data);

G_END_DECLS

#endif /* RSVG_SIZE_CALLBACK_H */
