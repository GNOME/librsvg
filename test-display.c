/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 4 -*-
 * test-display: 
 *
 * Copyright (C) 2002 Dom Lachowicz
 *
 * This program is released into the PUBLIC DOMAIN, and is meant to be a
 * useful example if how to draw a SVG image inside of a GtkWidget. This 
 * program is free software; you can redistribute it and/or modify it at your
 * will.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE
 *
 * Simple utility to view a SVG file inside of a GtkWindow
 *
 * To compile, basically:
 *   gcc `pkg-config --cflags --libs gtk+-2.0 librsvg-2.0` -lpopt -o svg-display test-display.c
 */

#include "config.h"
#include "rsvg.h"

#include <stdio.h>
#include <stdlib.h>
#include <popt.h>

#include <gtk/gtk.h>

static void
quit_cb (GtkWidget *win, gpointer unused)
{
  /* exit the main loop */
  gtk_main_quit();
}

static void
view_pixbuf (GdkPixbuf * pixbuf)
{
  GtkWidget *win, *img;

  /* create toplevel window and set its title */
  win = gtk_window_new (GTK_WINDOW_TOPLEVEL);
  gtk_window_set_title (GTK_WINDOW(win), "SVG Viewer");

  /* exit when 'X' is clicked */
  g_signal_connect(G_OBJECT(win), "destroy", G_CALLBACK(quit_cb), NULL);

  /* create a new image */
  img = gtk_image_new_from_pixbuf (pixbuf);

  /* pack the window with the image */
  gtk_container_add(GTK_CONTAINER(win), img);
  gtk_widget_show_all (win);
}

int 
main (int argc, char **argv)
{
  poptContext popt_context;
  double x_zoom = 1.0;
  double y_zoom = 1.0;
  int width  = -1;
  int height = -1;
  int bVersion = 0;
  
  struct poptOption options_table[] = {
    { "x-zoom", 'x',  POPT_ARG_DOUBLE, &x_zoom,  0, "x zoom factor", "<float>" },
    { "y-zoom", 'y',  POPT_ARG_DOUBLE, &y_zoom,  0, "y zoom factor", "<float>" },
    { "width",  'w',  POPT_ARG_INT,    &width,   0, "width", "<int>" },
    { "height", 'h',  POPT_ARG_INT,    &height,  0, "height", "<int>" },
    { "version", 'v', POPT_ARG_NONE,   &bVersion, 0, "show version information", NULL },
    POPT_AUTOHELP
    POPT_TABLEEND
  };
  int c;
  const char * const *args;
  gint n_args = 0;
  GdkPixbuf *pixbuf;
    
  popt_context = poptGetContext ("svg-display", argc, (const char **)argv, options_table, 0);
  poptSetOtherOptionHelp(popt_context, "[OPTIONS...] file.svg");
  
  c = poptGetNextOpt (popt_context);
  args = poptGetArgs (popt_context);
  
  if (bVersion != 0)
    {
      printf ("svg-display version %s\n", VERSION);
      return 0;
    }
  
  if (args)
    {
      while (args[n_args] != NULL)
	n_args++;
    }
  
  if (n_args != 1)
    {
      poptPrintHelp (popt_context, stderr, 0);
      poptFreeContext (popt_context);
      return 1;
    }

  /* initialize gtk+ */
  gtk_init (&argc, &argv) ;

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
  
  poptFreeContext (popt_context);

  if (!pixbuf)
	  {
		  fprintf (stderr, "Error displaying pixbuf!\n");
		  return 1;
	  }

  view_pixbuf (pixbuf);

  /* run the gtk+ main loop */
  gtk_main ();

  g_object_unref (G_OBJECT(pixbuf));

  return 0;
}
