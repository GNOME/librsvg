/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
#include "rsvg-size-callback.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <locale.h>

#include <gtk/gtk.h>
#include <gdk/gdk.h>

#if 0 // defined (G_OS_UNIX)
#include <gio/gunixinputstream.h>
#endif

#define DEFAULT_WIDTH  640
#define DEFAULT_HEIGHT 480

/* RsvgImage */

#define RSVG_TYPE_IMAGE (rsvg_image_get_type ())
#define RSVG_IMAGE(obj) (G_TYPE_CHECK_INSTANCE_CAST ((obj), RSVG_TYPE_IMAGE, RsvgImage))

typedef struct _RsvgImage       RsvgImage;
typedef struct _RsvgImageClass  RsvgImageClass;

struct _RsvgImage {
    GtkWidget parent_instance;

    cairo_surface_t *surface; /* a cairo image surface */
};

struct _RsvgImageClass {
    GtkWidgetClass parent_class;
};

static GType rsvg_image_get_type (void);

static void
rsvg_image_take_surface (RsvgImage *image,
                         cairo_surface_t *surface)
{
    if (image->surface == surface)
      return;
    if (image->surface)
      cairo_surface_destroy (image->surface);
    image->surface = surface; /* adopted */

    gtk_widget_queue_resize (GTK_WIDGET (image));
}

G_DEFINE_TYPE (RsvgImage, rsvg_image, GTK_TYPE_WIDGET);

static void
rsvg_image_init (RsvgImage *image)
{
  gtk_widget_set_has_window (GTK_WIDGET (image), FALSE);
}

static void
rsvg_image_finalize (GObject *object)
{
  RsvgImage *image = RSVG_IMAGE (object);

  rsvg_image_take_surface (image, NULL);
}

static void
rsvg_image_get_preferred_width (GtkWidget *widget,
                                gint      *minimum,
                                gint      *natural)
{
  RsvgImage *image = RSVG_IMAGE (widget);

  *minimum = *natural = image->surface ? cairo_image_surface_get_width (image->surface) : 1;
}

static void
rsvg_image_get_preferred_height (GtkWidget *widget,
                                 gint      *minimum,
                                 gint      *natural)
{
  RsvgImage *image = RSVG_IMAGE (widget);

  *minimum = *natural = image->surface ? cairo_image_surface_get_height (image->surface) : 1;
}

static gboolean
rsvg_image_draw (GtkWidget *widget,
                 cairo_t *cr)
{
  RsvgImage *image = RSVG_IMAGE (widget);

  if (image->surface == NULL)
      return FALSE;

  cairo_save (cr);
  cairo_set_source_surface (cr, image->surface, 0, 0);
  cairo_paint (cr);
  cairo_restore (cr);

  return FALSE;
}

static void
rsvg_image_class_init (RsvgImageClass *klass)
{
  GObjectClass *gobject_class = G_OBJECT_CLASS (klass);
  GtkWidgetClass *widget_class = GTK_WIDGET_CLASS (klass);

  gobject_class->finalize = rsvg_image_finalize;
  widget_class->get_preferred_width = rsvg_image_get_preferred_width;
  widget_class->get_preferred_height = rsvg_image_get_preferred_height;
  widget_class->draw = rsvg_image_draw;
}

static RsvgImage *
rsvg_image_new_take_surface (cairo_surface_t *surface)
{
  RsvgImage *image;

  image = g_object_new (RSVG_TYPE_IMAGE, NULL);
  rsvg_image_take_surface (image, surface);

  return image;
}

static cairo_surface_t *
rsvg_image_get_surface (RsvgImage *image)
{
  return image->surface;
}

/* Main */

static char *
_rsvg_basename (const char *file)
{
    if (file && *file)
        return g_path_get_basename (file);

    return NULL;
}

typedef struct _ViewerCbInfo ViewerCbInfo;
struct _ViewerCbInfo {
    GtkWidget *window;
    GtkWidget *popup_menu;
    RsvgImage *image;
    RsvgHandle *handle;
    GtkAccelGroup *accel_group;
    char *base_uri;
    char *id;
    RsvgDimensionData dimensions;
    gdouble x_zoom;
    gdouble y_zoom;
};

static cairo_surface_t *
render_to_surface (ViewerCbInfo *info)
{
    int width, height;
    cairo_matrix_t matrix;
    cairo_surface_t *surface;
    cairo_t *cr;

    width = ceil ((double) info->dimensions.width * info->x_zoom);
    height = ceil ((double) info->dimensions.height * info->y_zoom);

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32, width, height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return NULL;
    }

    cr = cairo_create (surface);

    cairo_matrix_init_scale (&matrix, info->x_zoom, info->y_zoom);
    cairo_transform (cr, &matrix);

    if (!rsvg_handle_render_cairo_sub (info->handle, cr, info->id)) {
        cairo_destroy (cr);
        cairo_surface_destroy (surface);
        return NULL;
    }

    cairo_destroy (cr);

    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        g_printerr ("Error while rendering image: %d\n", cairo_surface_status (surface));
        cairo_surface_destroy (surface);
        return NULL;
    }

    return surface;
}

static void
set_window_title (ViewerCbInfo * info)
{
    char *title;
    gchar *zoom_string;

    if (info->x_zoom != info->y_zoom) {
        zoom_string = g_strdup_printf ("%4d%% : %4d%%",
                                       (gint) (info->x_zoom * 100),
                                       (gint) (info->y_zoom * 100));
    } else {
        zoom_string = g_strdup_printf ("%4d%%",
                                       (gint) (info->x_zoom * 100));
    }

    if (info->id) {
        title = g_strdup_printf ("%s#%s (%s) — %s",
                                 info->base_uri, info->id,
                                 zoom_string,
                                 _("SVG Viewer"));
    } else {
        title = g_strdup_printf ("%s (%s) — %s",
                                 info->base_uri,
                                 zoom_string,
                                 _("SVG Viewer"));
    }
    gtk_window_set_title (GTK_WINDOW (info->window), title);
    g_free (title);
    g_free (zoom_string);
}

static void
zoom_image (ViewerCbInfo * info, gdouble factor)
{
    info->x_zoom *= factor;
    info->y_zoom *= factor;

    rsvg_image_take_surface (info->image, render_to_surface (info));

    set_window_title (info);
}

static void
zoom_in (GObject * ignored, ViewerCbInfo * info)
{
    zoom_image (info, sqrt (G_SQRT2));
}

static void
zoom_out (GObject * ignored, ViewerCbInfo * info)
{
    zoom_image (info, 1. / sqrt (G_SQRT2));
}

static void
begin_print (GtkPrintOperation *operation,
			 GtkPrintContext   *context,
			 gpointer           user_data)
{
	gtk_print_operation_set_n_pages (operation, 1);
}

static void
draw_page (GtkPrintOperation *operation,
		   GtkPrintContext   *context,
		   gint               page_nr,
		   gpointer           user_data)
{
    ViewerCbInfo *info = (ViewerCbInfo *) user_data;
    cairo_t *cr;
    gdouble page_width, page_height, page_aspect;
    gdouble width, height, aspect;
    cairo_matrix_t matrix;

    cr = gtk_print_context_get_cairo_context (context);
    page_width = gtk_print_context_get_width (context);
    page_height = gtk_print_context_get_height (context);
    page_aspect = page_width / page_height;

    // FIXMEchpe
    rsvg_handle_set_dpi_x_y (info->handle, 
                             gtk_print_context_get_dpi_x(context), 
                             gtk_print_context_get_dpi_y(context));

    width = info->dimensions.width;
    height = info->dimensions.height;
    aspect = width / height;

    if (aspect <= page_aspect) {
        width = page_height * aspect;
        height = page_height;
    } else {
        width = page_width;
        height = page_width / aspect;
    }

    cairo_save (cr);
    cairo_matrix_init_scale (&matrix, 
                             width / info->dimensions.width,
                             height / info->dimensions.height);
    cairo_transform (cr, &matrix);
    rsvg_handle_render_cairo (info->handle, cr);
    cairo_restore (cr);
}

static void
print_pixbuf (GObject * ignored, gpointer user_data)
{
  GtkPrintOperation *print;
  ViewerCbInfo *info = (ViewerCbInfo *) user_data;

  print = gtk_print_operation_new ();

  g_signal_connect (print, "begin_print", G_CALLBACK (begin_print), info);
  g_signal_connect (print, "draw_page", G_CALLBACK (draw_page), info);

  (void)gtk_print_operation_run (print, GTK_PRINT_OPERATION_ACTION_PRINT_DIALOG,
                                 GTK_WINDOW (info->window), NULL);

  g_object_unref (print);
}

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
    cairo_surface_t *surface;

    base_name = _rsvg_basename (info->base_uri);
    if (base_name)
        filename_suggestion = g_strdup_printf ("%s.png", base_name);
    else
        filename_suggestion = NULL;

    filename = save_file (_("Save SVG as PNG"), filename_suggestion, info->window, &success);
    g_free (base_name);
    g_free (filename_suggestion);

    if (filename) {
        surface = rsvg_image_get_surface (info->image);
        if (cairo_surface_write_to_png (surface, filename) != CAIRO_STATUS_SUCCESS) {
                GtkWidget *errmsg;

                errmsg = gtk_message_dialog_new (GTK_WINDOW (info->window),
                                                 GTK_DIALOG_MODAL | GTK_DIALOG_DESTROY_WITH_PARENT,
                                                 GTK_MESSAGE_WARNING,
                                                 GTK_BUTTONS_CLOSE, "Failed to save");

                gtk_dialog_run (GTK_DIALOG (errmsg));

                gtk_widget_destroy (errmsg);
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
        gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_KEY_C,
                                    GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);
    }

    menu_item = gtk_image_menu_item_new_with_label (_("Save as PNG"));
    stock = gtk_image_new_from_stock (GTK_STOCK_SAVE_AS, GTK_ICON_SIZE_MENU);
    gtk_widget_show (stock);
    gtk_image_menu_item_set_image (GTK_IMAGE_MENU_ITEM (menu_item), stock);
    g_signal_connect (menu_item, "activate", G_CALLBACK (save_pixbuf), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_KEY_S,
                                GDK_CONTROL_MASK | GDK_SHIFT_MASK, GTK_ACCEL_VISIBLE);

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_PRINT, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (print_pixbuf), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_KEY_P, GDK_CONTROL_MASK,
                                GTK_ACCEL_VISIBLE);

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_ZOOM_IN, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (zoom_in), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_KEY_plus,
                                GDK_CONTROL_MASK, GTK_ACCEL_VISIBLE);

    menu_item = gtk_image_menu_item_new_from_stock (GTK_STOCK_ZOOM_OUT, NULL);
    g_signal_connect (menu_item, "activate", G_CALLBACK (zoom_out), info);
    gtk_widget_show (menu_item);
    gtk_menu_shell_append (GTK_MENU_SHELL (popup_menu), menu_item);
    gtk_widget_add_accelerator (menu_item, "activate", info->accel_group, GDK_KEY_minus,
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
populate_window (GtkWidget * win, 
                 ViewerCbInfo * info, 
                 cairo_surface_t *surface /* adopted */,
                 gint win_width, 
                 gint win_height)
{
    GtkWidget *vbox;
    GtkWidget *scroll;
    GtkWidget *toolbar;
    GtkToolItem *toolitem;
    GtkRequisition requisition;
    gint img_width, img_height;

#if GTK_CHECK_VERSION (3, 2, 0)
    vbox = gtk_box_new (GTK_ORIENTATION_VERTICAL, 0);
#else
    vbox = gtk_vbox_new (FALSE, 0);
#endif
    gtk_container_add (GTK_CONTAINER (win), vbox);

    /* pack the window with the image */
    img_width = cairo_image_surface_get_width (surface);
    img_height = cairo_image_surface_get_height (surface);

    /* create a new image */
    info->image = rsvg_image_new_take_surface (surface);

    toolbar = gtk_toolbar_new ();
    gtk_box_pack_start (GTK_BOX (vbox), toolbar, FALSE, FALSE, 0);

    toolitem = gtk_tool_button_new_from_stock (GTK_STOCK_ZOOM_IN);
    gtk_toolbar_insert (GTK_TOOLBAR (toolbar), toolitem, 0);
    g_signal_connect (toolitem, "clicked", G_CALLBACK (zoom_in), info);

    toolitem = gtk_tool_button_new_from_stock (GTK_STOCK_ZOOM_OUT);
    gtk_toolbar_insert (GTK_TOOLBAR (toolbar), toolitem, 1);
    g_signal_connect (toolitem, "clicked", G_CALLBACK (zoom_out), info);

    gtk_widget_size_request(toolbar, &requisition);

    /* HACK: adjust for frame width & height + packing borders */
    img_height += requisition.height + 30;
    win_height += requisition.height + 30;
    img_width  += 20;
    win_width  += 20;

    scroll = gtk_scrolled_window_new (NULL, NULL);
    gtk_scrolled_window_set_policy (GTK_SCROLLED_WINDOW (scroll),
                                    GTK_POLICY_AUTOMATIC, GTK_POLICY_AUTOMATIC);
    gtk_scrolled_window_add_with_viewport (GTK_SCROLLED_WINDOW (scroll), 
                                           GTK_WIDGET (info->image));
    gtk_box_pack_start (GTK_BOX (vbox), scroll, TRUE, TRUE, 0);
}

static void
view_surface (ViewerCbInfo * info, 
              cairo_surface_t *surface /* adopted */,
              const char *color)
{
    GtkWidget *win;
    GdkColor bg_color;
    gint win_width, win_height;

    /* create toplevel window and set its title */

    win = gtk_window_new (GTK_WINDOW_TOPLEVEL);

    win_width = DEFAULT_WIDTH;
    win_height = DEFAULT_HEIGHT;

    populate_window (win, info, surface, win_width, win_height);

    /* exit when 'X' is clicked */
    g_signal_connect (win, "destroy", G_CALLBACK (quit_cb), NULL);
    g_signal_connect (win, "delete_event", G_CALLBACK (quit_cb), NULL);

    if (color && strcmp (color, "none") != 0) {
        if (gdk_color_parse (color, &bg_color)) {
            GtkWidget *parent_widget = gtk_widget_get_parent (GTK_WIDGET (info->image));

            gtk_widget_modify_bg (parent_widget, GTK_STATE_NORMAL, &bg_color);
        } else
            g_warning (_("Couldn't parse color '%s'"), color);
    }

    create_popup_menu (info);

    info->window = win;
    gtk_window_add_accel_group (GTK_WINDOW (win), info->accel_group);

    g_signal_connect (win, "button-press-event", G_CALLBACK (button_press_event), info);

    gtk_widget_show_all (win);

    set_window_title (info);
}

int
main (int argc, char **argv)
{
    int retval = 1;
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
    gboolean keep_aspect_ratio = FALSE;
    char *id = NULL;
    GInputStream *input;
    GFileInfo *file_info;
    gboolean compressed;
    GFile *file, *base_file;
    cairo_surface_t *surface;

    int from_stdin = 0;
    ViewerCbInfo info;

    char **args = NULL;
    gint n_args = 0;

    GOptionEntry options_table[] = {
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
        {"keep-aspect", 'k', 0, G_OPTION_ARG_NONE, &keep_aspect_ratio,
         N_("Preserve the image's aspect ratio"), NULL},
        {"version", 'v', 0, G_OPTION_ARG_NONE, &bVersion, N_("Show version information"), NULL},
        {G_OPTION_REMAINING, 0, 0, G_OPTION_ARG_FILENAME_ARRAY, &args, NULL, N_("[FILE...]")},
        {NULL}
    };

	/* Set the locale so that UTF-8 filenames work */
    setlocale(LC_ALL, "");

    g_type_init ();

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

    if (args)
        n_args = g_strv_length (args);
    else
        n_args = 0;

    if ((!from_stdin) && (n_args != 1)) {
        g_print (_("No files specified, and not using --stdin\n"));
        return 1;
    }

    rsvg_set_default_dpi_x_y (dpi_x, dpi_y);

    compressed = FALSE;

    if (from_stdin) {
#if 0 // defined (G_OS_UNIX)
        input = g_unix_input_stream_new (STDIN_FILENO, FALSE);
#else
        input = NULL;
        g_set_error_literal (&err, G_IO_ERROR, G_IO_ERROR_NOT_SUPPORTED,
                             "Reading from stdin not supported");
#endif
        base_file = NULL;
    } else {
        file = g_file_new_for_commandline_arg (args[0]);
        input = (GInputStream *) g_file_read (file, NULL, &err);

        if (base_uri)
            base_file = g_file_new_for_uri (base_uri);
        else
            base_file = g_object_ref (file);

        if ((file_info = g_file_query_info (file,
                                            G_FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
                                            G_FILE_QUERY_INFO_NONE,
                                            NULL,
                                            NULL))) {
            const char *content_type;
            char *gz_content_type;

            content_type = g_file_info_get_content_type (file_info);
            gz_content_type = g_content_type_from_mime_type ("application/x-gzip");
            compressed = (content_type != NULL && g_content_type_is_a (content_type, gz_content_type));
            g_free (gz_content_type);
            g_object_unref (file_info);
        }

        g_object_unref (file);
    }

    g_strfreev (args);

    if (input == NULL) {
        g_printerr ("Failed to read input: %s\n", err->message);
        g_error_free (err);
        return 1;
    }

    if (compressed) {
        GZlibDecompressor *decompressor;
        GInputStream *converter_stream;

        decompressor = g_zlib_decompressor_new (G_ZLIB_COMPRESSOR_FORMAT_GZIP);
        converter_stream = g_converter_input_stream_new (input, G_CONVERTER (decompressor));
        g_object_unref (input);
        input = converter_stream;
    }

    info.base_uri = base_file ? g_file_get_uri (base_file) : g_strdup ("");
    info.id = id;
    info.x_zoom = x_zoom;
    info.y_zoom = y_zoom;

    info.handle = rsvg_handle_new_from_stream_sync (input, 
                                                    base_file, 
                                                    RSVG_HANDLE_FLAGS_NONE,
                                                    NULL /* cancellable */,
                                                    &err);
    g_object_unref (base_file);
    g_object_unref (input);

    if (info.handle == NULL) {
        g_printerr ("Failed to load SVG: %s\n", err->message);
        g_error_free (err);
        return 1;
    }

    rsvg_handle_get_dimensions (info.handle, &info.dimensions);

    if (width != -1) {
        info.x_zoom = (double) width / info.dimensions.width;
    } else {
        info.x_zoom = x_zoom;
    }
    if (height != -1) {
        info.y_zoom = (double) height / info.dimensions.height;
    } else {
        info.y_zoom = y_zoom;
    }

    surface = render_to_surface (&info);
    if (surface == NULL) {
        g_printerr ("Unknown error while rendering image\n");
        goto done;
    }

    retval = 0;

    info.accel_group = gtk_accel_group_new ();

    view_surface (&info, surface, bg_color);

    /* run the gtk+ main loop */
    gtk_main ();

  done:

    g_free (info.base_uri);
    g_object_unref (info.handle);

    rsvg_cleanup ();

    return retval;
}
