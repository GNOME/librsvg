/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-image.c: Image loading and displaying

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "rsvg-image.h"
#include <string.h>
#include <math.h>
#include <errno.h>
#include "rsvg-css.h"

static const char s_UTF8_B64Alphabet[64] = {
	0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
	0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, /* A-Z */
	0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f,
	0x70, 0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, /* a-z */
	0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, /* 0-9 */
	0x2b, /* + */
	0x2f  /* / */
};
static const char utf8_b64_pad = 0x3d;

static gboolean b64_decode_char (char c, int * b64)
{
	if ((c >= 0x41) && (c <= 0x5a))
		{
			*b64 = c - 0x41;
			return TRUE;
		}
	if ((c >= 0x61) && (c <= 0x7a))
		{
			*b64 = c - (0x61 - 26);
			return TRUE;
		}
	if ((c >= 0x30) && (c <= 0x39))
		{
			*b64 = c + (52 - 0x30);
			return TRUE;
		}
	if (c == 0x2b)
		{
			*b64 = 62;
			return TRUE;
		}
	if (c == 0x2f)
		{
			*b64 = 63;
			return TRUE;
		}
	return FALSE;
}

static gboolean utf8_base64_decode(guchar ** binptr, size_t * binlen, const char * b64ptr, size_t b64len)
{
	gboolean decoded = TRUE;
	gboolean padding = FALSE;
	
	int i = 0;
	glong ucs4_len, j;

	unsigned char byte1 = 0;
	unsigned char byte2;
	
	gunichar ucs4, * ucs4_str;
	
	if (b64len == 0) 
		return TRUE;
	
	if ((binptr == 0) || (b64ptr == 0)) 
		return FALSE;
	
	ucs4_str = g_utf8_to_ucs4_fast(b64ptr, b64len, &ucs4_len);
	
	for(j = 0; j < ucs4_len; j++)
		{
			ucs4 = ucs4_str[j];
			if ((ucs4 & 0x7f) == ucs4)
				{
					int b64;
					char c = (char)(ucs4);

					if (b64_decode_char (c, &b64))
						{
							if (padding || (*binlen == 0))
								{
									decoded = FALSE;
									break;
								}

							switch (i)
								{
								case 0:
									byte1 = (unsigned char)(b64) << 2;
									i++;
									break;
								case 1:
									byte2 = (unsigned char)(b64);
									byte1 |= byte2 >> 4;
									*(*binptr)++ = (char)(byte1);
									(*binlen)--;
									byte1 = (byte2 & 0x0f) << 4;
									i++;
									break;
								case 2:
									byte2 = (unsigned char)(b64);
									byte1 |= byte2 >> 2;
									*(*binptr)++ = (char)(byte1);
									(*binlen)--;
									byte1 = (byte2 & 0x03) << 6;
									i++;
									break;
								default:
									byte1 |= (unsigned char)(b64);
									*(*binptr)++ = (char)(byte1);
									(*binlen)--;
									i = 0;
									break;
								}
							
							if (!decoded) 
								break;

							continue;
						}
					else if (c == utf8_b64_pad)
						{
							switch (i)
								{
								case 0:
								case 1:
									decoded = FALSE;
									break;
								case 2:
									if (*binlen == 0) 
										decoded = FALSE;
									else
										{
											*(*binptr)++ = (char)(byte1);
											(*binlen)--;
											padding = TRUE;
										}
									i++;
									break;
								default:
									if (!padding)
										{
											if (*binlen == 0) 
												decoded = FALSE;
											else
												{
													*(*binptr)++ = (char)(byte1);
													(*binlen)--;
													padding = TRUE;
												}
										}
									i = 0;
									break;
								}
							if (!decoded) 
								break;

							continue;
						}
				}
			if (g_unichar_isspace (ucs4)) 
				continue;

			decoded = FALSE;
			break;
		}

	g_free(ucs4_str);
	return decoded;
}

static GByteArray *
rsvg_acquire_base64_resource (const char *data,
							GError    **error)
{
	GByteArray * array;
	
	guchar *bufptr;
	size_t buffer_len, buffer_max_len, data_len;

	g_return_val_if_fail (data != NULL, NULL);

	while (*data) if (*data++ == ',') break;

	data_len = strlen(data);
	
	buffer_max_len = ((data_len >> 2) + 1) * 3;
	buffer_len = buffer_max_len;

	array = g_byte_array_sized_new (buffer_max_len);
	bufptr = array->data;

	if(!utf8_base64_decode(&bufptr, &buffer_len, data, data_len)) {
		g_byte_array_free (array, TRUE);
		return NULL;
	}

	array->len = buffer_max_len - buffer_len;
	
	return array;
}

gchar *
rsvg_get_file_path (const gchar * filename, const gchar *basedir)
{
	gchar *absolute_filename;

	if (g_file_test(filename, G_FILE_TEST_EXISTS) || g_path_is_absolute(filename)) {
		absolute_filename = g_strdup (filename);
	} else {
		gchar *tmpcdir;

		if (basedir)
			tmpcdir = g_path_get_dirname (basedir);
		else
			tmpcdir = g_get_current_dir ();

		absolute_filename = g_build_filename (tmpcdir, filename, NULL);
		g_free(tmpcdir);
	}

	return absolute_filename;
}

static GByteArray *
rsvg_acquire_file_resource (const char *filename,
							const char *base_uri,
							GError    **error)
{
	GByteArray *array;
	gchar *path;

	guchar buffer [4096];
	int length;
	FILE *f;

	g_return_val_if_fail (filename != NULL, NULL);
	
	path = rsvg_get_file_path (filename, base_uri);
	f = fopen (path, "rb");
	g_free (path);
	
	if (!f) {
		g_set_error (error,
					 G_FILE_ERROR,
					 g_file_error_from_errno (errno),
					 _("Failed to open file '%s': %s"),
					 filename, g_strerror (errno));
		return NULL;
	}

	/* TODO: an optimization is to use the file's size */
	array = g_byte_array_new ();
	
	while (!feof (f)) {
		length = fread (buffer, 1, sizeof (buffer), f);
		if (length > 0) {
			if (g_byte_array_append (array, buffer, length) == NULL) {
				fclose (f);
				g_byte_array_free (array, TRUE);
				return NULL;
			}
		} else if (ferror (f)) {
			fclose (f);
			g_byte_array_free (array, TRUE);
			return NULL;
		}
	}
	
	fclose (f);
	
	return array;
}

#ifdef HAVE_GNOME_VFS

#include <libgnomevfs/gnome-vfs.h>

static GByteArray *
rsvg_acquire_vfs_resource (const char *filename,
						   const char *base_uri,
						   GError    **error)
{
	GByteArray *array;
	
	guchar buffer [4096];
	GnomeVFSFileSize length;
	GnomeVFSHandle *f = NULL;
	GnomeVFSResult res;
	
	g_return_val_if_fail (filename != NULL, NULL);
	g_return_val_if_fail (gnome_vfs_initialized (), NULL);

	res = gnome_vfs_open (&f, filename, GNOME_VFS_OPEN_READ);

	if (res != GNOME_VFS_OK) {
		if (base_uri) {
			GnomeVFSURI * base = gnome_vfs_uri_new (base_uri);
			if (base) {
				GnomeVFSURI * uri = gnome_vfs_uri_resolve_relative (base, filename);
				if (uri) {
					res = gnome_vfs_open_uri (&f, uri, GNOME_VFS_OPEN_READ);
					gnome_vfs_uri_unref (uri);
				}

				gnome_vfs_uri_unref (base);
			}
		}
	}

	if (res != GNOME_VFS_OK) {
		g_set_error (error, rsvg_error_quark (), (gint) res,
					 gnome_vfs_result_to_string (res));
		return NULL;
	}
	
	/* TODO: an optimization is to use the file's size */
	array = g_byte_array_new ();
	
	while (TRUE) {
		res = gnome_vfs_read (f, buffer, sizeof (buffer), &length);
		if (res == GNOME_VFS_OK && length > 0) {
			if (g_byte_array_append (array, buffer, length) == NULL) {
				gnome_vfs_close (f);
				g_byte_array_free (array, TRUE);
				return NULL;
			}
		} else {
			break;
		}
	}
	
	gnome_vfs_close (f);
	
	return array;
}

#endif

GByteArray *
_rsvg_acquire_xlink_href_resource (const char *href,
								   const char *base_uri,
								   GError **err)
{
	GByteArray * arr = NULL;

	if(!strncmp(href, "data:", 5))
		arr = rsvg_acquire_base64_resource (href, err);
	
	if(!arr)
		arr = rsvg_acquire_file_resource (href, base_uri, err);

#ifdef HAVE_GNOME_VFS
	if(!arr)
		arr = rsvg_acquire_vfs_resource (href, base_uri, err);
#endif

	return arr;
}

GdkPixbuf *
rsvg_pixbuf_new_from_href (const char *href,
						   const char *base_uri,
						   GError    **error)
{
	GByteArray * arr;

	arr = _rsvg_acquire_xlink_href_resource (href, base_uri, error);
	if (arr) {
		GdkPixbufLoader *loader;
		GdkPixbuf * pixbuf = NULL;
		int res;

		loader = gdk_pixbuf_loader_new ();
	
		res = gdk_pixbuf_loader_write (loader, arr->data, arr->len, error);
		g_byte_array_free (arr, TRUE);

		if (!res) {
			gdk_pixbuf_loader_close (loader, NULL);
			g_object_unref (loader);
			return NULL;
		}
		
		if (!gdk_pixbuf_loader_close (loader, error)) {
			g_object_unref (loader);
			return NULL;
		}
	
		pixbuf = gdk_pixbuf_loader_get_pixbuf (loader);
	
		if (!pixbuf) {
			g_object_unref (loader);
			g_set_error (error,
						 GDK_PIXBUF_ERROR,
						 GDK_PIXBUF_ERROR_FAILED,
						 _("Failed to load image '%s': reason not known, probably a corrupt image file"),
						 href);
			return NULL;
		}
		
		g_object_ref (pixbuf);
		
		g_object_unref (loader);

		return pixbuf;
	}

	return NULL;
}

void
rsvg_preserve_aspect_ratio(unsigned int aspect_ratio, double width, 
						   double height, double * w, double * h,
						   double * x, double * y)
{
	double neww, newh;
	if (aspect_ratio)
		{
			neww = *w;
			newh = *h; 
			if ((height * *w > width * *h) == 
				((aspect_ratio & RSVG_ASPECT_RATIO_SLICE) == 0))
				{
					neww = width * *h 
						/ height;
				} 
			else 
				{
					newh = height * *w 
						/ width;
				}

			if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
				aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
				aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMAX)
				{}
			else if (aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
					 aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMID ||
					 aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMAX)
				*x -= (neww - *w) / 2;
			else
				*x -= neww - *w;

			if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
				aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
				aspect_ratio & RSVG_ASPECT_RATIO_XMAX_YMIN)
				{}
			else if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
					 aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMID ||
					 aspect_ratio & RSVG_ASPECT_RATIO_XMAX_YMID)
				*y -= (newh - *h) / 2;
			else
				*y -= newh - *h;

			*w = neww;
			*h = newh;
		}
}

static void 
rsvg_node_image_free (RsvgNode * self)
{
	RsvgNodeImage *z = (RsvgNodeImage *)self;
	rsvg_state_finalize (z->super.state);
	g_free(z->super.state);
	g_object_unref (G_OBJECT (z->img)); 
	g_free (z);	
}

static void 
rsvg_node_image_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							   int dominate)
{
	RsvgNodeImage *z = (RsvgNodeImage *)self;
	double x = z->x, y = z->y, w = z->w, h = z->h;
	unsigned int aspect_ratio = z->preserve_aspect_ratio;
	GdkPixbuf *img = z->img;

	rsvg_state_reinherit_top(ctx, z->super.state, dominate);

	rsvg_push_discrete_layer(ctx);

	if (!z->overflow && (aspect_ratio & RSVG_ASPECT_RATIO_SLICE)){
		rsvg_add_clipping_rect(ctx, x, y, w, h);
	}

	rsvg_preserve_aspect_ratio(aspect_ratio, (double)gdk_pixbuf_get_width(img),
							   (double)gdk_pixbuf_get_height(img), &w, &h,
							   &x, &y);

	rsvg_render_image(ctx, img, x, y, w, h);

	rsvg_pop_discrete_layer(ctx);
}

static void
rsvg_node_image_set_atts (RsvgNode *self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double font_size;
	const char * klazz = NULL, * id = NULL, *value;
	GdkPixbuf *img;
	GError *err = NULL;
	RsvgNodeImage *image = (RsvgNodeImage *)self;

	font_size = rsvg_state_current_font_size(ctx);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				image->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				image->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				image->w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				image->h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			/* path is used by some older adobe illustrator versions */
			if ((value = rsvg_property_bag_lookup (atts, "path")) || (value = rsvg_property_bag_lookup (atts, "xlink:href")))
				{
					img = rsvg_pixbuf_new_from_href (value, 
													 rsvg_handle_get_base_uri (ctx), 
													 NULL); 
					
					if (!img)
						{
							if (err)
								{
									g_warning (_("Couldn't load image: %s\n"), err->message);
									g_error_free (err);
								}
							return;
						}
					image->img = img;
				}
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, id, &image->super);
				}
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
				image->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);
			if ((value = rsvg_property_bag_lookup (atts, "overflow")))
				image->overflow = rsvg_css_parse_overflow(value);

			rsvg_parse_style_attrs (ctx, image->super.state, "image", klazz, id, atts);
		}	
}

RsvgNode *
rsvg_new_image (void)
{
	RsvgNodeImage *image;
	image = g_new (RsvgNodeImage, 1);
	image->img = NULL;
	image->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	image->x = 0;
	image->y = 0;
	image->w = -1;
	image->h = -1;
	image->overflow = FALSE;
	image->super.state = g_new(RsvgState, 1);
	rsvg_state_init(image->super.state);
	image->super.type = RSVG_NODE_PATH;
	image->super.free = rsvg_node_image_free;
	image->super.draw = rsvg_node_image_draw;
	image->super.set_atts = rsvg_node_image_set_atts;
	return &image->super;
}
