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
#include <gdk/gdkkeysyms.h>

#ifdef ENABLE_XEMBED
#include <gdk/gdkx.h>
#endif /* ENABLE_XEMBED */

#define DEFAULT_WIDTH  240
#define DEFAULT_HEIGHT 240

typedef struct _ViewerCbInfo ViewerCbInfo;
struct _ViewerCbInfo
{
	GtkWidget  * window;
	GtkWidget  * popup_menu;
	GdkPixbuf  * pixbuf;
	GByteArray * svg_bytes;
	GtkAccelGroup * accel_group;
	char * base_uri;
};

static void
rsvg_window_set_default_icon (GtkWindow * window, GdkPixbuf *icon)
{
  GList *list;
  
  list = g_list_prepend (NULL, icon);
  gtk_window_set_icon_list (window, list);
  g_list_free (list);
}

#ifdef HAVE_GNOME_PRINT

#include <libgnomeprint/gnome-print.h>
#include <libgnomeprint/gnome-print-job.h>
#include <libgnomeprintui/gnome-print-dialog.h>
#include <libgnomeprintui/gnome-print-job-preview.h>

static void 
print_pixbuf (GObject * ignored, gpointer user_data)
{
	ViewerCbInfo * info = (ViewerCbInfo *)user_data;
	GtkWidget *gpd;	
	gint result;

	gpd = gnome_print_dialog_new (gnome_print_job_new (gnome_print_config_default ()), _("Print SVG"), 0);
	gtk_window_set_transient_for(GTK_WINDOW (gpd), GTK_WINDOW (info->window));
			  
	if ((result = gtk_dialog_run (GTK_DIALOG (gpd))) != GNOME_PRINT_DIALOG_RESPONSE_CANCEL) 
		{
			GnomePrintJob *gpm;
			GnomePrintContext * gpc;
			GdkPixbuf * image;
			
			gint width, height, rowstride;
			const guchar * pixels;
			
			gdouble page_width, page_height;
			
			gpm = gnome_print_job_new (gnome_print_dialog_get_config (GNOME_PRINT_DIALOG(gpd)));
			gpc = gnome_print_job_get_context (gpm);
			
			gnome_print_config_get_page_size (gnome_print_job_get_config (gpm), &page_width, &page_height);		
			image = info->pixbuf;
			
			width     = gdk_pixbuf_get_width (image);
			height    = gdk_pixbuf_get_height (image);
			
			if (width > page_width ||
				height > page_height) 
				{
					struct RsvgSizeCallbackData size_data;

					/* scale down the image to the page's size, while preserving the aspect ratio */

					if ((double)height * (double)page_width >
						(double)width * (double)page_height) 
						{
							width = 0.5 + (double)width * (double)page_height / (double)height;
							height = page_height;
						} 
					else 
						{
							height = 0.5 + (double)height * (double)page_width / (double)width;
							width = page_width;
						}

					size_data.type = RSVG_SIZE_WH;
					size_data.width = width;
					size_data.height = height;
					size_data.keep_aspect_ratio = FALSE;

					image = rsvg_pixbuf_from_data_with_size_data (info->svg_bytes->data, info->svg_bytes->len, &size_data, info->base_uri, NULL);
				} 
			else 
				g_object_ref (G_OBJECT (image));
			
			rowstride = gdk_pixbuf_get_rowstride (image);
			pixels    = gdk_pixbuf_get_pixels (image);
			
			gnome_print_beginpage(gpc, "1");
			gnome_print_gsave (gpc);
			gnome_print_translate (gpc, 0, page_height - height);
			gnome_print_scale (gpc, width, height);
			gnome_print_moveto (gpc, 0, 0);
			
			gnome_print_rgbaimage (gpc, pixels, width, height, rowstride);
			
			gnome_print_grestore (gpc);
			gnome_print_showpage (gpc);
			gnome_print_job_close (gpm);
			
			if(result == GNOME_PRINT_DIALOG_RESPONSE_PRINT)
				gnome_print_job_print (gpm);
			else
				{
					GtkWidget * preview;
					
					preview = gnome_print_job_preview_new (gpm, _("SVG Print Preview"));
					gtk_window_set_transient_for(GTK_WINDOW(preview), GTK_WINDOW(info->window));
					gtk_widget_show (GTK_WIDGET (preview));
				}
			
			g_object_unref (G_OBJECT (gpm));
			g_object_unref (G_OBJECT (image));
		}
	
	gtk_widget_destroy (gpd);
}

#endif /* HAVE_GNOME_PRINT */

static void 
save_pixbuf (GObject * ignored, gpointer user_data)
{
	GtkWidget * filesel;
	ViewerCbInfo * info = (ViewerCbInfo *)user_data;
	
	filesel = gtk_file_selection_new (_("Save SVG as PNG"));
	gtk_window_set_transient_for(GTK_WINDOW(filesel), GTK_WINDOW(info->window));
	
	if (gtk_dialog_run (GTK_DIALOG (filesel)) == GTK_RESPONSE_OK)
		{
			const char * filename;
			
			filename = gtk_file_selection_get_filename (GTK_FILE_SELECTION (filesel));
			
			if (filename) 
				{
					GError * err = NULL;
					
					if (!gdk_pixbuf_save (info->pixbuf, filename, "png", &err, NULL)) 
						{
							if (err) 
								{
									GtkWidget * errmsg;
									
									errmsg = gtk_message_dialog_new (GTK_WINDOW(filesel),
																	 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
																	 GTK_MESSAGE_WARNING,
																	 GTK_BUTTONS_CLOSE,
																	 err->message);
									gtk_window_set_transient_for(GTK_WINDOW(errmsg), GTK_WINDOW(info->window));
									
									gtk_dialog_run (GTK_DIALOG (errmsg));
									
									g_error_free (err);
									gtk_widget_destroy (errmsg);
								}
						}
				} 
			else 
				{
					GtkWidget * errmsg;
					
					errmsg = gtk_message_dialog_new (GTK_WINDOW(filesel),
													 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
													 GTK_MESSAGE_WARNING,
													 GTK_BUTTONS_CLOSE,
													 _("No filename given"));
					gtk_window_set_transient_for(GTK_WINDOW(errmsg), GTK_WINDOW(info->window));
					
					gtk_dialog_run (GTK_DIALOG (errmsg));
					gtk_widget_destroy (errmsg);
				}
		}
	
	gtk_widget_destroy (filesel);
}

static void 
save_svg (GObject * ignored, gpointer user_data)
{
	GtkWidget * filesel;
	ViewerCbInfo * info = (ViewerCbInfo *)user_data;
	
	filesel = gtk_file_selection_new (_("Save SVG"));
	gtk_window_set_transient_for(GTK_WINDOW(filesel), GTK_WINDOW(info->window));

	if (gtk_dialog_run (GTK_DIALOG (filesel)) == GTK_RESPONSE_OK)
		{
			const char * filename;
			
			filename = gtk_file_selection_get_filename (GTK_FILE_SELECTION (filesel));
			
			if (filename) 
				{
					FILE * fp;

					fp = fopen(filename, "wb");
					if (!fp) 
						{
							GtkWidget * errmsg;
							
							errmsg = gtk_message_dialog_new (GTK_WINDOW(filesel),
															 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
															 GTK_MESSAGE_WARNING,
															 GTK_BUTTONS_CLOSE,
															 _("Couldn't save %s"),
															 filename);
							gtk_window_set_transient_for(GTK_WINDOW(errmsg), GTK_WINDOW(info->window));
									
							gtk_dialog_run (GTK_DIALOG (errmsg));
							gtk_widget_destroy (errmsg);
						}
					else
						{
							size_t written = 0, remaining = info->svg_bytes->len;
							const char * buffer = info->svg_bytes->data;

							while (remaining > 0) {
								written = fwrite (buffer + (info->svg_bytes->len - remaining), 1, 
												  remaining, fp);
								if ((written < remaining) && ferror (fp) != 0)
									{
										GtkWidget * errmsg;
										
										errmsg = gtk_message_dialog_new (GTK_WINDOW(filesel),
																		 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
																		 GTK_MESSAGE_WARNING,
																		 GTK_BUTTONS_CLOSE,
																		 _("Couldn't save %s"),
																		 filename);
										gtk_window_set_transient_for(GTK_WINDOW(errmsg), GTK_WINDOW(info->window));
										
										gtk_dialog_run (GTK_DIALOG (errmsg));
										gtk_widget_destroy (errmsg);

										break;
									}
								
								remaining -= written;
							}

							fclose(fp);
						}
				} 
			else 
				{
					GtkWidget * errmsg;
					
					errmsg = gtk_message_dialog_new (GTK_WINDOW(filesel),
													 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
													 GTK_MESSAGE_WARNING,
													 GTK_BUTTONS_CLOSE,
													 _("No filename given"));
					gtk_window_set_transient_for(GTK_WINDOW(errmsg), GTK_WINDOW(info->window));
					
					gtk_dialog_run (GTK_DIALOG (errmsg));
					gtk_widget_destroy (errmsg);
				}
		}
	
	gtk_widget_destroy (filesel);
}

static void
create_popup_menu (ViewerCbInfo * info)
{
	GtkWidget * popup_menu;
	GtkWidget * menu_item;
	GtkWidget * stock;

	popup_menu = gtk_menu_new ();
	gtk_menu_set_accel_group (GTK_MENU (popup_menu), info->accel_group);

	menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_SAVE, NULL);
	g_signal_connect (menu_item, "activate",
					  G_CALLBACK (save_svg), info);
	gtk_widget_show (menu_item);
	gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
	gtk_widget_add_accelerator(menu_item, "activate", info->accel_group, GDK_S, GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);

	menu_item = gtk_image_menu_item_new_with_label (_("Save as PNG"));
	stock = gtk_image_new_from_stock (GTK_STOCK_SAVE_AS, GTK_ICON_SIZE_MENU);
	gtk_widget_show (stock);
	gtk_image_menu_item_set_image (GTK_IMAGE_MENU_ITEM (menu_item), stock);
	g_signal_connect (menu_item, "activate",
					  G_CALLBACK (save_pixbuf), info);
	gtk_widget_show (menu_item);
	gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
	gtk_widget_add_accelerator(menu_item, "activate", info->accel_group, GDK_S, GDK_CONTROL_MASK | GDK_SHIFT_MASK, GTK_ACCEL_VISIBLE);

#ifdef HAVE_GNOME_PRINT
	menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_PRINT, NULL);
	g_signal_connect (menu_item, "activate",
					  G_CALLBACK (print_pixbuf), info);
	gtk_widget_show (menu_item);
	gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
	gtk_widget_add_accelerator(menu_item, "activate", info->accel_group, GDK_P, GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);
#endif

	info->popup_menu = popup_menu;
}

static gint
button_press_event (GObject        *widget,
					GdkEventButton *event,
					gpointer        user_data)
{
	if (event->button == 3 && event->type == GDK_BUTTON_PRESS)
		{
			ViewerCbInfo * info = (ViewerCbInfo *)user_data;

			gtk_menu_popup (GTK_MENU (info->popup_menu), NULL, NULL,
							NULL, NULL, event->button, event->time);
			return TRUE;
		}
	
	return FALSE;
}

static void
quit_cb (GtkWidget *win, gpointer unused)
{
	/* exit the main loop */
	gtk_main_quit();
}

static void
view_pixbuf (ViewerCbInfo * info, int xid, const char * color)
{
	GtkWidget *win, *img;
	gint width, height;
	GdkColor bg_color;

	/* create toplevel window and set its title */

	if(xid > 0)
		{
			GdkWindow *gdk_parent;

			win = gtk_plug_new(0);

			gdk_parent = gdk_window_foreign_new(xid);
			gdk_window_get_geometry(gdk_parent, NULL, NULL, &width, &height, NULL);

			/* so that button presses get registered */
			gtk_widget_add_events (win, GDK_BUTTON_PRESS_MASK | GDK_BUTTON_RELEASE_MASK);
		}
	else
		{
			win = gtk_window_new (GTK_WINDOW_TOPLEVEL);
			width = MIN(gdk_pixbuf_get_width (info->pixbuf), DEFAULT_WIDTH) + 20;
			height = MIN(gdk_pixbuf_get_height (info->pixbuf), DEFAULT_HEIGHT) + 20;

			gtk_window_set_title (GTK_WINDOW(win), _("SVG Viewer"));
			rsvg_window_set_default_icon (GTK_WINDOW(win), info->pixbuf);
		}

	gtk_window_set_default_size(GTK_WINDOW(win), width, height);

	/* exit when 'X' is clicked */
	g_signal_connect(G_OBJECT(win), "destroy", G_CALLBACK(quit_cb), NULL);
	g_signal_connect(G_OBJECT(win), "delete_event", G_CALLBACK(quit_cb), NULL);	

	/* create a new image */
	img = gtk_image_new_from_pixbuf (info->pixbuf);

	/* pack the window with the image */
	if(xid > 0)
		gtk_container_add(GTK_CONTAINER(win), img);
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
						g_warning (_("Couldn't allocate color '%s'"), color);
				}
			else
				g_warning (_("Couldn't parse color '%s'"), color);
		}

	create_popup_menu (info);

	info->window = win;
	gtk_window_add_accel_group (GTK_WINDOW (win), info->accel_group);

	g_signal_connect (G_OBJECT (win), "button-press-event",
					  G_CALLBACK (button_press_event), info);
	
	gtk_widget_show_all (win);

#ifdef ENABLE_XEMBED
	if(xid > 0)
		{
			XReparentWindow (GDK_WINDOW_XDISPLAY( win->window),
							 GDK_WINDOW_XID (win->window),
							 xid, 0, 0);
			XMapWindow (GDK_WINDOW_XDISPLAY (win->window),
						GDK_WINDOW_XID (win->window));
		}
#endif
}

int 
main (int argc, char **argv)
{
	FILE * in;
	GError * err = NULL;
	poptContext popt_context;
	double x_zoom = 1.0;
	double y_zoom = 1.0;
	double dpi = -1.0;
	int width  = -1;
	int height = -1;
	int bVersion = 0;
	char * bg_color = NULL;
	char * base_uri = NULL;
	int bKeepAspect = 0;

	int xid = -1;
	int from_stdin = 0;
	ViewerCbInfo info;

	struct RsvgSizeCallbackData size_data;

	struct poptOption options_table[] = 
		{
#ifdef ENABLE_XEMBED
			{ "xid",         'i',  POPT_ARG_INT,    &xid,         0, _("XWindow ID [for X11 embedding]"), _("<int>") },
#endif
			{ "stdin",       's',  POPT_ARG_NONE,   &from_stdin,  0, _("Read from stdin instead of a file"), NULL },
			{ "dpi",         'd',  POPT_ARG_DOUBLE, &dpi,         0, _("Set the # of Pixels Per Inch"), _("<float>") },
			{ "x-zoom",      'x',  POPT_ARG_DOUBLE, &x_zoom,      0, _("Set the x zoom factor"), _("<float>") },
			{ "y-zoom",      'y',  POPT_ARG_DOUBLE, &y_zoom,      0, _("Set the y zoom factor"), _("<float>") },
			{ "width",       'w',  POPT_ARG_INT,    &width,       0, _("Set the image's width"), _("<int>") },
			{ "height",      'h',  POPT_ARG_INT,    &height,      0, _("Set the image's height"), _("<int>") },
			{ "bg-color",    'b',  POPT_ARG_STRING, &bg_color,    0, _("Set the image background color (default: transparent)"), _("<string>") },
			{ "base-uri",    'u',  POPT_ARG_STRING, &base_uri,    0, _("Set the base URI (default: none)"), _("<string>") },
			{ "keep-aspect", 'k',  POPT_ARG_NONE,   &bKeepAspect, 0, _("Preserve the image's aspect ratio"), NULL },
			{ "version",     'v',  POPT_ARG_NONE,   &bVersion,    0, _("Show version information"), NULL },
			POPT_AUTOHELP
			POPT_TABLEEND
		};
	int c;
	const char * const *args;
	gint n_args = 0;
    
	info.pixbuf = NULL;
	info.svg_bytes = NULL;
	info.window = NULL;
	info.popup_menu = NULL;
	info.base_uri = base_uri;

	popt_context = poptGetContext ("rsvg-view", argc, (const char **)argv, options_table, 0);
	poptSetOtherOptionHelp (popt_context, _("[OPTIONS...] [file.svg]"));
	
	c = poptGetNextOpt (popt_context);
	args = poptGetArgs (popt_context);
	
	if (bVersion != 0)
		{
			g_message (_("rsvg-view version %s\n"), VERSION);
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

	size_data.keep_aspect_ratio = bKeepAspect;
	
	if(from_stdin)
		in = stdin;
	else {
		in = fopen (args[0], "rb");
		/* base_uri = args[0]; */
	}

	if(!in)
		{
			g_critical (_("Couldn't open %s\n"), args[0]);
			poptFreeContext (popt_context);

			return 1;
		}

	info.svg_bytes = g_byte_array_new ();

	for (;;)
		{
			char buf[1024 * 8];
			size_t nread = fread (buf, 1, sizeof(buf), in);
			
			if (nread > 0)
				g_byte_array_append (info.svg_bytes, buf, nread);
			
			if (nread < sizeof (buf))
				{
					if (ferror (in))
						{
							g_critical (_("Error reading\n"));
							g_byte_array_free (info.svg_bytes, TRUE);
							poptFreeContext (popt_context);
							fclose(in);
							
							return 1;
						}
					else if (feof(in))
						break;
				}
		}
	
	fclose(in);

	poptFreeContext (popt_context);

	info.pixbuf = rsvg_pixbuf_from_data_with_size_data (info.svg_bytes->data, info.svg_bytes->len, &size_data, base_uri, &err);

	if (!info.pixbuf)
		{
			g_critical (_("Error displaying pixbuf!\n"));

			if (err)
				{
					g_critical (err->message);
					g_error_free (err);
				}

			return 1;
		}

	info.accel_group = gtk_accel_group_new ();

	view_pixbuf (&info, xid, bg_color);

	/* run the gtk+ main loop */
	gtk_main ();	

	g_object_unref (G_OBJECT (info.pixbuf));
	g_byte_array_free (info.svg_bytes, TRUE);

	return 0;
}
