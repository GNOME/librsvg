/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 4 -*-

   rsvg-convert.c: Command line utility for exercising rsvg with cairo.
 
   Copyright (C) 2005 Red Hat, Inc.
   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2005 Caleb Moore <c.moore@student.unsw.edu.au>
  
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
  
   Authors: Carl Worth <cworth@cworth.org>, 
            Caleb Moore <c.moore@student.unsw.edu.au>,
            Dom Lachowicz <cinamod@hotmail.com>
*/

#include "config.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <popt.h>

#include "rsvg.h"
#include "rsvg-cairo.h"
#include "rsvg-private.h"

#ifdef CAIRO_HAS_PS_SURFACE
#include <cairo-ps.h>
#endif

#ifdef CAIRO_HAS_PDF_SURFACE
#include <cairo-pdf.h>
#endif

#ifdef CAIRO_HAS_SVG_SURFACE
#include <cairo-svg.h>
#endif

static RsvgHandle * 
rsvg_handle_new_from_stdio_file (FILE * f,
								 GError **error)
{
	RsvgHandle * handle;
	gchar * base_uri;
	
	handle = rsvg_handle_new ();

	while (!feof (f)) 
		{
			guchar buffer [4096];
			gsize length = fread (buffer, 1, sizeof (buffer), f);

			if (length > 0) 
				{
					if (!rsvg_handle_write (handle, buffer, length, error))
						{
							g_object_unref (G_OBJECT(handle));
							return NULL;
						}
				}
			else if (ferror (f)) 
				{
					g_object_unref (G_OBJECT(handle));
					return NULL;
				}
		}

	if(!rsvg_handle_close (handle, error)) {
		g_object_unref(G_OBJECT(handle));
		return NULL;
	}

	base_uri = g_get_current_dir ();
	rsvg_handle_set_base_uri (handle, base_uri);
	g_free (base_uri);

	return handle;
}

static void
rsvg_cairo_size_callback (int *width,
						  int *height,
						  gpointer  data)
{
	RsvgDimensionData * dimensions = data;
	*width = dimensions->width;
	*height = dimensions->height;
}

static cairo_status_t
rsvg_cairo_write_func (void *closure,
					   const unsigned char *data,
					   unsigned int length)
{
	fwrite (data, 1, length, (FILE *)closure);
	return CAIRO_STATUS_SUCCESS;
}

int
main (int argc, const char **argv)
{
	poptContext popt_context;
	double x_zoom = 1.0;
	double y_zoom = 1.0;
	double zoom = 1.0;
	double dpi_x = -1.0;
	double dpi_y = -1.0;
	int width  = -1;
	int height = -1;
	int bVersion = 0;
	char * format = NULL;
	char * output = NULL;
	int keep_aspect_ratio = FALSE;
	char * base_uri = NULL;
	gboolean using_stdin = FALSE;

	struct poptOption options_table[] = {
		{ "dpi-x",   'd',  POPT_ARG_DOUBLE, &dpi_x,    0, N_("pixels per inch [optional; defaults to 90dpi]"), N_("<float>") },
		{ "dpi-y",   'p',  POPT_ARG_DOUBLE, &dpi_y,    0, N_("pixels per inch [optional; defaults to 90dpi]"), N_("<float>") },
		{ "x-zoom",  'x',  POPT_ARG_DOUBLE, &x_zoom,   0, N_("x zoom factor [optional; defaults to 1.0]"), N_("<float>") },
		{ "y-zoom",  'y',  POPT_ARG_DOUBLE, &y_zoom,   0, N_("y zoom factor [optional; defaults to 1.0]"), N_("<float>") },
		{ "zoom",    'z',  POPT_ARG_DOUBLE, &zoom,     0, N_("zoom factor [optional; defaults to 1.0]"), N_("<float>") },
		{ "width",   'w',  POPT_ARG_INT,    &width,    0, N_("width [optional; defaults to the SVG's width]"), N_("<int>") },
		{ "height",  'h',  POPT_ARG_INT,    &height,   0, N_("height [optional; defaults to the SVG's height]"), N_("<int>") },		
		{ "format",  'f',  POPT_ARG_STRING, &format,   0, N_("save format [optional; defaults to 'png']"), N_("[png, pdf, ps, svg]") },
		{ "output",  'o',  POPT_ARG_STRING, &output,   0, N_("output filename [optional; defaults to stdout]"), NULL },
		{ "keep-aspect-ratio", 'a', POPT_ARG_NONE, &keep_aspect_ratio, 0, N_("whether to preserve the aspect ratio [optional; defaults to FALSE]"), NULL },
		{ "version", 'v',  POPT_ARG_NONE,   &bVersion, 0, N_("show version information"), NULL },
		{ "base-uri", 'b', POPT_ARG_STRING, &base_uri, 0, N_("base uri"), NULL },
		POPT_AUTOHELP
		POPT_TABLEEND
	};
	int c, i;
	const char * const *args;
	gint n_args = 0;
	RsvgHandle *rsvg;
	cairo_surface_t *surface = NULL;
	cairo_t *cr = NULL;
	RsvgDimensionData dimensions;
	FILE * output_file = stdout;

	popt_context = poptGetContext ("rsvg-cairo", argc, argv, options_table, 0);
	poptSetOtherOptionHelp (popt_context, _("[OPTIONS...]"));

	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);

	if (bVersion != 0)
		{
		    g_print (_("rsvg-cairo version %s\n"), VERSION);
			return 0;
		}

	if (output != NULL)
		{
			output_file = fopen (output, "wb");
			if (!output_file)
				{
					g_print (_("Error saving to file %s\n"), output);
					exit (1);
				}
		}

	if (args)
		while (args[n_args] != NULL)
			n_args++;

	if (n_args == 0)
		{
			n_args = 1;
			using_stdin = TRUE;
		}
	else if (n_args > 1 && (!format || !strcmp (format, "png"))) 
		{
			g_print (_("Multiple SVG files are only allowed for PDF, PS and SVG output.\n"));
			exit (1);
		}

	if (zoom != 1.0)
		x_zoom = y_zoom = zoom;

	rsvg_init ();
	rsvg_set_default_dpi_x_y (dpi_x, dpi_y);

	for(i = 0; i < n_args; i++) 
		{
			if (using_stdin)
				rsvg = rsvg_handle_new_from_stdio_file (stdin, NULL);
			else
				rsvg = rsvg_handle_new_from_file (args[i], NULL);
			
			if (base_uri)
				rsvg_handle_set_base_uri (rsvg, base_uri);

			/* in the case of multi-page output, all subsequent SVS are scaled to the first's size */
			rsvg_handle_set_size_callback (rsvg, rsvg_cairo_size_callback, &dimensions, NULL);			

			if (i == 0) 
				{
					struct RsvgSizeCallbackData size_data;

					rsvg_handle_get_dimensions (rsvg, &dimensions);
					/* if both are unspecified, assume user wants to zoom the image in at least 1 dimension */
					if (width == -1 && height == -1)
						{
							size_data.type = RSVG_SIZE_ZOOM;
							size_data.x_zoom = x_zoom;
							size_data.y_zoom = y_zoom;
							size_data.keep_aspect_ratio = keep_aspect_ratio;
						}
					/* if both are unspecified, assume user wants to resize image in at least 1 dimension */
					else if (x_zoom == 1.0 && y_zoom == 1.0)
						{
							/* if one parameter is unspecified, assume user wants to keep the aspect ratio */
							if (width == -1 || height == -1)
								{
									size_data.type = RSVG_SIZE_WH_MAX;
									size_data.width = width;
									size_data.height = height;
									size_data.keep_aspect_ratio = keep_aspect_ratio;
								}
							else
								{
									size_data.type = RSVG_SIZE_WH;
									size_data.width = width;
									size_data.height = height;
									size_data.keep_aspect_ratio = keep_aspect_ratio;
								}
						}
					else
						{
							/* assume the user wants to zoom the image, but cap the maximum size */
							size_data.type = RSVG_SIZE_ZOOM_MAX;
							size_data.x_zoom = x_zoom;
							size_data.y_zoom = y_zoom;
							size_data.width = width;
							size_data.height = height;
							size_data.keep_aspect_ratio = keep_aspect_ratio;
						}

					_rsvg_size_callback (&dimensions.width, &dimensions.height, &size_data);
					
					if (!format || !strcmp (format, "png"))
						surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
															  dimensions.width, dimensions.height);
#ifdef CAIRO_HAS_PDF_SURFACE
					else if (!strcmp (format, "pdf"))
						surface = cairo_pdf_surface_create_for_stream (rsvg_cairo_write_func, output_file,
																	   dimensions.width, dimensions.height);
#endif
#ifdef CAIRO_HAS_PS_SURFACE
					else if (!strcmp (format, "ps"))
						surface = cairo_ps_surface_create_for_stream (rsvg_cairo_write_func, output_file,
																	  dimensions.width, dimensions.height);
#endif
#ifdef CAIRO_HAS_SVG_SURFACE
					else if (!strcmp (format, "svg"))
						surface = cairo_svg_surface_create_for_stream (rsvg_cairo_write_func, output_file,
																	   dimensions.width, dimensions.height);
#endif
					else 
						{
							g_error ("Unknown output format.");
							exit (1);
						}

					cr = cairo_create (surface);
				}

			/* cairo deficiency - need to clear the pixels to full-alpha */
			if(!format || !strcmp(format, "png")) {
				cairo_save(cr);
				cairo_set_operator(cr, CAIRO_OPERATOR_CLEAR);
				cairo_paint(cr);
				cairo_restore(cr);
			}

			rsvg_handle_render_cairo (rsvg, cr);

			if (!format || !strcmp (format, "png"))
				cairo_surface_write_to_png_stream (surface, rsvg_cairo_write_func, output_file);
			else
				cairo_show_page (cr);

			g_object_unref (G_OBJECT(rsvg));
		}
	
	cairo_destroy (cr);
	cairo_surface_destroy (surface);

	fclose (output_file);

	poptFreeContext (popt_context);
	rsvg_term();

	return 0;
}
