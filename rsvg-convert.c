/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*

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
#include <locale.h>

#include "rsvg-css.h"
#include "rsvg.h"
#include "rsvg-cairo.h"
#include "rsvg-private.h"
#include "rsvg-size-callback.h"

#ifdef CAIRO_HAS_PS_SURFACE
#include <cairo-ps.h>
#endif

#ifdef CAIRO_HAS_PDF_SURFACE
#include <cairo-pdf.h>
#endif

#ifdef CAIRO_HAS_SVG_SURFACE
#include <cairo-svg.h>
#endif

#ifdef CAIRO_HAS_XML_SURFACE
#include <cairo-xml.h>
#endif

static void
display_error (GError * err)
{
    if (err) {
        g_print ("%s\n", err->message);
        g_error_free (err);
    }
}

static RsvgHandle *
rsvg_handle_new_from_stdio_file (FILE * f, GError ** error)
{
    RsvgHandle *handle;
    gchar *current_dir;
    gchar *base_uri;

    handle = rsvg_handle_new ();

    while (!feof (f)) {
        guchar buffer[4096];
        gsize length = fread (buffer, 1, sizeof (buffer), f);

        if (length > 0) {
            if (!rsvg_handle_write (handle, buffer, length, error)) {
                g_object_unref (handle);
                return NULL;
            }
        } else if (ferror (f)) {
            g_object_unref (handle);
            return NULL;
        }
    }

    if (!rsvg_handle_close (handle, error)) {
        g_object_unref (handle);
        return NULL;
    }

    current_dir = g_get_current_dir ();
    base_uri = g_build_filename (current_dir, "file.svg", NULL);
    rsvg_handle_set_base_uri (handle, base_uri);
    g_free (base_uri);
    g_free (current_dir);

    return handle;
}

static void
rsvg_cairo_size_callback (int *width, int *height, gpointer data)
{
    RsvgDimensionData *dimensions = data;
    *width = dimensions->width;
    *height = dimensions->height;
}

static cairo_status_t
rsvg_cairo_write_func (void *closure, const unsigned char *data, unsigned int length)
{
    if (fwrite (data, 1, length, (FILE *) closure) == length)
        return CAIRO_STATUS_SUCCESS;
    return CAIRO_STATUS_WRITE_ERROR;
}

int
main (int argc, char **argv)
{
    GOptionContext *g_option_context;
    double x_zoom = 1.0;
    double y_zoom = 1.0;
    double zoom = 1.0;
    double dpi_x = -1.0;
    double dpi_y = -1.0;
    int width = -1;
    int height = -1;
    int bVersion = 0;
    char *format = NULL;
    char *output = NULL;
    int keep_aspect_ratio = FALSE;
    guint32 background_color = 0;
    char *background_color_str = NULL;
    char *base_uri = NULL;
    gboolean using_stdin = FALSE;
    GError *error = NULL;

    int i;
    char **args = NULL;
    gint n_args = 0;
    RsvgHandle *rsvg;
    cairo_surface_t *surface = NULL;
    cairo_t *cr = NULL;
    RsvgDimensionData dimensions;
    FILE *output_file = stdout;

    GOptionEntry options_table[] = {
        {"dpi-x", 'd', 0, G_OPTION_ARG_DOUBLE, &dpi_x,
         N_("pixels per inch [optional; defaults to 90dpi]"), N_("<float>")},
        {"dpi-y", 'p', 0, G_OPTION_ARG_DOUBLE, &dpi_y,
         N_("pixels per inch [optional; defaults to 90dpi]"), N_("<float>")},
        {"x-zoom", 'x', 0, G_OPTION_ARG_DOUBLE, &x_zoom,
         N_("x zoom factor [optional; defaults to 1.0]"), N_("<float>")},
        {"y-zoom", 'y', 0, G_OPTION_ARG_DOUBLE, &y_zoom,
         N_("y zoom factor [optional; defaults to 1.0]"), N_("<float>")},
        {"zoom", 'z', 0, G_OPTION_ARG_DOUBLE, &zoom, N_("zoom factor [optional; defaults to 1.0]"),
         N_("<float>")},
        {"width", 'w', 0, G_OPTION_ARG_INT, &width,
         N_("width [optional; defaults to the SVG's width]"), N_("<int>")},
        {"height", 'h', 0, G_OPTION_ARG_INT, &height,
         N_("height [optional; defaults to the SVG's height]"), N_("<int>")},
        {"format", 'f', 0, G_OPTION_ARG_STRING, &format,
         N_("save format [optional; defaults to 'png']"), N_("[png, pdf, ps, svg, xml, recording]")},
        {"output", 'o', 0, G_OPTION_ARG_STRING, &output,
         N_("output filename [optional; defaults to stdout]"), NULL},
        {"keep-aspect-ratio", 'a', 0, G_OPTION_ARG_NONE, &keep_aspect_ratio,
         N_("whether to preserve the aspect ratio [optional; defaults to FALSE]"), NULL},
        {"background-color", 'b', 0, G_OPTION_ARG_STRING, &background_color_str,
         N_("set the background color [optional; defaults to None]"), N_("[black, white, #abccee, #aaa...]")},
        {"version", 'v', 0, G_OPTION_ARG_NONE, &bVersion, N_("show version information"), NULL},
        {"base-uri", 'b', 0, G_OPTION_ARG_STRING, &base_uri, N_("base uri"), NULL},
        {G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, N_("[FILE...]")},
        {NULL}
    };

    /* Set the locale so that UTF-8 filenames work */
    setlocale(LC_ALL, "");

    g_type_init ();

    g_option_context = g_option_context_new (_("- SVG Converter"));
    g_option_context_add_main_entries (g_option_context, options_table, NULL);
    g_option_context_set_help_enabled (g_option_context, TRUE);
    if (!g_option_context_parse (g_option_context, &argc, &argv, &error)) {
        g_option_context_free (g_option_context);
        display_error (error);
        exit (1);
    }

    g_option_context_free (g_option_context);

    if (bVersion != 0) {
        printf (_("rsvg-convert version %s\n"), VERSION);
        return 0;
    }

    if (output != NULL) {
        output_file = fopen (output, "wb");
        if (!output_file) {
            fprintf (stderr, _("Error saving to file: %s\n"), output);
            g_free (output);
            exit (1);
        }

        g_free (output);
    }

    if (args)
        while (args[n_args] != NULL)
            n_args++;

    if (n_args == 0) {
        n_args = 1;
        using_stdin = TRUE;
    } else if (n_args > 1 && (!format || !(!strcmp (format, "ps") || !strcmp (format, "pdf")))) {
        fprintf (stderr, _("Multiple SVG files are only allowed for PDF and PS output.\n"));
        exit (1);
    }

    if (zoom != 1.0)
        x_zoom = y_zoom = zoom;

    rsvg_set_default_dpi_x_y (dpi_x, dpi_y);

    for (i = 0; i < n_args; i++) {

        if (using_stdin)
            rsvg = rsvg_handle_new_from_stdio_file (stdin, &error);
        else
            rsvg = rsvg_handle_new_from_file (args[i], &error);

        if (!rsvg) {
            fprintf (stderr, _("Error reading SVG:"));
            display_error (error);
            fprintf (stderr, "\n");
            exit (1);
        }

        if (base_uri)
            rsvg_handle_set_base_uri (rsvg, base_uri);

        /* in the case of multi-page output, all subsequent SVGs are scaled to the first's size */
        rsvg_handle_set_size_callback (rsvg, rsvg_cairo_size_callback, &dimensions, NULL);

        if (i == 0) {
            struct RsvgSizeCallbackData size_data;

            rsvg_handle_get_dimensions (rsvg, &dimensions);
            /* if both are unspecified, assume user wants to zoom the image in at least 1 dimension */
            if (width == -1 && height == -1) {
                size_data.type = RSVG_SIZE_ZOOM;
                size_data.x_zoom = x_zoom;
                size_data.y_zoom = y_zoom;
                size_data.keep_aspect_ratio = keep_aspect_ratio;
            }
            /* if both are unspecified, assume user wants to resize image in at least 1 dimension */
            else if (x_zoom == 1.0 && y_zoom == 1.0) {
                /* if one parameter is unspecified, assume user wants to keep the aspect ratio */
                if (width == -1 || height == -1) {
                    size_data.type = RSVG_SIZE_WH_MAX;
                    size_data.width = width;
                    size_data.height = height;
                    size_data.keep_aspect_ratio = keep_aspect_ratio;
                } else {
                    size_data.type = RSVG_SIZE_WH;
                    size_data.width = width;
                    size_data.height = height;
                    size_data.keep_aspect_ratio = keep_aspect_ratio;
                }
            } else {
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
#ifdef CAIRO_HAS_XML_SURFACE
            else if (!strcmp (format, "xml")) {
                cairo_device_t *device = cairo_xml_create_for_stream (rsvg_cairo_write_func, output_file);
                surface = cairo_xml_surface_create (device, CAIRO_CONTENT_COLOR_ALPHA,
                                                    dimensions.width, dimensions.height);
                cairo_device_destroy (device);
            }
#if CAIRO_VERSION >= CAIRO_VERSION_ENCODE (1, 10, 0)
            else if (!strcmp (format, "recording"))
                surface = cairo_recording_surface_create (CAIRO_CONTENT_COLOR_ALPHA, NULL);
#endif
#endif
            else {
                fprintf (stderr, _("Unknown output format."));
                exit (1);
            }

            cr = cairo_create (surface);
        }

        // Set background color
        if (background_color_str && g_ascii_strcasecmp(background_color_str, "none") != 0) {
            background_color = rsvg_css_parse_color(background_color_str, FALSE);

            cairo_set_source_rgb (
                cr, 
                ((background_color >> 16) & 0xff) / 255.0, 
                ((background_color >> 8) & 0xff) / 255.0, 
                ((background_color >> 0) & 0xff) / 255.0);
            cairo_rectangle (cr, 0, 0, dimensions.width, dimensions.height);
            cairo_fill (cr);
        }

        rsvg_handle_render_cairo (rsvg, cr);

        if (!format || !strcmp (format, "png"))
            cairo_surface_write_to_png_stream (surface, rsvg_cairo_write_func, output_file);
#if CAIRO_HAS_XML_SURFACE && CAIRO_VERSION >= CAIRO_VERSION_ENCODE (1, 10, 0)
        else if (!strcmp (format, "recording")) {
            cairo_device_t *device = cairo_xml_create_for_stream (rsvg_cairo_write_func, output_file);
            cairo_xml_for_recording_surface (device, surface);
            cairo_device_destroy (device);
        }
#endif
        else if (!strcmp (format, "xml"))
          ;
        else if (!strcmp (format, "svg") || !strcmp (format, "pdf") || !strcmp (format, "ps"))
            cairo_show_page (cr);
        else
          g_assert_not_reached ();

        g_object_unref (rsvg);
    }

    cairo_destroy (cr);

    cairo_surface_destroy (surface);

    fclose (output_file);

    g_strfreev (args);

    rsvg_cleanup ();

    return 0;
}
