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

typedef void (* RsvgSizeFunc) (gint     *width,
			       gint     *height,
			       gpointer  user_data);


RsvgHandle *rsvg_handle_new               (void);
void        rsvg_handle_set_size_callback (RsvgHandle      *handle,
					   RsvgSizeFunc     size_func,
					   gpointer         user_data,
					   GDestroyNotify   user_data_destroy);
gboolean    rsvg_handle_write             (RsvgHandle      *handle,
					   const guchar    *buf,
					   gsize            count,
					   GError         **error);
gboolean    rsvg_handle_close             (RsvgHandle      *handle,
					   GError         **error);
GdkPixbuf  *rsvg_handle_get_pixbuf        (RsvgHandle      *handle);
void        rsvg_handle_free              (RsvgHandle      *handle);

/* convenience API */
GdkPixbuf  *rsvg_pixbuf_from_file             (const gchar  *file_name,
					       GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_zoom     (const gchar  *file_name,
					       double        x_zoom,
					       double        y_zoom,
					       GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_size     (const gchar  *file_name,
					       gint          width,
					       gint          height,
					       GError      **error);
GdkPixbuf  *rsvg_pixbuf_from_file_at_max_size (const gchar  *file_name,
					       gint          max_width,
					       gint          max_height,
					       GError      **error);

G_END_DECLS

#endif
