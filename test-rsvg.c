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
#include "rsvg-private.h"

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
	double dpi_x = -1.0;
	double dpi_y = -1.0;
	int width  = -1;
	int height = -1;
	int bVersion = 0;
	int quality = 100;
	char * quality_str = NULL;
	char * format = NULL;

	struct poptOption options_table[] = {
		{ "dpi-x",   'd',  POPT_ARG_DOUBLE, &dpi_x,    0, "pixels per inch", "<float>"},
		{ "dpi-y",   'p',  POPT_ARG_DOUBLE, &dpi_y,    0, "pixels per inch", "<float>"},
		{ "x-zoom",  'x',  POPT_ARG_DOUBLE, &x_zoom,   0, "x zoom factor", "<float>" },
		{ "y-zoom",  'y',  POPT_ARG_DOUBLE, &y_zoom,   0, "y zoom factor", "<float>" },
		{ "width",   'w',  POPT_ARG_INT,    &width,    0, "width", "<int>" },
		{ "height",  'h',  POPT_ARG_INT,    &height,   0, "height", "<int>" },
		{ "quality", 'q',  POPT_ARG_INT,    &quality,  0, "JPEG quality", "<int>"},
		{ "format",  'f',  POPT_ARG_STRING, &format,   0, "save format", "[png, jpeg]"},
		{ "version", 'v',  POPT_ARG_NONE,   &bVersion, 0, "show version information", NULL },
		POPT_AUTOHELP
		POPT_TABLEEND
	};
	int c;
	const char * const *args;
	gint n_args = 0;
	GdkPixbuf *pixbuf;

	popt_context = poptGetContext ("rsvg", argc, argv, options_table, 0);
	poptSetOtherOptionHelp(popt_context, "[OPTIONS...] file.svg file.png");

	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);

	if (bVersion != 0)
		{
		    g_print ("rsvg version %s\n", VERSION);
			return 0;
		}

	if (args)
		while (args[n_args] != NULL)
			n_args++;

	if (n_args != 2)
		{
			poptPrintHelp (popt_context, stderr, 0);
			poptFreeContext (popt_context);
			return 1;
		}

	if(format == NULL)
		format = "png";
	else if (strstr (format, "jpg") != NULL) /* backward compatibility */
		format = "jpeg";

	rsvg_init ();

	rsvg_set_default_dpi_x_y (dpi_x, dpi_y);

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
		if (strcmp (format, "jpeg") == 0) {
			if (quality < 1 || quality > 100) /* is an invalid quality */
				gdk_pixbuf_save (pixbuf, args[1], format, NULL, NULL);
			else {
				quality_str = g_strdup_printf ("%d", quality);
				gdk_pixbuf_save (pixbuf, args[1], format, NULL, "quality", quality_str, NULL);
				g_free (quality_str);
			}
		}
		else {
			gdk_pixbuf_save (pixbuf, args[1], format, NULL, NULL);
		}
	else {
		poptFreeContext (popt_context);
		g_warning (_("Error loading SVG file.\n"));
		return 1;
	}

	g_object_unref (G_OBJECT (pixbuf));

	poptFreeContext (popt_context);
	rsvg_term();

	return 0;
}
