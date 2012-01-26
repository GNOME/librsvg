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

#include "config.h"

#include "rsvg-size-callback.h"

#include <math.h>

void
_rsvg_size_callback (int *width, int *height, gpointer data)
{
    struct RsvgSizeCallbackData *real_data = (struct RsvgSizeCallbackData *) data;
    double zoomx, zoomy, zoom;

    int in_width, in_height;

    in_width = *width;
    in_height = *height;

    switch (real_data->type) {
    case RSVG_SIZE_ZOOM:
        if (*width < 0 || *height < 0)
            return;

        *width = floor (real_data->x_zoom * *width + 0.5);
        *height = floor (real_data->y_zoom * *height + 0.5);
        break;

    case RSVG_SIZE_ZOOM_MAX:
        if (*width < 0 || *height < 0)
            return;

        *width = floor (real_data->x_zoom * *width + 0.5);
        *height = floor (real_data->y_zoom * *height + 0.5);

        if (*width > real_data->width || *height > real_data->height) {
            zoomx = (double) real_data->width / *width;
            zoomy = (double) real_data->height / *height;
            zoom = MIN (zoomx, zoomy);

            *width = floor (zoom * *width + 0.5);
            *height = floor (zoom * *height + 0.5);
        }
        break;

    case RSVG_SIZE_WH_MAX:
        if (*width < 0 || *height < 0)
            return;

        zoomx = (double) real_data->width / *width;
        zoomy = (double) real_data->height / *height;
        if (zoomx < 0)
            zoom = zoomy;
        else if (zoomy < 0)
            zoom = zoomx;
        else
            zoom = MIN (zoomx, zoomy);

        *width = floor (zoom * *width + 0.5);
        *height = floor (zoom * *height + 0.5);
        break;

    case RSVG_SIZE_WH:
        if (real_data->width != -1)
            *width = real_data->width;
        if (real_data->height != -1)
            *height = real_data->height;
        break;

    default:
        g_assert_not_reached ();
    }

    if (real_data->keep_aspect_ratio) {
        int out_min = MIN (*width, *height);

        if (out_min == *width) {
            *height = in_height * ((double) *width / (double) in_width);
        } else {
            *width = in_width * ((double) *height / (double) in_height);
        }
    }
}
