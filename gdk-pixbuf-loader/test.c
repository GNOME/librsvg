/*
   Copyright © 2011 Christian Persch

   This program is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public License as
   published by the Free Software Foundation; either version 2.1 of the
   License, or (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with this program; if not, write to the
   Free Software Foundation, Inc., 59 Temple Place - Suite 330,
   Boston, MA 02111-1307, USA.
*/

#include "config.h"

#include <locale.h>

#include <glib.h>
#include <gdk-pixbuf/gdk-pixbuf.h>

int
main (int argc, char **argv)
{
    GOptionContext *context;
    int width = -1;
    int height = -1;
    char **args = NULL;
    GdkPixbuf *pixbuf = NULL;
    GError *error = NULL;
    int ret = 1;

    GOptionEntry options_table[] = {
        { "width", 'w', 0, G_OPTION_ARG_INT, &width,
          "width [optional; defaults to the SVG's width]", "WIDTH" },
        { "height", 'h', 0, G_OPTION_ARG_INT, &height,
          "height [optional; defaults to the SVG's height]", "HEIGHT" },
        { G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, "INPUT-FILE OUTPUT-FILE" },
        { NULL }
    };

    setlocale(LC_ALL, "");

    /* Use the locally built rsvg loader, not the system one */
    g_setenv ("GDK_PIXBUF_MODULE_FILE", "./gdk-pixbuf.loaders", TRUE);

    g_type_init ();

    context = g_option_context_new ("- Pixbuf Test Loader");
    g_option_context_add_main_entries (context, options_table, NULL);
    g_option_context_parse (context, &argc, &argv, &error);
    g_option_context_free (context);
    if (error)
      goto done;

    if (args == NULL || g_strv_length (args) != 2) {
        g_printerr ("Need to specify input and output filenames\n");
        goto done;
    }

    pixbuf = gdk_pixbuf_new_from_file_at_size (args[0], width, height, &error);
    if (pixbuf == NULL)
      goto done;

    if (!gdk_pixbuf_save (pixbuf, args[1], "png", &error, NULL))
      goto done;

    /* Success! */
    ret = 0;

  done:

    if (error) {
      g_printerr ("Error: %s\n", error->message);
      g_error_free (error);
    }

    if (pixbuf)
      g_object_unref (pixbuf);

    g_strfreev (args);

    return ret;
}
