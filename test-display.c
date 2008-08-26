/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
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
#include "rsvg-cairo.h"
#include "rsvg-private.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <gtk/gtk.h>
#include <gdk/gdk.h>
#include <gdk/gdkkeysyms.h>

#ifdef HAVE_BASENAME
#include <libgen.h>
#endif

#ifdef ENABLE_XEMBED
#include <gdk/gdkx.h>
#endif                          /* ENABLE_XEMBED */

#define DEFAULT_WIDTH  640
#define DEFAULT_HEIGHT 480

static char *
_rsvg_basename (const char *file)
{
#ifdef HAVE_BASENAME
    if (file && *file) {
        char *file_dup = g_strdup (file);
        return basename (file_dup);
    }
#endif

    return NULL;
}

static GdkPixbuf *
pixbuf_from_data_with_size_data (const guchar * buff,
                                 size_t len,
                                 struct RsvgSizeCallbackData *data,
                                 const char *base_uri,
                                 const char *id,
                                 GError ** error)
{
    RsvgHandle *handle;
    GdkPixbuf *retval;

    handle = rsvg_handle_new ();

    if (!handle) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error creating SVG reader"));
        return NULL;
    }

    rsvg_handle_set_size_callback (handle, _rsvg_size_callback, data, NULL);
    rsvg_handle_set_base_uri (handle, base_uri);

    if (!rsvg_handle_write (handle, buff, len, error)) {
        g_object_unref (G_OBJECT (handle));
        return NULL;
    }

    if (!rsvg_handle_close (handle, error)) {
        g_object_unref (G_OBJECT (handle));
        return NULL;
    }

    retval = rsvg_handle_get_pixbuf_sub (handle, id);
    g_object_unref (G_OBJECT (handle));

    return retval;
}

typedef struct _ViewerCbInfo ViewerCbInfo;
struct _ViewerCbInfo {
    GtkWidget *window;
    GtkWidget *popup_menu;
    GtkWidget *image;           /* the image widget */

    GdkPixbuf *pixbuf;
    GByteArray *svg_bytes;
    GtkAccelGroup *accel_group;
    char *base_uri;
    char *id;
};

static void
zoom_image (ViewerCbInfo * info, gint width, gint height)
{
    struct RsvgSizeCallbackData size_data;
    GdkPixbuf *save_pixbuf = info->pixbuf;

    size_data.type = RSVG_SIZE_WH;
    size_data.width = width;
    size_data.height = height;
    size_data.keep_aspect_ratio = FALSE;

    info->pixbuf =
        pixbuf_from_data_with_size_data (info->svg_bytes->data, info->svg_bytes->len,
                                         &size_data, info->base_uri, info->id, NULL);
    gtk_image_set_from_pixbuf (GTK_IMAGE (info->image), info->pixbuf);

    if (save_pixbuf)
        g_object_unref (G_OBJECT (save_pixbuf));
}

static void
zoom_in (GObject * ignored, ViewerCbInfo * info)
{
    if (!info->pixbuf)
        return;
    zoom_image (info, gdk_pixbuf_get_width (info->pixbuf) * 1.25,
                gdk_pixbuf_get_height (info->pixbuf) * 1.25);
}

static void
zoom_out (GObject * ignored, ViewerCbInfo * info)
{
    if (!info->pixbuf)
        return;
    zoom_image (info, gdk_pixbuf_get_width (info->pixbuf) / 1.25,
                gdk_pixbuf_get_height (info->pixbuf) / 1.25);
}

static void
rsvg_window_set_default_icon (GtkWindow * window, GdkPixbuf * src)
{
    GList *list;
    GdkPixbuf *icon;
    gint width, height;

    width = gdk_pixbuf_get_width (src);
    height = gdk_pixbuf_get_height (src);

    if (width > 128 || height > 128) {
        /* sending images greater than 128x128 has this nasty tendency to 
           cause broken pipe errors X11 Servers */
        if (width > height) {
            width = 0.5 + width * 128. / height;
            height = 128;
        } else {
            height = 0.5 + height * 128. / width;
            width = 128;
        }

        icon = gdk_pixbuf_scale_simple (src, width, height, GDK_INTERP_BILINEAR);
    } else {
        icon = g_object_ref (G_OBJECT (src));
    }

    list = g_list_prepend (NULL, icon);
    gtk_window_set_icon_list (window, list);
    g_list_free (list);

    g_object_unref (G_OBJECT (icon));
}

#if GTK_CHECK_VERSION(2,10,0)

static void
begin_print (GtkPrintOperation *operation,
			 GtkPrintContext   *context,
			 gpointer           user_data)
{
	gtk_print_operation_set_n_pages(operation, 1);
}

static void
draw_page (GtkPrintOperation *operation,
		   GtkPrintContext   *context,
		   gint               page_nr,
		   gpointer           user_data)
{
    ViewerCbInfo *info = (ViewerCbInfo *) user_data;

	cairo_t *cr;
	gdouble page_width, page_height;
  
	cr = gtk_print_context_get_cairo_context (context);
	page_width = gtk_print_context_get_width (context);
	page_height = gtk_print_context_get_height (context);

	{
		RsvgHandle *handle;
		RsvgDimensionData svg_dimensions;
		struct RsvgSizeCallbackData size_data;

		/* should not fail */
		handle = rsvg_handle_new_from_data(info->svg_bytes->data, info->svg_bytes->len, NULL);
		rsvg_handle_set_base_uri (handle, info->base_uri);
		rsvg_handle_set_dpi_x_y (handle, gtk_print_context_get_dpi_x(context), 
								 gtk_print_context_get_dpi_y(context));
		rsvg_handle_get_dimensions(handle, &svg_dimensions);

        if (svg_dimensions.width > page_width || svg_dimensions.height > page_height) {
            /* scale down the image to the page's size, while preserving the aspect ratio */

            if ((double) svg_dimensions.height * (double) page_width > (double) svg_dimensions.width * (double) page_height) {
                svg_dimensions.width = 0.5 + (double) svg_dimensions.width *(double) page_height / (double) svg_dimensions.height;
                svg_dimensions.height = page_height;
            } else {
                svg_dimensions.height = 0.5 + (double) svg_dimensions.height *(double) page_width / (double) svg_dimensions.width;
                svg_dimensions.width = page_width;
            }
        }

		size_data.type = RSVG_SIZE_WH;
		size_data.width = svg_dimensions.width;
		size_data.height = svg_dimensions.height;
		size_data.keep_aspect_ratio = FALSE;
		rsvg_handle_set_size_callback (handle, _rsvg_size_callback, &size_data, NULL);

		rsvg_handle_render_cairo(handle, cr);

		g_object_unref (handle);
	}
}

static void
print_pixbuf (GObject * ignored, gpointer user_data)
{
  GtkPrintOperation *print;
  GtkPrintOperationResult res;
  ViewerCbInfo *info = (ViewerCbInfo *) user_data;

  print = gtk_print_operation_new ();

  g_signal_connect (print, "begin_print", G_CALLBACK (begin_print), info);
  g_signal_connect (print, "draw_page", G_CALLBACK (draw_page), info);

  res = gtk_print_operation_run (print, GTK_PRINT_OPERATION_ACTION_PRINT_DIALOG,
                                 GTK_WINDOW (info->window), NULL);

  g_object_unref (print);
}

#endif                          /* HAVE_GNOME_PRINT */

static char *
save_file (const char *title, const char *suggested_filename, GtkWidget * parent, int *success)
{
    GtkWidget *dialog;
    char *filename = NULL;

    *success = 0;
    dialog = gtk_file_chooser_dialog_new (title,
                                          GTK_WINDOW (parent),
                                          GTK_FILE_CHOOSER_ACTION_SAVE,
                                          GTK_STOCK_CANCEL, GTK_RESPONSE_CANCEL,
                                          GTK_STOCK_SAVE, GTK_RESPONSE_ACCEPT, NULL);

    if (suggested_filename && *suggested_filename) {
        gtk_file_chooser_set_current_name (GTK_FILE_CHOOSER (dialog), suggested_filename);
    }

    if (gtk_dialog_run (GTK_DIALOG (dialog)) == GTK_RESPONSE_ACCEPT) {
        filename = gtk_file_chooser_get_filename (GTK_FILE_CHOOSER (dialog));
        *success = 1;
    }

    gtk_widget_destroy (dialog);

    return filename;
}

static void
save_pixbuf (GObject * ignored, gpointer user_data)
{
    ViewerCbInfo *info = (ViewerCbInfo *) user_data;
    char *filename, *base_name, *filename_suggestion;
    int success = 0;

    base_name = _rsvg_basename (info->base_uri);
    if (base_name)
        filename_suggestion = g_strdup_printf ("%s.png", base_name);
    else
        filename_suggestion = NULL;

    filename = save_file (_("Save SVG as PNG"), filename_suggestion, info->window, &success);
    g_free (base_name);
    g_free (filename_suggestion);

    if (filename) {
        GError *err = NULL;

        if (!gdk_pixbuf_save (info->pixbuf, filename, "png", &err, NULL)) {
            if (err) {
                GtkWidget *errmsg;

                errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                                 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
                                                 GTK_MESSAGE_WARNING,
                                                 GTK_BUTTONS_CLOSE, "%s", err->message);

                gtk_dialog_run (GTK_DIALOG (errmsg));

                g_error_free (err);
                gtk_widget_destroy (errmsg);
            }
        }

        g_free (filename);
    } else if (success) {
        GtkWidget *errmsg;

        errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                         GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
                                         GTK_MESSAGE_WARNING,
                                         GTK_BUTTONS_CLOSE, _("No filename given"));
        gtk_window_set_transient_for (GTK_WINDOW (errmsg), GTK_WINDOW (info->window));

        gtk_dialog_run (GTK_DIALOG (errmsg));
        gtk_widget_destroy (errmsg);
    }
}

static void
save_svg (GObject * ignored, gpointer user_data)
{
    ViewerCbInfo *info = (ViewerCbInfo *) user_data;
    char *filename, *base_name;
    int success = 0;

    base_name = _rsvg_basename (info->base_uri);
    filename = save_file (_("Save SVG"), base_name, info->window, &success);
    g_free (base_name);

    if (filename) {
        FILE *fp;

        /* todo: make this support gnome vfs */
        fp = fopen (filename, "wb");
        if (!fp) {
            GtkWidget *errmsg;

            errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                             GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
                                             GTK_MESSAGE_WARNING,
                                             GTK_BUTTONS_CLOSE, _("Couldn't save %s"), filename);
            gtk_window_set_transient_for (GTK_WINDOW (errmsg), GTK_WINDOW (info->window));

            gtk_dialog_run (GTK_DIALOG (errmsg));
            gtk_widget_destroy (errmsg);
        } else {
            size_t written = 0, remaining = info->svg_bytes->len;
            const unsigned char *buffer = info->svg_bytes->data;

            while (remaining > 0) {
                written = fwrite (buffer + (info->svg_bytes->len - remaining), 1, remaining, fp);
                if ((written < remaining) && ferror (fp) != 0) {
                    GtkWidget *errmsg;

                    errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                                     GTK_DIALOG_MODAL |
                                                     GTK_DIALOG_DESTROY_WITH_PARENT,
                                                     GTK_MESSAGE_WARNING, GTK_BUTTONS_CLOSE,
                                                     _("Couldn't save %s"), filename);
                    gtk_window_set_transient_for (GTK_WINDOW (errmsg), GTK_WINDOW (info->window));

                    gtk_dialog_run (GTK_DIALOG (errmsg));
                    gtk_widget_destroy (errmsg);

                    break;
                }

                remaining -= written;
            }

            fclose (fp);
        }

        g_free (filename);
    } else if (success) {
        GtkWidget *errmsg;

        errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                         GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
                                         GTK_MESSAGE_WARNING,
                                         GTK_BUTTONS_CLOSE, _("No filename given"));
        gtk_window_set_transient_for (GTK_WINDOW (errmsg), GTK_WINDOW (info->window));

        gtk_dialog_run (GTK_DIALOG (errmsg));
        gtk_widget_destroy (errmsg);
    }
}

static void
copy_svg_location (GObject * ignored, gpointer user_data)
{
    ViewerCbInfo *info = (ViewerCbInfo *) user_data;
    GtkClipboard *clipboard = NULL;

    if (info->base_uri) {
        clipboard = gtk_clipboard_get (GDK_SELECTION_CLIPBOARD);
        gtk_clipboard_set_text (clipboard, info->base_uri, -1);
    }
}

static void
create_popup_menu (ViewerCbInfo * info)
{
    GtkWidget *popup_menu;
    GtkWidget *menu_item;
    GtkWidget *stock;

    popup_menu = gtk_menu_new ();
    gtk_menu_set_accel_group (GTK_MENU (popup_menu), info->accel_group);

    if (info->base_uri) {
        menu_item = gtk_image_menu_item_new_with_label (_("Copy SVG location"));
        stock = gtk_image_new_from_stock (GTK_STOCK_COPY, GTK_ICON_SIZE_MENU);
        gtk_widget_show (stock);
        gtk_image_menu_item_set_image (GTK_IMAGE_MENU_ITEM (menu_item), stock);
        g_signal_connect (menu_item, "activate", G_CALLBACK (copy_svg_location), info);
        gtk_widget_show (menu_item);
        gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
        gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_C,
                                    GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);
    }

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_SAVE, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (save_svg), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_S, GDK_CONTROL_MASK,
                                GTK_ACCEL_VISIBLE);

    menu_item = gtk_image_menu_item_new_with_label (_("Save as PNG"));
    stock = gtk_image_new_from_stock (GTK_STOCK_SAVE_AS, GTK_ICON_SIZE_MENU);
    gtk_widget_show (stock);
    gtk_image_menu_item_set_image (GTK_IMAGE_MENU_ITEM (menu_item), stock);
    g_signal_connect (menu_item, "activate", G_CALLBACK (save_pixbuf), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_S,
                                GDK_CONTROL_MASK | GDK_SHIFT_MASK, GTK_ACCEL_VISIBLE);

#if GTK_CHECK_VERSION(2,10,0)
    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_PRINT, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (print_pixbuf), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_P, GDK_CONTROL_MASK,
                                GTK_ACCEL_VISIBLE);
#endif

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_ZOOM_IN, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (zoom_in), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_plus,
                                GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_ZOOM_OUT, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (zoom_out), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_minus,
                                GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);

    info->popup_menu = popup_menu;
}

static gint
button_press_event (GObject * widget, GdkEventButton * event, gpointer user_data)
{
    if (event->button == 3 && event->type == GDK_BUTTON_PRESS) {
        ViewerCbInfo *info = (ViewerCbInfo *) user_data;

        gtk_menu_popup (GTK_MENU (info->popup_menu), NULL, NULL,
                        NULL, NULL, event->button, event->time);
        return TRUE;
    }

    return FALSE;
}

static void
quit_cb (GtkWidget * win, gpointer unused)
{
    /* exit the main loop */
    gtk_main_quit ();
}

static void
populate_window (GtkWidget * win, ViewerCbInfo * info, int xid, gint win_width, gint win_height)
{
    GtkWidget *vbox;
    GtkWidget *scroll;
    gint img_width, img_height;

    vbox = gtk_vbox_new (FALSE, 0);
    gtk_container_add (GTK_CONTAINER (win), vbox);

    /* create a new image */
    info->image = gtk_image_new_from_pixbuf (info->pixbuf);

    /* pack the window with the image */
    img_width = gdk_pixbuf_get_width (info->pixbuf);
    img_height = gdk_pixbuf_get_height (info->pixbuf);

    if (xid <= 0) {
		GtkWidget *toolbar;
		GtkToolItem *toolitem;
		GtkRequisition requisition;

        toolbar = gtk_toolbar_new ();
        gtk_box_pack_start (GTK_BOX (vbox), toolbar, FALSE, FALSE, 0);

        toolitem = gtk_tool_button_new_from_stock (GTK_STOCK_ZOOM_IN);
        gtk_toolbar_insert (GTK_TOOLBAR (toolbar), toolitem, 0);
        g_signal_connect (G_OBJECT (toolitem), "clicked", G_CALLBACK (zoom_in), info);

        toolitem = gtk_tool_button_new_from_stock (GTK_STOCK_ZOOM_OUT);
        gtk_toolbar_insert (GTK_TOOLBAR (toolbar), toolitem, 1);
        g_signal_connect (G_OBJECT (toolitem), "clicked", G_CALLBACK (zoom_out), info);

		gtk_widget_size_request(toolbar, &requisition);

		/* HACK: adjust for frame width & height + packing borders */
		img_height += requisition.height + 30;
		win_height += requisition.height + 30;
		img_width  += 20;
		win_width  += 20;
    }

    if ((xid > 0 && (img_width > win_width || img_height > win_height))
        || (xid <= 0)) {
        gtk_window_set_default_size (GTK_WINDOW (win), MIN (img_width, win_width),
                                     MIN (img_height, win_height));

        scroll = gtk_scrolled_window_new (NULL, NULL);
        gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (scroll),
                                        GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
        gtk_scrolled_window_add_with_viewport (GTK_SCROLLED_WINDOW (scroll), info->image);
        gtk_box_pack_start (GTK_BOX (vbox), scroll, TRUE, TRUE, 0);
    } else {
        gtk_box_pack_start (GTK_BOX (vbox), info->image, TRUE, TRUE, 0);
        gtk_window_set_default_size (GTK_WINDOW (win), img_width, img_height);
    }
}

static void
view_pixbuf (ViewerCbInfo * info, int xid, const char *color)
{
    GtkWidget *win;
    GdkColor bg_color;
    gint win_width, win_height;

    /* create toplevel window and set its title */

#ifdef ENABLE_XEMBED
    if (xid > 0) {
        GdkWindow *gdk_parent;

        win = gtk_plug_new (0);

        gdk_parent = gdk_window_foreign_new (xid);
        gdk_window_get_geometry (gdk_parent, NULL, NULL, &win_width, &win_height, NULL);

        /* so that button and key presses get registered */
        gtk_widget_add_events (win, GDK_BUTTON_PRESS_MASK | GDK_BUTTON_RELEASE_MASK);
    } else
#endif
    {
        char *title;

        win = gtk_window_new (GTK_WINDOW_TOPLEVEL);

        win_width = DEFAULT_WIDTH;
        win_height = DEFAULT_HEIGHT;

        if (info->id) {
            title = g_strdup_printf ("%s#%s — %s", info->base_uri, info->id, _("SVG Viewer"));
        } else {
            title = g_strdup_printf ("%s — %s", info->base_uri,  _("SVG Viewer"));
        }
        gtk_window_set_title (GTK_WINDOW (win), title);
        g_free (title);
    }

    populate_window (win, info, xid, win_width, win_height);

    rsvg_window_set_default_icon (GTK_WINDOW (win), info->pixbuf);

    /* exit when 'X' is clicked */
    g_signal_connect (G_OBJECT (win), "destroy", G_CALLBACK (quit_cb), NULL);
    g_signal_connect (G_OBJECT (win), "delete_event", G_CALLBACK (quit_cb), NULL);

    if (color && strcmp (color, "none") != 0) {
        if (gdk_color_parse (color, &bg_color)) {
            GtkWidget *parent_widget = gtk_widget_get_parent (info->image);

            if (gdk_colormap_alloc_color
                (gtk_widget_get_colormap (parent_widget), &bg_color, FALSE, TRUE))
                gtk_widget_modify_bg (parent_widget, GTK_STATE_NORMAL, &bg_color);
            else
                g_warning (_("Couldn't allocate color '%s'"), color);
        } else
            g_warning (_("Couldn't parse color '%s'"), color);
    }

    create_popup_menu (info);

    info->window = win;
    gtk_window_add_accel_group (GTK_WINDOW (win), info->accel_group);

    g_signal_connect (G_OBJECT (win), "button-press-event", G_CALLBACK (button_press_event), info);

    gtk_widget_show_all (win);

#ifdef ENABLE_XEMBED
    if (xid > 0) {
        XReparentWindow (GDK_WINDOW_XDISPLAY (win->window),
                         GDK_WINDOW_XID (win->window), xid, 0, 0);
        XMapWindow (GDK_WINDOW_XDISPLAY (win->window), GDK_WINDOW_XID (win->window));
    }
#endif
}

int
main (int argc, char **argv)
{
    GError *err = NULL;
    GOptionContext *g_option_context;
    double x_zoom = 1.0;
    double y_zoom = 1.0;
    double dpi_x = -1.0;
    double dpi_y = -1.0;
    int width = -1;
    int height = -1;
    int bVersion = 0;
    char *bg_color = NULL;
    char *base_uri = NULL;
    int bKeepAspect = 0;
    char *id = NULL;

    int xid = -1;
    int from_stdin = 0;
    ViewerCbInfo info;

    struct RsvgSizeCallbackData size_data;

    char **args = NULL;
    gint n_args = 0;

    GOptionEntry options_table[] = {
#ifdef ENABLE_XEMBED
        {"xid", 'i', 0, G_OPTION_ARG_INT, &xid, N_("XWindow ID [for X11 embedding]"), N_("<int>")},
#endif
        {"stdin", 's', 0, G_OPTION_ARG_NONE, &from_stdin, N_("Read from stdin instead of a file"),
         NULL},
        {"dpi-x", 'd', 0, G_OPTION_ARG_DOUBLE, &dpi_x, N_("Set the # of Pixels Per Inch"),
         N_("<float>")},
        {"dpi-y", 'p', 0, G_OPTION_ARG_DOUBLE, &dpi_y, N_("Set the # of Pixels Per Inch"),
         N_("<float>")},
        {"x-zoom", 'x', 0, G_OPTION_ARG_DOUBLE, &x_zoom, N_("Set the x zoom factor"),
         N_("<float>")},
        {"y-zoom", 'y', 0, G_OPTION_ARG_DOUBLE, &y_zoom, N_("Set the y zoom factor"),
         N_("<float>")},
        {"width", 'w', 0, G_OPTION_ARG_INT, &width, N_("Set the image's width"), N_("<int>")},
        {"height", 'h', 0, G_OPTION_ARG_INT, &height, N_("Set the image's height"), N_("<int>")},
        {"bg-color", 'b', 0, G_OPTION_ARG_STRING, &bg_color,
         N_("Set the image background color (default: transparent)"), N_("<string>")},
        {"base-uri", 'u', 0, G_OPTION_ARG_STRING, &base_uri, N_("Set the base URI (default: none)"),
         N_("<string>")},
        {"id", 0, 0, G_OPTION_ARG_STRING, &id, N_("Only show one node (default: all)"),
         N_("<string>")},
        {"keep-aspect", 'k', 0, G_OPTION_ARG_NONE, &bKeepAspect,
         N_("Preserve the image's aspect ratio"), NULL},
        {"version", 'v', 0, G_OPTION_ARG_NONE, &bVersion, N_("Show version information"), NULL},
        {G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, N_("[FILE...]")},
        {NULL}
    };

    g_thread_init (NULL);

    info.pixbuf = NULL;
    info.svg_bytes = NULL;
    info.window = NULL;
    info.popup_menu = NULL;

    g_option_context = g_option_context_new ("- SVG Viewer");
    g_option_context_add_main_entries (g_option_context, options_table, NULL);
    g_option_context_add_group (g_option_context, gtk_get_option_group (TRUE));
    g_option_context_set_help_enabled (g_option_context, TRUE);
    if (!g_option_context_parse (g_option_context, &argc, &argv, NULL)) {
        exit (1);
    }

    g_option_context_free (g_option_context);

    if (bVersion != 0) {
        g_message (_("rsvg-view version %s\n"), VERSION);
        return 0;
    }

    if (args) {
        while (args[n_args] != NULL)
            n_args++;
    }

    if ((!from_stdin) && (n_args != 1)) {
        g_print (_("No files specified, and not using --stdin\n"));
        return 1;
    }

    /* initialize gtk+ and rsvg */
    rsvg_init ();

    rsvg_set_default_dpi_x_y (dpi_x, dpi_y);

    /* if both are unspecified, assume user wants to zoom the pixbuf in at least 1 dimension */
    if (width == -1 && height == -1) {
        size_data.type = RSVG_SIZE_ZOOM;
        size_data.x_zoom = x_zoom;
        size_data.y_zoom = y_zoom;
    }
    /* if both are unspecified, assume user wants to resize pixbuf in at least 1 dimension */
    else if (x_zoom == 1.0 && y_zoom == 1.0) {
        size_data.type = RSVG_SIZE_WH;
        size_data.width = width;
        size_data.height = height;
    }
    /* assume the user wants to zoom the pixbuf, but cap the maximum size */
    else {
        size_data.type = RSVG_SIZE_ZOOM_MAX;
        size_data.x_zoom = x_zoom;
        size_data.y_zoom = y_zoom;
        size_data.width = width;
        size_data.height = height;
    }

    size_data.keep_aspect_ratio = bKeepAspect;

    if (!from_stdin) {
        if (base_uri == NULL)
            base_uri = (char *) args[0];

        info.svg_bytes = _rsvg_acquire_xlink_href_resource (args[0], base_uri, NULL);
    } else {
        info.svg_bytes = g_byte_array_new ();

        for (;;) {
            unsigned char buf[1024 * 8];
            size_t nread = fread (buf, 1, sizeof (buf), stdin);

            if (nread > 0)
                g_byte_array_append (info.svg_bytes, buf, nread);

            if (nread < sizeof (buf)) {
                if (ferror (stdin)) {
                    g_print (_("Error reading\n"));
                    g_byte_array_free (info.svg_bytes, TRUE);
                    fclose (stdin);

                    return 1;
                } else if (feof (stdin))
                    break;
            }
        }

        fclose (stdin);
    }

    if (!info.svg_bytes || !info.svg_bytes->len) {
        g_print (_("Couldn't open %s\n"), args[0]);
        return 1;
    }

    info.base_uri = base_uri;
    info.id = id;

    info.pixbuf = 
        pixbuf_from_data_with_size_data (info.svg_bytes->data, info.svg_bytes->len, &size_data,
                                         base_uri, id, &err);

    if (!info.pixbuf) {
        g_print (_("Error displaying image"));

        if (err) {
            g_print (": %s", err->message);
            g_error_free (err);
        }

        g_print ("\n");

        return 1;
    }

    info.accel_group = gtk_accel_group_new ();

    view_pixbuf (&info, xid, bg_color);

    /* run the gtk+ main loop */
    gtk_main ();

    g_object_unref (G_OBJECT (info.pixbuf));
    g_byte_array_free (info.svg_bytes, TRUE);
    rsvg_term ();

    return 0;
}
