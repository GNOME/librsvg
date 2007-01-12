/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 8; tab-width: 8 -*-

   test-performance.c: performance tests.
 
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
#include "rsvg-private.h"

int
main (int argc, char **argv)
{
    int i, count = 10;
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
    GdkPixbuf *pixbuf;

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
    if (!g_option_context_parse (g_option_context, &argc, &argv, NULL)) {
        exit (1);
    }

    g_option_context_free (g_option_context);

    if (bVersion != 0) {
        g_print ("test-performance version %s\n", VERSION);
        return 0;
    }

    if (args)
        while (args[n_args] != NULL)
            n_args++;

    if (n_args != 1) {
        g_print (_("Must specify a SVG file\n"));
        return 1;
    }

    rsvg_init ();

    fprintf (stdout, "File '%s'\n", args[0]);

    timer = g_timer_new ();
    g_timer_start (timer);

    for (i = 0; i < count; i++) {
        /* if both are unspecified, assume user wants to zoom the pixbuf in at least 1 dimension */
        if (width == -1 && height == -1)
            pixbuf = rsvg_pixbuf_from_file_at_zoom (args[0], x_zoom, y_zoom, NULL);
        /* if both are unspecified, assume user wants to resize pixbuf in at least 1 dimension */
        else if (x_zoom == 1.0 && y_zoom == 1.0)
            pixbuf = rsvg_pixbuf_from_file_at_size (args[0], width, height, NULL);
        else
            /* assume the user wants to zoom the pixbuf, but cap the maximum size */
            pixbuf = rsvg_pixbuf_from_file_at_zoom_with_max (args[0], x_zoom, y_zoom,
                                                             width, height, NULL);

        g_object_unref (pixbuf);
    }

    fprintf (stdout, "Rendering took %g(s)\n", g_timer_elapsed (timer, NULL) / count);
    g_timer_destroy (timer);

    rsvg_term ();

    return 0;
}
