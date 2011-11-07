/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*

   Copyright (C) 2002 Ximian, Inc.
   Copyright (C) 2004 Dom Lachowicz
  
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
  
   Author: Michael Meeks <michael@ximian.com>
   Author: Dom Lachowicz <cinamod@hotmail.com>
*/

#include <config.h>
#include <glib.h>
#include <stdio.h>
#include <stdlib.h>

#include "rsvg.h"
#include "rsvg-cairo.h"
#include "rsvg-private.h"
#include "rsvg-tools-main.h"

static gboolean
read_contents (const gchar *file_name, guint8 **contents, gsize *length)
{
    GFile *file;
    GFileInputStream *input_stream;
    gboolean success = FALSE;

    file = g_file_new_for_commandline_arg (file_name);
    input_stream = g_file_read (file, NULL, NULL);
    if (input_stream) {
        GFileInfo *file_info;

        file_info = g_file_input_stream_query_info (input_stream,
                                                    G_FILE_ATTRIBUTE_STANDARD_SIZE,
                                                    NULL, NULL);
        if (file_info) {
            gsize bytes_read;

            *length = g_file_info_get_size (file_info);
            *contents = (guint8 *) g_new (guint8*, *length);
            success = g_input_stream_read_all (G_INPUT_STREAM(input_stream),
                                               *contents,
                                               *length,
                                               &bytes_read,
                                               NULL,
                                               NULL);
            g_object_unref (file_info);
        }
        g_object_unref (input_stream);
    }

    g_object_unref (file);

    return success;
}

int
rsvg_tools_main (int *argc, char ***argv)
{
    int i, j, count = 10;
    GTimer *timer;

    GOptionContext *g_option_context;
    double x_zoom = 1.0;
    double y_zoom = 1.0;
    double dpi = -1.0;
    int width = -1;
    int height = -1;
    int bVersion = 0;

    char **args;
    gint n_args = 0;
    cairo_surface_t *image;
    cairo_t *cr;
    guint8 *contents = NULL;
    gsize length;
    RsvgHandle *handle;
    RsvgDimensionData dimensions;

    GOptionEntry options_table[] = {
        {"dpi", 'd', 0, G_OPTION_ARG_DOUBLE, &dpi, "pixels per inch", "<float>"},
        {"x-zoom", 'x', 0, G_OPTION_ARG_DOUBLE, &x_zoom, "x zoom factor", "<float>"},
        {"y-zoom", 'y', 0, G_OPTION_ARG_DOUBLE, &y_zoom, "y zoom factor", "<float>"},
        {"width", 'w', 0, G_OPTION_ARG_INT, &width, "width", "<int>"},
        {"height", 'h', 0, G_OPTION_ARG_INT, &height, "height", "<int>"},
        {"count", 'c', 0, G_OPTION_ARG_INT, &count, "number of times to render the SVG", "<int>"},
        {"version", 'v', 0, G_OPTION_ARG_NONE, &bVersion, "show version information", NULL},
        {G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, N_("[FILE...]")},
        {NULL}
    };

    g_option_context = g_option_context_new (_("- SVG Performance Test"));
    g_option_context_add_main_entries (g_option_context, options_table, NULL);
    g_option_context_set_help_enabled (g_option_context, TRUE);
    if (!g_option_context_parse (g_option_context, argc, argv, NULL)) {
        exit (EXIT_FAILURE);
    }

    g_option_context_free (g_option_context);

    if (bVersion != 0) {
        g_print ("test-performance version %s\n", VERSION);
        exit (EXIT_SUCCESS);
    }

    if (args)
        while (args[n_args] != NULL)
            n_args++;

    if (n_args < 1) {
        g_print (_("Must specify a SVG file\n"));
        exit (EXIT_FAILURE);
    }

    g_type_init ();

    for (j = 0; j < n_args; j++) {
        if (!read_contents (args[j], &contents, &length))
            continue;

        handle = rsvg_handle_new_from_data (contents, length, NULL);
        if (!handle) {
            g_free (contents);
            continue;
        }

        rsvg_handle_get_dimensions (handle, &dimensions);
        /* if both are unspecified, assume user wants to zoom the pixbuf in at least 1 dimension */
        if (width == -1 && height == -1) {
            width = dimensions.width * x_zoom;
            height = dimensions.height * y_zoom;
        } else if (x_zoom == 1.0 && y_zoom == 1.0) {
            /* if both are unspecified, assume user wants to resize pixbuf in at least 1 dimension */
        } else { /* assume the user wants to zoom the pixbuf, but cap the maximum size */
            if (dimensions.width * x_zoom < width)
                width = dimensions.width * x_zoom;
            if (dimensions.height * y_zoom < height)
                height = dimensions.height * y_zoom;
        }
        g_object_unref (handle);

        image = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
        cr = cairo_create (image);
        cairo_surface_destroy (image);

        timer = g_timer_new ();
        g_timer_start (timer);

        for (i = 0; i < count; i++) {
            handle = rsvg_handle_new_from_data (contents, length, NULL);
            cairo_save (cr);
            cairo_scale (cr, (double) width / dimensions.width, (double) height / dimensions.height);
            rsvg_handle_render_cairo (handle, cr);
            cairo_restore (cr);
            g_object_unref (handle);
        }

        g_print ("%-50s\t\t%g(s)\n", args[j], g_timer_elapsed (timer, NULL) / count);
        g_timer_destroy (timer);

        g_free (contents);
        cairo_destroy (cr);
    }

    rsvg_cleanup ();

    return 0;
}
