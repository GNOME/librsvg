
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
	GOptionContext		 *context;
	char const		 *fragment;
	char const	 	**filenames;
	char const		 *file;
	RsvgHandle		 *handle;
	RsvgDimensionData	  dimensions;
	GError			 *error;
	int			  exit_code;
	int                       i;

	GOptionEntry options[] = {
		{ "fragment", 'f', 0, G_OPTION_ARG_STRING, &fragment, "The SVG fragment to address.", "<string>" },
		{ G_OPTION_REMAINING, 0, G_OPTION_FLAG_FILENAME, G_OPTION_ARG_FILENAME_ARRAY, &filenames, NULL, "[FILE...]" },
		{ NULL }
	};

	rsvg_init ();

	fragment = NULL;
	filenames = NULL;

	context = g_option_context_new ("- SVG measuring tool.");
	g_option_context_add_main_entries (context, options, NULL);

	/* No args? */
	if (argc < 2) {
		show_help (context);
		exit_code = EXIT_SUCCESS;
		goto bail1;
	}

	error = NULL;
	g_option_context_parse (context, &argc, &argv, &error);
	if (error) {
		show_help (context);
		g_warning (error->message);
		exit_code = EXIT_FAILURE;
		goto bail2;
	}

	/* Invalid / missing args? */
	if (filenames == NULL) {
		show_help (context);
		exit_code = EXIT_FAILURE;
		goto bail2;
	}

	g_option_context_free (context), context = NULL;

	for (i = 0; NULL != (file = filenames[i]); i++) {

		error = NULL;
		handle = rsvg_handle_new_from_file (file, &error);
		if (error) {
			g_warning (error->message);
			exit_code = EXIT_FAILURE;
			goto bail2;
		}

		if (fragment && handle) {
			gboolean have_fragment;
			have_fragment = rsvg_handle_get_dimensions_sub (handle,
						&dimensions, fragment);
			if (!have_fragment) {
				g_warning ("%s: fragment `'%s' not found.",
						file, fragment);
				exit_code = EXIT_FAILURE;
				goto bail3;
			}

			printf ("%s, fragment `%s': %dx%d, em=%f, ex=%f\n",
					file, fragment,
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
			goto bail2;
		}

		g_object_unref (G_OBJECT (handle)), handle = NULL;
	}

	exit_code = EXIT_SUCCESS;

bail3:
	if (handle)
		g_object_unref (G_OBJECT (handle)), handle = NULL;
bail2:
	if (context)
		g_option_context_free (context), context = NULL;
	if (error)
		g_error_free (error), error = NULL;
bail1:
	rsvg_term ();
	return exit_code;
}

