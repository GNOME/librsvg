/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
 * License: Public Domain.
 * Author: Robert Staudinger <robsta@gnome.org>.
 */

#include <stdio.h>
#include <stdlib.h>
#include <glib.h>
#include <rsvg.h>

static void
show_help (GOptionContext *context)
{
    char *help;
    help = g_option_context_get_help (context, TRUE, NULL);
    perror (help);
    g_free (help), help = NULL;
}

int
main (int	  argc,
      char	**argv)
{
    GOptionContext      *context;
    char const          *fragment;
    char const         **filenames;
    char const          *file;
    RsvgHandle          *handle;
    RsvgDimensionData    dimensions;
    RsvgPositionData     position;
    GError              *error;
    int                  exit_code;
    int                  i;

    GOptionEntry options[] = {
        { "fragment", 'f', 0, G_OPTION_ARG_STRING, &fragment, "The SVG fragment to address.", "<string>" },
        { G_OPTION_REMAINING, 0, G_OPTION_FLAG_FILENAME, G_OPTION_ARG_FILENAME_ARRAY, &filenames, NULL, "[FILE...]" },
        { NULL }
    };

    g_type_init ();

    context = NULL;
    fragment = NULL;
    filenames = NULL;
    handle = NULL;
    error = NULL;

    context = g_option_context_new ("- SVG measuring tool.");
    g_option_context_add_main_entries (context, options, NULL);

    /* No args? */
    if (argc < 2) {
        show_help (context);
        exit_code = EXIT_SUCCESS;
        goto bail;
    }

    error = NULL;
    g_option_context_parse (context, &argc, &argv, &error);
    if (error) {
        show_help (context);
        g_warning ("%s", error->message);
        exit_code = EXIT_FAILURE;
        goto bail;
    }

    /* Invalid / missing args? */
    if (filenames == NULL) {
        show_help (context);
        exit_code = EXIT_FAILURE;
        goto bail;
    }

    g_option_context_free (context), context = NULL;

    for (i = 0; NULL != (file = filenames[i]); i++) {

        error = NULL;
        handle = rsvg_handle_new_from_file (file, &error);
        if (error) {
            g_warning ("%s", error->message);
            exit_code = EXIT_FAILURE;
            goto bail;
        }

        if (fragment && handle) {
            gboolean have_fragment = FALSE;
            have_fragment |= rsvg_handle_get_dimensions_sub (handle,
                    &dimensions, fragment);
            have_fragment |= rsvg_handle_get_position_sub (handle,
                    &position, fragment);
            if (!have_fragment) {
                g_warning ("%s: fragment `'%s' not found.",
                        file, fragment);
                exit_code = EXIT_FAILURE;
                goto bail;
            }

            printf ("%s, fragment `%s': x=%d, y=%d, %dx%d, em=%f, ex=%f\n",
                    file, fragment,
                    position.x, position.y,
                    dimensions.width, dimensions.height,
                    dimensions.em, dimensions.ex);

        } else if (handle) {
            rsvg_handle_get_dimensions (handle, &dimensions);
            printf ("%s: %dx%d, em=%f, ex=%f\n", file,
                    dimensions.width, dimensions.height,
                    dimensions.em, dimensions.ex);
        } else {
            g_warning ("Could not open file `%s'", file);
            exit_code = EXIT_FAILURE;
            goto bail;
        }

        g_object_unref (handle), handle = NULL;
    }

    exit_code = EXIT_SUCCESS;

bail:
    if (handle)
        g_object_unref (handle), handle = NULL;
    if (context)
        g_option_context_free (context), context = NULL;
    if (error)
        g_error_free (error), error = NULL;

    rsvg_cleanup ();

    return exit_code;
}
