/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 4 -*-

   rsvg-cairo.c: Command line utility for exercising rsvg with cairo.
 
   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz
   Copyright (C) 2005 Carl Worth.
  
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
  
   Authors: Raph Levien <raph@artofcode.com>
            Carl Worth <cworth@cworth.org>
*/

#include "config.h"
#include "rsvg-cairo.h"
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

	struct poptOption options_table[] = {
		{ "dpi-x",   'd',  POPT_ARG_DOUBLE, &dpi_x,    0, N_("pixels per inch"), N_("<float>") },
		{ "dpi-y",   'p',  POPT_ARG_DOUBLE, &dpi_y,    0, N_("pixels per inch"), N_("<float>") },
		{ "x-zoom",  'x',  POPT_ARG_DOUBLE, &x_zoom,   0, N_("x zoom factor"), N_("<float>") },
		{ "y-zoom",  'y',  POPT_ARG_DOUBLE, &y_zoom,   0, N_("y zoom factor"), N_("<float>") },
		{ "width",   'w',  POPT_ARG_INT,    &width,    0, N_("width"), N_("<int>") },
		{ "height",  'h',  POPT_ARG_INT,    &height,   0, N_("height"), N_("<int>") },
		{ "version", 'v',  POPT_ARG_NONE,   &bVersion, 0, N_("show version information"), NULL },
		POPT_AUTOHELP
		POPT_TABLEEND
	};
	int c;
	const char * const *args;
	gint n_args = 0;
	RsvgHandle *rsvg;
	cairo_surface_t *surface;
	cairo_t *cr;
	RsvgDimensionData dimensions;

	popt_context = poptGetContext ("rsvg-cairo", argc, argv, options_table, 0);
	poptSetOtherOptionHelp(popt_context, _("[OPTIONS...] file.svg file.png"));

	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);

	if (bVersion != 0)
		{
		    g_print ("rsvg-cairo version %s\n", VERSION);
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

	rsvg_init ();

	rsvg_set_default_dpi (dpi_x, dpi_y);

	rsvg = rsvg_handle_new_from_file (args[0], NULL);
	rsvg_handle_get_dimensions (rsvg, &dimensions);

	/* XXX: Need to handle various scaling options here. */

	surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
										  dimensions.width, dimensions.height);
	cr = cairo_create (surface);

	rsvg_cairo_render (cr, rsvg);

	cairo_surface_write_to_png (surface, args[1]);

	cairo_destroy (cr);
	cairo_surface_destroy (surface);

	rsvg_handle_free (rsvg);

	poptFreeContext (popt_context);
	rsvg_term();

	return 0;
}
