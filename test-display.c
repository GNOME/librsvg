/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 4; tab-width: 4 -*-
 * test-display: 
 *
 * Copyright (C) 2002-2004 Dom Lachowicz
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
 */

#include "config.h"
#include "rsvg.h"
#include "rsvg-private.h"

#include <stdio.h>
#include <stdlib.h>
#include <popt.h>

#include <gtk/gtk.h>
#include <gdk/gdk.h>

#ifdef ENABLE_XEMBED
#include <gdk/gdkx.h>
#endif

#define DEFAULT_WIDTH  240
#define DEFAULT_HEIGHT 240

static void
quit_cb (GtkWidget *win, gpointer unused)
{
	/* exit the main loop */
	gtk_main_quit();
}

static void 
win_embedded_cb (GtkPlug *plug, gpointer data)
{
}

static void
view_pixbuf (GdkPixbuf * pixbuf, int xid, const char * color)
{
	GtkWidget *win, *img;
	gint width, height;
	GdkColor bg_color;

	/* create toplevel window and set its title */

	if(xid > 0)
		{
			GdkWindow *gdk_parent;

			win = gtk_plug_new(0);
			g_signal_connect(G_OBJECT(win), "embedded",
							 G_CALLBACK(win_embedded_cb), NULL);

			gdk_parent = gdk_window_foreign_new(xid);
			gdk_window_get_geometry(gdk_parent, NULL, NULL, &width, &height, NULL);			
		}
	else
		{
			win = gtk_window_new (GTK_WINDOW_TOPLEVEL);
			width = MIN(gdk_pixbuf_get_width (pixbuf), DEFAULT_WIDTH) + 20;
			height = MIN(gdk_pixbuf_get_height (pixbuf), DEFAULT_HEIGHT) + 20;

			gtk_window_set_title (GTK_WINDOW(win), "SVG Viewer");
		}

	gtk_window_set_default_size(GTK_WINDOW(win), width, height);

	/* exit when 'X' is clicked */
	g_signal_connect(G_OBJECT(win), "destroy", G_CALLBACK(quit_cb), NULL);
	g_signal_connect(G_OBJECT(win), "delete_event", G_CALLBACK(quit_cb), NULL);	

	/* create a new image */
	img = gtk_image_new_from_pixbuf (pixbuf);

	/* pack the window with the image */
	if(xid > 0)
		{
			gtk_container_add(GTK_CONTAINER(win), img);
		}
	else
		{
			GtkWidget *scroll;

			scroll = gtk_scrolled_window_new(NULL, NULL);
			gtk_scrolled_window_add_with_viewport (GTK_SCROLLED_WINDOW(scroll), img);
			gtk_container_add(GTK_CONTAINER(win), scroll);
		}

	if (color && strcmp (color, "none") != 0)
		{
			if (gdk_color_parse (color, &bg_color))
				{
					GtkWidget * parent_widget = gtk_widget_get_parent(img);

					if (gdk_colormap_alloc_color (gtk_widget_get_colormap(parent_widget), &bg_color, FALSE, TRUE))
						gtk_widget_modify_bg (parent_widget, GTK_STATE_NORMAL, &bg_color);
					else
						g_warning ("Couldn't allocate color '%s'", color);
				}
			else
				g_warning ("Couldn't parse color '%s'", color);
		}

	gtk_widget_show_all (win);

#ifdef ENABLE_XEMBED
	if(xid > 0){
		XReparentWindow(GDK_WINDOW_XDISPLAY(win->window),
						GDK_WINDOW_XID(win->window),
						xid, 0, 0);
		XMapWindow(GDK_WINDOW_XDISPLAY(win->window),
				   GDK_WINDOW_XID(win->window));
	}
#endif
}

int 
main (int argc, char **argv)
{
	poptContext popt_context;
	double x_zoom = 1.0;
	double y_zoom = 1.0;
	double dpi = -1.0;
	int width  = -1;
	int height = -1;
	int bVersion = 0;
	char * bg_color = NULL;

	int xid = -1;
	int from_stdin = 0;

	struct RsvgSizeCallbackData size_data;

	struct poptOption options_table[] = {
#ifdef ENABLE_XEMBED
		{ "xid",      'i',  POPT_ARG_INT,    &xid,        0, "XWindow ID [for X11 embedding]", "<int>" },
#endif
		{ "stdin",    's',  POPT_ARG_NONE,   &from_stdin, 0, "Use stdin", NULL },
		{ "dpi",      'd',  POPT_ARG_DOUBLE, &dpi,        0, "Pixels Per Inch", "<float>" },
		{ "x-zoom",   'x',  POPT_ARG_DOUBLE, &x_zoom,     0, "x zoom factor", "<float>" },
		{ "y-zoom",   'y',  POPT_ARG_DOUBLE, &y_zoom,     0, "y zoom factor", "<float>" },
		{ "width",    'w',  POPT_ARG_INT,    &width,      0, "width", "<int>" },
		{ "height",   'h',  POPT_ARG_INT,    &height,     0, "height", "<int>" },
		{ "bg-color", 'b',  POPT_ARG_STRING, &bg_color,   0, "color", "<string>" },
		{ "version",  'v',  POPT_ARG_NONE,   &bVersion,   0, "show version information", NULL },
		POPT_AUTOHELP
		POPT_TABLEEND
	};
	int c;
	const char * const *args;
	gint n_args = 0;
	GdkPixbuf *pixbuf;
    
	popt_context = poptGetContext ("rsvg-view", argc, (const char **)argv, options_table, 0);
	poptSetOtherOptionHelp(popt_context, "[OPTIONS...] [file.svg]");
	
	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);
	
	if (bVersion != 0)
		{
			g_message ("rsvg-view version %s\n", VERSION);
			return 0;
		}
	
	if (args)
		{
			while (args[n_args] != NULL)
				n_args++;
		}
  
	if ((!from_stdin) && (n_args != 1))
		{
			poptPrintHelp (popt_context, stderr, 0);
			poptFreeContext (popt_context);
			return 1;
		}
	
	/* initialize gtk+ */
	gtk_init (&argc, &argv) ;

	if (dpi > 0.)
		rsvg_set_default_dpi (dpi);
	
	/* if both are unspecified, assume user wants to zoom the pixbuf in at least 1 dimension */
	if (width == -1 && height == -1)
		{
			size_data.type = RSVG_SIZE_ZOOM;
			size_data.x_zoom = x_zoom;
			size_data.y_zoom = y_zoom;
		}
	/* if both are unspecified, assume user wants to resize pixbuf in at least 1 dimension */
	else if (x_zoom == 1.0 && y_zoom == 1.0)
		{
			size_data.type = RSVG_SIZE_WH;
			size_data.width = width;
			size_data.height = height;
		}
	/* assume the user wants to zoom the pixbuf, but cap the maximum size */
	else
		{
			size_data.type = RSVG_SIZE_ZOOM_MAX;
			size_data.x_zoom = x_zoom;
			size_data.y_zoom = y_zoom;
			size_data.width = width;
			size_data.height = height;
		}
	
	if(from_stdin)
		pixbuf = rsvg_pixbuf_from_stdio_file_with_size_data (stdin, &size_data, NULL);
	else
		pixbuf = rsvg_pixbuf_from_file_with_size_data (args[0], &size_data, NULL);

	poptFreeContext (popt_context);

	if (!pixbuf)
		{
			g_critical ("Error displaying pixbuf!\n");
			return 1;
		}
	
	view_pixbuf (pixbuf, xid, bg_color);
	
	/* run the gtk+ main loop */
	gtk_main ();
	
	g_object_unref (G_OBJECT(pixbuf));
	
	return 0;
}
