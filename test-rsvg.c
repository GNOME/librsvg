/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 4 -*-

   test-rsvg.c: Command line utility for exercising rsvg.
 
   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz
  
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
#include "rsvg.h"

#include <popt.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int
main (int argc, const char **argv)
{
	poptContext popt_context;
	double x_zoom = 1.0;
	double y_zoom = 1.0;
	gint width  = -1;
	gint height = -1;
	char * format = "png";

	struct poptOption options_table[] = {
		{ "x-zoom", 'x', POPT_ARG_DOUBLE, &x_zoom, 0, NULL, "x zoom factor" },
		{ "y-zoom", 'y', POPT_ARG_DOUBLE, &y_zoom, 0, NULL, "y zoom factor" },
		{ "width",  'w', POPT_ARG_INT,    &width,  0, NULL, "width" },
		{ "height", 'h', POPT_ARG_INT,    &height, 0, NULL, "height" },
		{ "format", 'f', POPT_ARG_STRING, &format, 0, NULL, "save format [png, jpeg]" },
		POPT_AUTOHELP
		{ NULL, 0, 0, NULL, 0, NULL, NULL }
	};
	int c;
	const char * const *args;
	gint n_args = 0;
	GdkPixbuf *pixbuf;

	g_type_init ();

	popt_context = poptGetContext ("test-rsvg", argc, argv, options_table, 0);

	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);

	if (args)
		{
			while(args[n_args] != NULL)
				n_args++;
		}

	if (n_args != 2)
		{
			poptPrintHelp (popt_context, stderr, 0);
			return 1;
		}

	if (strstr (format, "jpeg") != NULL)
		format = "jpeg";
	else
		format = "png";

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

	if (pixbuf) 
		gdk_pixbuf_save (pixbuf, args[1], format, NULL, NULL);
	else {
		fprintf (stderr, "Error loading SVG file.\n");
		return 1;
	}
	return 0;
}
