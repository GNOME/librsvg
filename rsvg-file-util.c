/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-file-util.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

#include "config.h"
#include "rsvg.h"
#include "rsvg-private.h"

#if HAVE_SVGZ
#include "rsvg-gz.h"
#endif

#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>

#ifdef G_OS_UNIX
#include <fcntl.h>
#endif

/* needed for the mozilla plugin */
static void set_nonblocking(FILE * f)
{
#ifdef G_OS_UNIX
	glong fcntl_flags = 0;

#ifdef O_NONBLOCK
    fcntl_flags |= O_NONBLOCK;
#else
    fcntl_flags |= O_NDELAY;
#endif

	(void)fcntl (fileno(f), F_SETFL, fcntl_flags);
#endif
}

#define SVG_BUFFER_SIZE (1024 * 8)

static void
rsvg_size_callback (int *width,
					int *height,
					gpointer  data)
{
	struct RsvgSizeCallbackData *real_data = (struct RsvgSizeCallbackData *) data;
	double zoomx, zoomy, zoom;
	
	switch (real_data->type) {
	case RSVG_SIZE_ZOOM:
		if (*width < 0 || *height < 0)
			return;
		
		*width = floor (real_data->x_zoom * *width + 0.5);
		*height = floor (real_data->y_zoom * *height + 0.5);
		return;
		
	case RSVG_SIZE_ZOOM_MAX:
		if (*width < 0 || *height < 0)
			return;
		
		*width = floor (real_data->x_zoom * *width + 0.5);
		*height = floor (real_data->y_zoom * *height + 0.5);
		
		if (*width > real_data->width || *height > real_data->height)
			{
				zoomx = (double) real_data->width / *width;
				zoomy = (double) real_data->height / *height;
				zoom = MIN (zoomx, zoomy);
				
				*width = floor (zoom * *width + 0.5);
				*height = floor (zoom * *height + 0.5);
			}
		return;
		
	case RSVG_SIZE_WH_MAX:
		if (*width < 0 || *height < 0)
			return;
		
		zoomx = (double) real_data->width / *width;
		zoomy = (double) real_data->height / *height;
		zoom = MIN (zoomx, zoomy);
		
		*width = floor (zoom * *width + 0.5);
		*height = floor (zoom * *height + 0.5);
		return;
		
	case RSVG_SIZE_WH:
		
		if (real_data->width != -1)
			*width = real_data->width;
		if (real_data->height != -1)
			*height = real_data->height;
		return;
	}
	
	g_assert_not_reached ();
}

static GdkPixbuf *
rsvg_pixbuf_from_file_with_size_data_ex (RsvgHandle * handle,
										 const gchar * file_name,
										 struct RsvgSizeCallbackData * data,
										 GError ** error)
{
	guchar chars[SVG_BUFFER_SIZE];
	GdkPixbuf *retval;
	gint result;
	FILE *f = fopen (file_name, "rb");

	if (!f)
		{
			g_set_error (error, G_FILE_ERROR,
						 g_file_error_from_errno (errno),
						 g_strerror (errno));
			return NULL;
		}
	
	rsvg_handle_set_size_callback (handle, rsvg_size_callback, data, NULL);

	while (!feof(f) && !ferror(f) && ((result = fread (chars, 1, SVG_BUFFER_SIZE, f)) > 0))
		rsvg_handle_write (handle, chars, result, error);
	
	rsvg_handle_close (handle, error);
	retval = rsvg_handle_get_pixbuf (handle);
	
	fclose (f);	
	return retval;
}

/* private */
GdkPixbuf *
rsvg_pixbuf_from_stdio_file_with_size_data(FILE * f,
										   struct RsvgSizeCallbackData * data,
										   GError ** error)
{
	RsvgHandle * handle;
	GdkPixbuf * retval;
	guchar chars[SVG_BUFFER_SIZE];
	int result;

	set_nonblocking(f);

	result = fread (chars, 1, SVG_BUFFER_SIZE, f);

	if (result == 0) {
		g_set_error (error, G_FILE_ERROR,
					 g_file_error_from_errno (errno),
					 g_strerror (errno));
		return NULL;
	}

#if HAVE_SVGZ
	/* test for GZ marker */
	if ((result >= 2) && (chars[0] == (guchar)0x1f) && (chars[1] == (guchar)0x8b))
		handle = rsvg_handle_new_gz ();
	else
		handle = rsvg_handle_new ();
#else
	handle = rsvg_handle_new ();
#endif

	rsvg_handle_set_size_callback (handle, rsvg_size_callback, data, NULL);

	rsvg_handle_write (handle, chars, result, error);

	while (!feof(f) && !ferror(f) && ((result = fread (chars, 1, SVG_BUFFER_SIZE, f)) > 0))
		rsvg_handle_write (handle, chars, result, error);
	
	rsvg_handle_close (handle, error);
	retval = rsvg_handle_get_pixbuf (handle);
	rsvg_handle_free (handle);

	return retval;
}

/* private */
GdkPixbuf *
rsvg_pixbuf_from_file_with_size_data (const gchar * file_name,
									  struct RsvgSizeCallbackData * data,
									  GError ** error)
{
	GdkPixbuf * pixbuf;
	FILE *f = fopen (file_name, "rb");

	if (!f)
		{
			g_set_error (error, G_FILE_ERROR,
						 g_file_error_from_errno (errno),
						 g_strerror (errno));
			return NULL;
		}
	
	pixbuf = rsvg_pixbuf_from_stdio_file_with_size_data(f, data, error);

	fclose(f);

	return pixbuf;
}


/**
 * rsvg_pixbuf_from_file_at_size_ex:
 * @handle: The RSVG handle you wish to render with (either normal or gzipped)
 * @file_name: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * either of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the returned pixbuf. If an
 * error occurred, @error is set and %NULL is returned. Returned handle is closed
 * by this call and must be freed by the caller.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 *
 * Since: 2.4
 */
GdkPixbuf  *
rsvg_pixbuf_from_file_at_size_ex (RsvgHandle * handle,
								  const gchar  *file_name,
								  gint          width,
								  gint          height,
								  GError      **error)
{
	struct RsvgSizeCallbackData data;
	
	data.type = RSVG_SIZE_WH;
	data.width = width;
	data.height = height;
	
	return rsvg_pixbuf_from_file_with_size_data_ex (handle, file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_ex:
 * @handle: The RSVG handle you wish to render with (either normal or gzipped)
 * @file_name: A file name
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  The caller must
 * assume the reference to the reurned pixbuf. If an error occurred, @error is
 * set and %NULL is returned. Returned handle is closed by this call and must be
 * freed by the caller.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 *
 * Since: 2.4
 */
GdkPixbuf  *
rsvg_pixbuf_from_file_ex (RsvgHandle * handle,
						  const gchar  *file_name,
						  GError      **error)
{
	return rsvg_pixbuf_from_file_at_size_ex (handle, file_name, -1, -1, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom_ex:
 * @handle: The RSVG handle you wish to render with (either normal or gzipped)
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom.  The
 * caller must assume the reference to the returned pixbuf. If an error
 * occurred, @error is set and %NULL is returned. Returned handle is closed by this 
 * call and must be freed by the caller.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 *
 * Since: 2.4
 */
GdkPixbuf  *
rsvg_pixbuf_from_file_at_zoom_ex (RsvgHandle * handle,
								  const gchar  *file_name,
								  double        x_zoom,
								  double        y_zoom,
								  GError      **error)
{
	struct RsvgSizeCallbackData data;
	
	g_return_val_if_fail (file_name != NULL, NULL);
	g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
	
	data.type = RSVG_SIZE_ZOOM;
	data.x_zoom = x_zoom;
	data.y_zoom = y_zoom;
	
	return rsvg_pixbuf_from_file_with_size_data_ex (handle, file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_max_size_ex:
 * @handle: The RSVG handle you wish to render with (either normal or gzipped)
 * @file_name: A file name
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is uniformly
 * scaled so that the it fits into a rectangle of size max_width * max_height. The
 * caller must assume the reference to the returned pixbuf. If an error occurred,
 * @error is set and %NULL is returned. Returned handle is closed by this call and 
 * must be freed by the caller.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 *
 * Since: 2.4
 */
GdkPixbuf  *
rsvg_pixbuf_from_file_at_max_size_ex (RsvgHandle * handle,
									  const gchar  *file_name,
									  gint          max_width,
									  gint          max_height,
									  GError      **error)
{
	struct RsvgSizeCallbackData data;
	
	data.type = RSVG_SIZE_WH_MAX;
	data.width = max_width;
	data.height = max_height;
	
	return rsvg_pixbuf_from_file_with_size_data_ex (handle, file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom_with_max_ex:
 * @handle: The RSVG handle you wish to render with (either normal or gzipped)
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom. If the
 * resulting pixbuf would be larger than max_width/max_heigh it is uniformly scaled
 * down to fit in that rectangle. The caller must assume the reference to the
 * returned pixbuf. If an error occurred, @error is set and %NULL is returned.
 * Returned handle is closed by this call and must be freed by the caller.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 *
 * Since: 2.4
 */
GdkPixbuf  *
rsvg_pixbuf_from_file_at_zoom_with_max_ex (RsvgHandle * handle,
										   const gchar  *file_name,
										   double        x_zoom,
										   double        y_zoom,
										   gint          max_width,
										   gint          max_height,
										   GError      **error)
{
	struct RsvgSizeCallbackData data;
	
	g_return_val_if_fail (file_name != NULL, NULL);
	g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
	
	data.type = RSVG_SIZE_ZOOM_MAX;
	data.x_zoom = x_zoom;
	data.y_zoom = y_zoom;
	data.width = max_width;
	data.height = max_height;
	
	return rsvg_pixbuf_from_file_with_size_data_ex (handle, file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file:
 * @file_name: A file name
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  The caller must
 * assume the reference to the reurned pixbuf. If an error occurred, @error is
 * set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file (const gchar *file_name,
					   GError     **error)
{
	return rsvg_pixbuf_from_file_at_size (file_name, -1, -1, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom:
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom.  The
 * caller must assume the reference to the returned pixbuf. If an error
 * occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom (const gchar *file_name,
							   double       x_zoom,
							   double       y_zoom,
							   GError     **error)
{
	struct RsvgSizeCallbackData data;
	
	g_return_val_if_fail (file_name != NULL, NULL);
	g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
	
	data.type = RSVG_SIZE_ZOOM;
	data.x_zoom = x_zoom;
	data.y_zoom = y_zoom;
	
	return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_zoom_with_max:
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom. If the
 * resulting pixbuf would be larger than max_width/max_heigh it is uniformly scaled
 * down to fit in that rectangle. The caller must assume the reference to the
 * returned pixbuf. If an error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf  *
rsvg_pixbuf_from_file_at_zoom_with_max (const gchar  *file_name,
										double        x_zoom,
										double        y_zoom,
										gint          max_width,
										gint          max_height,
										GError      **error)
{
	struct RsvgSizeCallbackData data;
	
	g_return_val_if_fail (file_name != NULL, NULL);
	g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);
	
	data.type = RSVG_SIZE_ZOOM_MAX;
	data.x_zoom = x_zoom;
	data.y_zoom = y_zoom;
	data.width = max_width;
	data.height = max_height;
	
	return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_size:
 * @file_name: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * either of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the returned pixbuf. If an
 * error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_size (const gchar *file_name,
							   gint         width,
							   gint         height,
							   GError     **error)
{
	struct RsvgSizeCallbackData data;
	
	data.type = RSVG_SIZE_WH;
	data.width = width;
	data.height = height;
	
	return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}

/**
 * rsvg_pixbuf_from_file_at_max_size:
 * @file_name: A file name
 * @max_width: The requested max width
 * @max_height: The requested max heigh
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is uniformly
 * scaled so that the it fits into a rectangle of size max_width * max_height. The
 * caller must assume the reference to the returned pixbuf. If an error occurred,
 * @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf  *
rsvg_pixbuf_from_file_at_max_size (const gchar     *file_name,
								   gint             max_width,
								   gint             max_height,
								   GError         **error)
{
	struct RsvgSizeCallbackData data;
	
	data.type = RSVG_SIZE_WH_MAX;
	data.width = max_width;
	data.height = max_height;
	
	return rsvg_pixbuf_from_file_with_size_data (file_name, &data, error);
}
