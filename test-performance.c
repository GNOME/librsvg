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
#include <popt.h>

#include "rsvg.h"

int
main (int argc, const char **argv)
{
	int         i, count = 10;
	GTimer     *timer;

	poptContext popt_context;
	double x_zoom = 1.0;
	double y_zoom = 1.0;
	double dpi = -1.0;
	int width  = -1;
	int height = -1;
	int bVersion = 0;

	struct poptOption options_table[] = {
		{ "dpi"   ,  'd',  POPT_ARG_DOUBLE, NULL, 0, "pixels per inch", "<float>"},
		{ "x-zoom",  'x',  POPT_ARG_DOUBLE, NULL, 0, "x zoom factor", "<float>" },
		{ "y-zoom",  'y',  POPT_ARG_DOUBLE, NULL, 0, "y zoom factor", "<float>" },
		{ "width",   'w',  POPT_ARG_INT,    NULL, 0, "width", "<int>" },
		{ "height",  'h',  POPT_ARG_INT,    NULL, 0, "height", "<int>" },
		{ "count",   'c',  POPT_ARG_INT,    NULL, 0, "number of times to render the SVG", "<int>" },
		{ "version", 'v',  POPT_ARG_NONE,   NULL, 0, "show version information", NULL },
		POPT_AUTOHELP
		POPT_TABLEEND
	};
	int c;
	const char * const *args;
	gint n_args = 0;
	GdkPixbuf *pixbuf;

	options_table[0].arg = &dpi;
	options_table[1].arg = &x_zoom;
	options_table[2].arg = &y_zoom;
	options_table[3].arg = &width;
	options_table[4].arg = &height;
	options_table[5].arg = &count;
	options_table[6].arg = &bVersion;

	popt_context = poptGetContext ("test-performance", argc, argv, options_table, 0);
	poptSetOtherOptionHelp(popt_context, "[OPTIONS...] file.svg");

	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);

	if (bVersion != 0)
		{
			g_print ("test-performance version %s\n", VERSION);
			return 0;
		}

	if (args)
		while (args[n_args] != NULL)
			n_args++;

	if (n_args != 1)
		{
			poptPrintHelp (popt_context, stderr, 0);
			poptFreeContext (popt_context);
			return 1;
		}

	g_type_init ();

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

	fprintf (stdout, "Rendering took %g(s)\n",
		 (double)g_timer_elapsed (timer, NULL) / count);

	return 0;
}
