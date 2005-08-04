/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg.h: SAX-based renderer for SVG files into a GdkPixbuf.
 
   Copyright (C) 2000 Eazel, Inc.
  
   This program is free software; you can redistribute it and/or
   modify it under the terms of the GNU Library General Public License as
   published by the Free Software Foundation; either version 2 of the
   License, or (at your option) any later version.
  
   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Library General Public License for more details.
  
   You should have received a copy of the GNU Library General Public
   License along with this program; if not, write to the
   Free Software Foundation, Inc., 59 Temple Place - Suite 330,
   Boston, MA 02111-1307, USA.
  
   Author: Raph Levien <raph@artofcode.com>
*/

#ifndef RSVG_H
#define RSVG_H

#include <gdk-pixbuf/gdk-pixbuf.h>

G_BEGIN_DECLS

typedef enum {
	RSVG_ERROR_FAILED
} RsvgError;

#define RSVG_ERROR (rsvg_error_quark ())
GQuark rsvg_error_quark (void) G_GNUC_CONST;

typedef struct RsvgHandle RsvgHandle;

typedef struct RsvgDimensionData RsvgDimensionData;
struct RsvgDimensionData {
	int width;
	int height;
	gdouble em, ex;
};

void        rsvg_init (void);
void        rsvg_term (void);

void        rsvg_set_default_dpi          (double dpi_x, double dpi_y);
void        rsvg_handle_set_dpi           (RsvgHandle * handle, double dpi_x, double dpi_y);

RsvgHandle *rsvg_handle_new               (void);
gboolean    rsvg_handle_write             (RsvgHandle      *handle,
										   const guchar    *buf,
										   gsize            count,
										   GError         **error);
gboolean    rsvg_handle_close             (RsvgHandle      *handle,
										   GError         **error);
GdkPixbuf  *rsvg_handle_get_pixbuf        (RsvgHandle      *handle);
void        rsvg_handle_free              (RsvgHandle      *handle);

G_CONST_RETURN char *
rsvg_handle_get_base_uri (RsvgHandle *handle);
void rsvg_handle_set_base_uri (RsvgHandle *handle,
							   const char *base_uri);

void rsvg_handle_get_dimensions(RsvgHandle * handle, RsvgDimensionData *dimension_data);

/* Accessibility API */

G_CONST_RETURN char *rsvg_handle_get_title         (RsvgHandle *handle);
G_CONST_RETURN char *rsvg_handle_get_desc          (RsvgHandle *handle);
G_CONST_RETURN char *rsvg_handle_get_metadata      (RsvgHandle *handle);

RsvgHandle * rsvg_handle_new_from_data (const guint8 *data,
										gsize data_len,
										GError **error);
RsvgHandle * rsvg_handle_new_from_file (const gchar *file_name,
										GError **error);

/**
 * RsvgSizeFunc ():
 * @width: Pointer to where to set/store the width
 * @height: Pointer to where to set/store the height
 * @user_data: User data pointer
 *
 * Function to let a user of the library specify the SVG's dimensions
 * @width: the ouput width the SVG should be
 * @height: the output height the SVG should be
 * @user_data: user data
 */
typedef void (* RsvgSizeFunc) (gint     *width,
							   gint     *height,
							   gpointer  user_data);
void        rsvg_handle_set_size_callback (RsvgHandle      *handle,
										   RsvgSizeFunc     size_func,
										   gpointer         user_data,
										   GDestroyNotify   user_data_destroy);

#ifndef RSVG_DISABLE_DEPRECATED

/*
 * TODO: decide whether we want to either:
 * 1) Keep these around, and just implement it in terms of libart or cairo, whichever one we have around
 * 2) Get rid of these completely
 * 3) Push these into librsvg-libart solely
 */

/* Convenience API */

GdkPixbuf  *rsvg_pixbuf_from_file                  (const gchar  *file_name,
													GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_zoom          (const gchar  *file_name,
													double        x_zoom,
													double        y_zoom,
													GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_size          (const gchar  *file_name,
													gint          width,
													gint          height,
													GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_max_size      (const gchar  *file_name,
													gint          max_width,
													gint          max_height,
													GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_zoom_with_max (const gchar  *file_name,
													double        x_zoom,
													double        y_zoom,
													gint          max_width,
													gint          max_height,
													GError      **error);
#endif /* RSVG_DISABLE_DEPRECATED */

G_END_DECLS

#endif /* RSVG_H */
