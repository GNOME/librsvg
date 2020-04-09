/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*

   rsvg-convert.c: Command line utility for exercising rsvg with cairo.
 
   Copyright (C) 2005 Red Hat, Inc.
   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2005 Caleb Moore <c.moore@student.unsw.edu.au>
   Copyright (C) 2019 Federico Mena Quintero <federico@gnome.org>
  
   This library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public
   License as published by the Free Software Foundation; either
   version 2.1 of the License, or (at your option) any later version.

   This library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with this library; if not, write to the Free Software
   Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
  
   Authors: Carl Worth <cworth@cworth.org>, 
            Caleb Moore <c.moore@student.unsw.edu.au>,
            Dom Lachowicz <cinamod@hotmail.com>,
            Federico Mena Quintero <federico@gnome.org>
*/

#define RSVG_DISABLE_DEPRECATION_WARNINGS

#include "config.h"

#include <math.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <locale.h>
#include <glib/gi18n.h>
#include <gio/gio.h>

#ifdef G_OS_UNIX
#include <gio/gunixinputstream.h>
#endif

#ifdef G_OS_WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <io.h>
#include <fcntl.h>

#include <gio/gwin32inputstream.h>
#endif

#include "librsvg/rsvg-css.h"
#include "librsvg/rsvg.h"

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

typedef enum {
    SIZE_KIND_ZOOM,
    SIZE_KIND_WH,
    SIZE_KIND_WH_MAX,
    SIZE_KIND_ZOOM_MAX
} SizeKind;

typedef struct {
    SizeKind kind;
    double x_zoom;
    double y_zoom;
    gint width;
    gint height;

    gboolean keep_aspect_ratio;
} SizeMode;

static void
get_final_size (int *width, int *height, SizeMode *real_data)
{
    double zoomx, zoomy, zoom;

    int in_width, in_height;

    in_width = *width;
    in_height = *height;

    switch (real_data->kind) {
    case SIZE_KIND_ZOOM:
        if (*width < 0 || *height < 0)
            return;

        *width = floor (real_data->x_zoom * *width + 0.5);
        *height = floor (real_data->y_zoom * *height + 0.5);
        break;

    case SIZE_KIND_ZOOM_MAX:
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

    case SIZE_KIND_WH_MAX:
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

    case SIZE_KIND_WH:
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

static void
display_error (GError * err)
{
    if (err) {
        g_printerr ("%s\n", err->message);
        g_error_free (err);
    }
}

static cairo_status_t
rsvg_cairo_write_func (void *closure, const unsigned char *data, unsigned int length)
{
    if (fwrite (data, 1, length, (FILE *) closure) == length)
        return CAIRO_STATUS_SUCCESS;
    return CAIRO_STATUS_WRITE_ERROR;
}

static char *
get_lookup_id_from_command_line (const char *lookup_id)
{
    char *export_lookup_id;

    if (lookup_id == NULL)
        export_lookup_id = NULL;
    else {
        /* rsvg_handle_has_sub() expects ids to have a '#' prepended to them, so
         * it can lookup ids in externs like "subfile.svg#subid".  For the
         * user's convenience, we include this '#' automatically; we only
         * support specifying ids from the toplevel, and don't expect users to
         * lookup things in externs.
         */
        export_lookup_id = g_strdup_printf ("#%s", lookup_id);
    }

    return export_lookup_id;
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
    char *stylesheet = NULL;
    char *export_id = NULL;
    int keep_aspect_ratio = FALSE;
    guint32 background_color = 0;
    char *background_color_str = NULL;
    gboolean using_stdin = FALSE;
    gboolean unlimited = FALSE;
    gboolean keep_image_data = FALSE;
    gboolean no_keep_image_data = FALSE;
    GError *error = NULL;

    gboolean success = TRUE;

    int i;
    char **args = NULL;
    gint n_args = 0;
    RsvgHandle *rsvg = NULL;
    cairo_surface_t *surface = NULL;
    cairo_t *cr = NULL;
    RsvgHandleFlags flags = RSVG_HANDLE_FLAGS_NONE;
    RsvgDimensionData dimensions;
    FILE *output_file = stdout;
    char *export_lookup_id;
    double unscaled_width, unscaled_height;
    int scaled_width, scaled_height;

    char *stylesheet_data = NULL;
    gsize stylesheet_data_len = 0;

    char buffer[25];
    char *endptr;
    char *source_date_epoch;
    time_t now;
    struct tm *build_time;
    unsigned long long epoch;

#ifdef G_OS_WIN32
    HANDLE handle;
#endif

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
         N_("save format [optional; defaults to 'png']"), N_("[png, pdf, ps, eps, svg, xml, recording]")},
        {"output", 'o', 0, G_OPTION_ARG_STRING, &output,
         N_("output filename [optional; defaults to stdout]"), NULL},
        {"export-id", 'i', 0, G_OPTION_ARG_STRING, &export_id,
         N_("SVG id of object to export [optional; defaults to exporting all objects]"), N_("<object id>")},
        {"keep-aspect-ratio", 'a', 0, G_OPTION_ARG_NONE, &keep_aspect_ratio,
         N_("whether to preserve the aspect ratio [optional; defaults to FALSE]"), NULL},
        {"background-color", 'b', 0, G_OPTION_ARG_STRING, &background_color_str,
         N_("set the background color [optional; defaults to None]"), N_("[black, white, #abccee, #aaa...]")},
        {"stylesheet", 's', 0, G_OPTION_ARG_FILENAME, &stylesheet, N_("Filename of CSS stylesheet"), NULL},
        {"unlimited", 'u', 0, G_OPTION_ARG_NONE, &unlimited, N_("Allow huge SVG files"), NULL},
        {"keep-image-data", 0, 0, G_OPTION_ARG_NONE, &keep_image_data, N_("Keep image data"), NULL},
        {"no-keep-image-data", 0, 0, G_OPTION_ARG_NONE, &no_keep_image_data, N_("Don't keep image data"), NULL},
        {"version", 'v', 0, G_OPTION_ARG_NONE, &bVersion, N_("show version information"), NULL},
        {G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, N_("[FILE...]")},
        {NULL}
    };

    /* Set the locale so that UTF-8 filenames work */
    setlocale(LC_ALL, "");

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

    if (stylesheet != NULL) {
        error = NULL;
        if (!g_file_get_contents (stylesheet, &stylesheet_data, &stylesheet_data_len, &error)) {
            g_printerr (_("Error reading stylesheet: %s\n"), error->message);
            exit (1);
        }
    }

    if (output != NULL) {
        output_file = fopen (output, "wb");
        if (!output_file) {
            g_printerr (_("Error saving to file: %s\n"), output);
            g_free (output);
            exit (1);
        }

        g_free (output);
    }
#ifdef G_OS_WIN32
    else {
        setmode (fileno (stdout), O_BINARY);
    }
#endif   

    if (args)
        while (args[n_args] != NULL)
            n_args++;

    if (n_args == 0) {
        const gchar * const stdin_args[] = { "stdin", NULL };
        n_args = 1;
        using_stdin = TRUE;
        g_strfreev (args);
        args = g_strdupv ((gchar **) stdin_args);
    } else if (n_args > 1 && (!format || !(!strcmp (format, "ps") || !strcmp (format, "eps") || !strcmp (format, "pdf")))) {
        g_printerr (_("Multiple SVG files are only allowed for PDF and (E)PS output.\n"));
        exit (1);
    }

    if (dpi_x <= 0.0) {
        dpi_x = 90.0;
    }

    if (dpi_y <= 0.0) {
        dpi_y = 90.0;
    }

    if (format != NULL &&
        (g_str_equal (format, "ps") || g_str_equal (format, "eps") || g_str_equal (format, "pdf")) &&
        !no_keep_image_data)
        keep_image_data = TRUE;

    if (zoom != 1.0)
        x_zoom = y_zoom = zoom;

    if (unlimited)
        flags |= RSVG_HANDLE_FLAG_UNLIMITED;

    if (keep_image_data)
        flags |= RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA;

    for (i = 0; i < n_args && success; i++) {
        GFile *file;
        GInputStream *stream;

        if (using_stdin) {
            file = NULL;
#ifdef _WIN32
            handle = GetStdHandle (STD_INPUT_HANDLE);

            if (handle == INVALID_HANDLE_VALUE) {
              gchar *emsg = g_win32_error_message (GetLastError());
              g_printerr ( _("Unable to acquire HANDLE for STDIN: %s\n"), emsg);
              g_free (emsg);
              exit (1);
            }
            stream = g_win32_input_stream_new (handle, FALSE);
#else
            stream = g_unix_input_stream_new (STDIN_FILENO, FALSE);
#endif
        } else {
            file = g_file_new_for_commandline_arg (args[i]);
            stream = (GInputStream *) g_file_read (file, NULL, &error);

            if (stream == NULL)
                goto done;
        }

        rsvg = rsvg_handle_new_from_stream_sync (stream, file, flags, NULL, &error);

    done:
        g_clear_object (&stream);
        g_clear_object (&file);

        if (error != NULL) {
            g_printerr (_("Error reading SVG:"));
            display_error (error);
            g_printerr ("\n");
            exit (1);
        }

        g_assert (rsvg != NULL);

        if (stylesheet_data != NULL) {
            if (!rsvg_handle_set_stylesheet (rsvg, stylesheet_data, stylesheet_data_len, &error)) {
                g_printerr (_("Error in stylesheet: %s\n"), error->message);
                exit (1);
            }
        }

        rsvg_handle_set_dpi_x_y (rsvg, dpi_x, dpi_y);

        export_lookup_id = get_lookup_id_from_command_line (export_id);
        if (export_lookup_id != NULL
            && !rsvg_handle_has_sub (rsvg, export_lookup_id)) {
            g_printerr (_("File %s does not have an object with id \"%s\"\n"), args[i], export_id);
            exit (1);
        }

        if (i == 0) {
            SizeMode size_data;

            if (!rsvg_handle_get_dimensions_sub (rsvg, &dimensions, export_lookup_id)) {
                g_printerr ("Could not get dimensions for file %s\n", args[i]);
                exit (1);
            }

            if (dimensions.width == 0 || dimensions.height == 0) {
                g_printerr ("The SVG %s has no dimensions\n", args[i]);
                exit (1);
            }

            unscaled_width = dimensions.width;
            unscaled_height = dimensions.height;

            /* if both are unspecified, assume user wants to zoom the image in at least 1 dimension */
            if (width == -1 && height == -1) {
                size_data.kind = SIZE_KIND_ZOOM;
                size_data.x_zoom = x_zoom;
                size_data.y_zoom = y_zoom;
                size_data.keep_aspect_ratio = keep_aspect_ratio;
            } else if (x_zoom == 1.0 && y_zoom == 1.0) {
                /* if one parameter is unspecified, assume user wants to keep the aspect ratio */
                if (width == -1 || height == -1) {
                    size_data.kind = SIZE_KIND_WH_MAX;
                    size_data.width = width;
                    size_data.height = height;
                    size_data.keep_aspect_ratio = keep_aspect_ratio;
                } else {
                    size_data.kind = SIZE_KIND_WH;
                    size_data.width = width;
                    size_data.height = height;
                    size_data.keep_aspect_ratio = keep_aspect_ratio;
                }
            } else {
                /* assume the user wants to zoom the image, but cap the maximum size */
                size_data.kind = SIZE_KIND_ZOOM_MAX;
                size_data.x_zoom = x_zoom;
                size_data.y_zoom = y_zoom;
                size_data.width = width;
                size_data.height = height;
                size_data.keep_aspect_ratio = keep_aspect_ratio;
            }

            scaled_width = dimensions.width;
            scaled_height = dimensions.height;
            get_final_size (&scaled_width, &scaled_height, &size_data);

            if (scaled_width > 32767 || scaled_height > 32767) {
                g_printerr (_("The resulting image would be larger than 32767 pixels on either dimension.\n"
                              "Librsvg currently cannot render to images bigger than that.\n"
                              "Please specify a smaller size.\n"));
                exit (1);
            }

            if (!format || !strcmp (format, "png"))
                surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
                                                      scaled_width, scaled_height);
#ifdef CAIRO_HAS_PDF_SURFACE
            else if (!strcmp (format, "pdf")) {
                surface = cairo_pdf_surface_create_for_stream (rsvg_cairo_write_func, output_file,
                                                               scaled_width, scaled_height);
                source_date_epoch = getenv("SOURCE_DATE_EPOCH");
                if (source_date_epoch) {
                    errno = 0;
                    epoch = strtoull(source_date_epoch, &endptr, 10);
                    if ((errno == ERANGE && (epoch == ULLONG_MAX || epoch == 0))
                            || (errno != 0 && epoch == 0)) {
                        g_printerr (_("Environment variable $SOURCE_DATE_EPOCH: strtoull: %s\n"),
                                    strerror(errno));
                        exit (1);
                    }
                    if (endptr == source_date_epoch) {
                        g_printerr (_("Environment variable $SOURCE_DATE_EPOCH: No digits were found: %s\n"),
                                    endptr);
                        exit (1);
                    }
                    if (*endptr != '\0') {
                        g_printerr (_("Environment variable $SOURCE_DATE_EPOCH: Trailing garbage: %s\n"),
                                    endptr);
                        exit (1);
                    }
                    if (epoch > ULONG_MAX) {
                        g_printerr (_("Environment variable $SOURCE_DATE_EPOCH: value must be smaller than or equal to %lu but was found to be: %llu \n"),
                                    ULONG_MAX, epoch);
                        exit (1);
                    }
                    now = (time_t) epoch;
                    build_time = gmtime(&now);
                    g_assert (strftime (buffer, sizeof (buffer), "%Y-%m-%dT%H:%M:%S%z", build_time));
                    cairo_pdf_surface_set_metadata (surface,
                                                    CAIRO_PDF_METADATA_CREATE_DATE,
                                                    buffer);
                }
            }
#endif
#ifdef CAIRO_HAS_PS_SURFACE
            else if (!strcmp (format, "ps") || !strcmp (format, "eps")){
                surface = cairo_ps_surface_create_for_stream (rsvg_cairo_write_func, output_file,
                                                              scaled_width, scaled_height);
                if(!strcmp (format, "eps"))
                    cairo_ps_surface_set_eps(surface, TRUE);
            }
#endif
#ifdef CAIRO_HAS_SVG_SURFACE
            else if (!strcmp (format, "svg")) {
                surface = cairo_svg_surface_create_for_stream (rsvg_cairo_write_func, output_file,
                                                               scaled_width, scaled_height);
                cairo_svg_surface_set_document_unit(surface, CAIRO_SVG_UNIT_PX);
            }
#endif
#ifdef CAIRO_HAS_XML_SURFACE
            else if (!strcmp (format, "xml")) {
                cairo_device_t *device = cairo_xml_create_for_stream (rsvg_cairo_write_func, output_file);
                surface = cairo_xml_surface_create (device, CAIRO_CONTENT_COLOR_ALPHA,
                                                    scaled_width, scaled_height);
                cairo_device_destroy (device);
            }
#if CAIRO_VERSION >= CAIRO_VERSION_ENCODE (1, 10, 0)
            else if (!strcmp (format, "recording"))
                surface = cairo_recording_surface_create (CAIRO_CONTENT_COLOR_ALPHA, NULL);
#endif
#endif
            else {
                g_printerr (_("Unknown output format.\n"));
                exit (1);
            }

            cr = cairo_create (surface);
            cairo_scale (cr,
                         scaled_width / unscaled_width,
                         scaled_height / unscaled_height);
        }

        // Set background color
        if (background_color_str && g_ascii_strcasecmp(background_color_str, "none") != 0) {
            RsvgCssColorSpec spec;

            spec = rsvg_css_parse_color_ (background_color_str);
            if (spec.kind == RSVG_CSS_COLOR_SPEC_ARGB) {
                background_color = spec.argb;
            } else {
                g_printerr (_("Invalid color specification.\n"));
                exit (1);
            }

            cairo_set_source_rgba (
                cr, 
                ((background_color >> 16) & 0xff) / 255.0, 
                ((background_color >> 8) & 0xff) / 255.0, 
                ((background_color >> 0) & 0xff) / 255.0,
                ((background_color >> 24) & 0xff) / 255.0);
            cairo_rectangle (cr, 0, 0, unscaled_width, unscaled_height);
            cairo_fill (cr);
        }

        if (export_lookup_id) {
            RsvgPositionData pos;

            if (!rsvg_handle_get_position_sub (rsvg, &pos, export_lookup_id)) {
                g_printerr (_("File %s does not have an object with id \"%s\"\n"), args[i], export_id);
                exit (1);
            }

            /* Move the whole thing to 0, 0 so the object to export is at the origin */
            cairo_translate (cr, -pos.x, -pos.y);
        }

        if (!rsvg_handle_render_cairo_sub (rsvg, cr, export_lookup_id)) {
            g_printerr ("Could not render file %s\n", args[i]);
            exit (1);
        }

        g_free (export_lookup_id);

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
        else if (!strcmp (format, "svg") || !strcmp (format, "pdf") || !strcmp (format, "ps") || !strcmp (format, "eps"))
            cairo_show_page (cr);
        else
          g_assert_not_reached ();

        g_object_unref (rsvg);
    }

    cairo_destroy (cr);

    cairo_surface_destroy (surface);

    fclose (output_file);

    g_strfreev (args);

    return 0;
}
