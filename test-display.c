/*
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
 * To compile:
 *   gcc `pkg-config --cflags --libs gtk+-2.0 librsvg-2.0` test-display.c
 */

#include <librsvg/rsvg.h>
#include <gtk/gtk.h>

static void
quit_cb (GtkWidget *win, gpointer unused)
{
  /* exit the main loop */
  gtk_main_quit();
}

int 
main (int argc, char **argv)
{
  GtkWidget *win, *img;
  GdkPixbuf *pixbuf;
  GError *error;

  /* initialize gtk+ */
  gtk_init (&argc, &argv) ;

  if (argc != 2)
    {
      g_print ("Usage: %s <svg>\n", argv[0]);
      exit (1);
    }

  /* create a pixbuf */
  pixbuf = rsvg_pixbuf_from_file (argv[1], &error) ;
  if (!pixbuf)
    {
      g_print ("Couldn't render %s\n", argv[1]);
      exit (1);
    }

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

  /* run the gtk+ main loop */
  gtk_main ();

  return 0;
}
