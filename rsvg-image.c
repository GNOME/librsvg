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
#include "rsvg-filter.h"
#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_rgb_svp.h>
#include "rsvg-css.h"
#include "rsvg-mask.h"
/*very art dependant at the moment*/
#include "rsvg-art-composite.h"
#include "rsvg-art-render.h"

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

	if (g_path_is_absolute(filename)) {
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
		if (length > 0)
			if (g_byte_array_append (array, buffer, length) == NULL) {
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
#else

	if(!arr)
		arr = rsvg_acquire_file_resource (href, base_uri, err);

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
rsvg_affine_image(GdkPixbuf *img, GdkPixbuf *intermediate, 
				  double * affine, double w, double h)
{
	gdouble tmp_affine[6];
	gdouble inv_affine[6];
	gdouble raw_inv_affine[6];
	gint intstride;
	gint basestride;	
	gint basex, basey;
	gdouble fbasex, fbasey;
	gdouble rawx, rawy;
	guchar * intpix;
	guchar * basepix;
	gint i, j, k, basebpp, ii, jj;
	gboolean has_alpha;
	gdouble pixsum[4];
	gboolean xrunnoff, yrunnoff;
	gint iwidth, iheight;
	gint width, height;

	width = gdk_pixbuf_get_width (img);
	height = gdk_pixbuf_get_height (img);
	iwidth = gdk_pixbuf_get_width (intermediate);
	iheight = gdk_pixbuf_get_height (intermediate);

	has_alpha = gdk_pixbuf_get_has_alpha (img);

	basestride = gdk_pixbuf_get_rowstride (img);
	intstride = gdk_pixbuf_get_rowstride (intermediate);
	basepix = gdk_pixbuf_get_pixels (img);
	intpix = gdk_pixbuf_get_pixels (intermediate);
	basebpp = has_alpha ? 4 : 3;

	art_affine_invert(raw_inv_affine, affine);

	/*scale to w and h*/
	tmp_affine[0] = (double)w;
	tmp_affine[3] = (double)h;
	tmp_affine[1] = tmp_affine[2] = tmp_affine[4] = tmp_affine[5] = 0;
	art_affine_multiply(tmp_affine, tmp_affine, affine);

	art_affine_invert(inv_affine, tmp_affine);


	/*apply the transformation*/
	for (i = 0; i < iwidth; i++)
		for (j = 0; j < iheight; j++)		
			{
				fbasex = (inv_affine[0] * (double)i + inv_affine[2] * (double)j + 
						  inv_affine[4]) * (double)width;
				fbasey = (inv_affine[1] * (double)i + inv_affine[3] * (double)j + 
						  inv_affine[5]) * (double)height;
				basex = floor(fbasex);
				basey = floor(fbasey);
				rawx = raw_inv_affine[0] * i + raw_inv_affine[2] * j + 
					raw_inv_affine[4];
				rawy = raw_inv_affine[1] * i + raw_inv_affine[3] * j + 
					raw_inv_affine[5];
				if (rawx < 0 || rawy < 0 || rawx >= w || 
					rawy >= h || basex < 0 || basey < 0 
					|| basex >= width || basey >= height)
					{					
						for (k = 0; k < 4; k++)
							intpix[i * 4 + j * intstride + k] = 0;
					}
				else
					{
						if (basex < 0 || basex + 1 >= width)
							xrunnoff = TRUE;
						else
							xrunnoff = FALSE;
						if (basey < 0 || basey + 1 >= height)
							yrunnoff = TRUE;
						else
							yrunnoff = FALSE;
						for (k = 0; k < basebpp; k++)
							pixsum[k] = 0;
						for (ii = 0; ii < 2; ii++)
							for (jj = 0; jj < 2; jj++)
								{
									if (basex + ii < 0 || basey + jj< 0 
										|| basex + ii >= width || basey + jj >= height)
										;
									else
										{
											for (k = 0; k < basebpp; k++)
												{
													pixsum[k] += 
														(double)basepix[basebpp * (basex + ii) + (basey + jj) * basestride + k] 
														* (xrunnoff ? 1 : fabs(fbasex - (double)(basex + (1 - ii))))
														* (yrunnoff ? 1 : fabs(fbasey - (double)(basey + (1 - jj))));
												}
										}
								}
						for (k = 0; k < basebpp; k++)
							intpix[i * 4 + j * intstride + k] = pixsum[k];
						if (!has_alpha)
							intpix[i * 4 + j * intstride + 3] = 255;
					}	

			}
}

void rsvg_clip_image(GdkPixbuf *intermediate, ArtSVP *path);

void
rsvg_clip_image(GdkPixbuf *intermediate, ArtSVP *path)
{
	gint intstride;
	gint basestride;	
	guchar * intpix;
	guchar * basepix;
	gint i, j;
	gint width, height;
	GdkPixbuf * base;

	width = gdk_pixbuf_get_width (intermediate);
	height = gdk_pixbuf_get_height (intermediate);

	intstride = gdk_pixbuf_get_rowstride (intermediate);
	intpix = gdk_pixbuf_get_pixels (intermediate);

	base = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 0, 8, 
						   width, height);
	basestride = gdk_pixbuf_get_rowstride (base);
	basepix = gdk_pixbuf_get_pixels (base);
	
	art_rgb_svp_aa(path, 0, 0, width, height, 0xFFFFFF, 0x000000, basepix, basestride, NULL);

	for (i = 0; i < width; i++)
		for (j = 0; j < height; j++)		
			{
				intpix[i * 4 + j * intstride + 3] = intpix[i * 4 + j * intstride + 3] * 
					basepix[i * 3 + j * basestride] / 255;
			}
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
			if (height * *w >
				width * *h != (aspect_ratio & RSVG_ASPECT_RATIO_SLICE))
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
rsvg_defs_drawable_image_free (RsvgDefVal * self)
{
	RsvgDefsDrawableImage *z = (RsvgDefsDrawableImage *)self;
	rsvg_state_finalize (&z->super.state);
	g_object_unref (G_OBJECT (z->img)); 
	g_free (z);	
}

static void 
rsvg_defs_drawable_image_draw (RsvgDefsDrawable * self, RsvgDrawingCtx *ctx, 
							   int dominate)
{
	RsvgDefsDrawableImage *z = (RsvgDefsDrawableImage *)self;
	double x = z->x, y = z->y, w = z->w, h = z->h;
	unsigned int aspect_ratio = z->preserve_aspect_ratio;
	ArtIRect temprect;
	GdkPixbuf *img = z->img;
	int i, j;
	double tmp_affine[6];
	double tmp_tmp_affine[6];
	RsvgState *state = rsvg_state_current(ctx);
	GdkPixbuf *intermediate;
	double basex, basey;
	ArtSVP * temppath;
	/*this will have to change*/
	GdkPixbuf * pixbuf = ((RsvgArtRender *)ctx->render)->pixbuf;

	rsvg_state_reinherit_top(ctx, &self->state, dominate);

	for (i = 0; i < 6; i++)
		tmp_affine[i] = state->affine[i];

	if (!z->overflow && (aspect_ratio & RSVG_ASPECT_RATIO_SLICE)){
		temppath = rsvg_rect_clip_path(x, y, w, h, ctx);
		state->clip_path_loaded = TRUE;
		state->clippath = rsvg_clip_path_merge(temppath,
											   state->clippath, 'i');
	}

	rsvg_preserve_aspect_ratio(aspect_ratio, (double)gdk_pixbuf_get_width(img),
							   (double)gdk_pixbuf_get_height(img), &w, &h,
							   &x, &y);

	/*translate to x and y*/
	tmp_tmp_affine[0] = tmp_tmp_affine[3] = 1;
	tmp_tmp_affine[1] = tmp_tmp_affine[2] = 0;
	tmp_tmp_affine[4] = x;
	tmp_tmp_affine[5] = y;

	art_affine_multiply(tmp_affine, tmp_tmp_affine, tmp_affine);

	intermediate = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
								   gdk_pixbuf_get_width (pixbuf),
								   gdk_pixbuf_get_height (pixbuf));

	if (!intermediate)
		{
			g_object_unref (G_OBJECT (img));
			return;
		}

	rsvg_affine_image(img, intermediate, tmp_affine, w, h);

	rsvg_push_discrete_layer(ctx);

	if (state->clippath)
		{
			rsvg_clip_image(intermediate, state->clippath);
		}

	/*slap it down*/
	rsvg_alpha_blt (intermediate, 0, 0,
					gdk_pixbuf_get_width (intermediate),
					gdk_pixbuf_get_height (intermediate),
					pixbuf, 
					0, 0);
	
	temprect.x0 = gdk_pixbuf_get_width (intermediate);
	temprect.y0 = gdk_pixbuf_get_height (intermediate);
	temprect.x1 = 0;
	temprect.y1 = 0;

	for (i = 0; i < 2; i++)
		for (j = 0; j < 2; j++)
			{
				basex = tmp_affine[0] * w * i + tmp_affine[2] * h * j + tmp_affine[4];
				basey = tmp_affine[1] * w * i + tmp_affine[3] * h * j + tmp_affine[5];
				temprect.x0 = MIN(basex, temprect.x0);
				temprect.y0 = MIN(basey, temprect.y0);
				temprect.x1 = MAX(basex, temprect.x1);
				temprect.y1 = MAX(basey, temprect.y1);
			}

	art_irect_union(&ctx->bbox, &ctx->bbox, &temprect);
	rsvg_pop_discrete_layer(ctx);

	g_object_unref (G_OBJECT (intermediate));
}

void
rsvg_start_image (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x = 0., y = 0., w = -1., h = -1., font_size;
	const char * href = NULL;
	const char * klazz = NULL, * id = NULL, *value;
	int aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	GdkPixbuf *img;
	GError *err = NULL;
	RsvgState state;
	RsvgDefsDrawableImage *image;
	gboolean overflow = FALSE;
	rsvg_state_init(&state);
	font_size = rsvg_state_current_font_size(ctx);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			/* path is used by some older adobe illustrator versions */
			if ((value = rsvg_property_bag_lookup (atts, "path")) || (value = rsvg_property_bag_lookup (atts, "xlink:href")))
				href = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
				aspect_ratio = rsvg_css_parse_aspect_ratio (value);
			if ((value = rsvg_property_bag_lookup (atts, "overflow")))
				overflow = rsvg_css_parse_overflow(value);

			rsvg_parse_style_attrs (ctx, &state, "image", klazz, id, atts);
		}
	
	if (!href || w <= 0. || h <= 0.)
		return;   	

	/*hmm, passing the error thingie into the next thing makes it screw up when using vfs*/
	img = rsvg_pixbuf_new_from_href (href, rsvg_handle_get_base_uri (ctx), NULL); 

	if (!img)
		{
			if (err)
				{
					g_warning (_("Couldn't load image: %s\n"), err->message);
					g_error_free (err);
				}
			return;
		}
	
	image = g_new (RsvgDefsDrawableImage, 1);
	image->img = img;
	image->preserve_aspect_ratio = aspect_ratio;
	image->x = x;
	image->y = y;
	image->w = w;
	image->h = h;
	image->overflow = overflow;
	image->super.state = state;
	image->super.super.type = RSVG_DEF_PATH;
	image->super.super.free = rsvg_defs_drawable_image_free;
	image->super.draw = rsvg_defs_drawable_image_draw;
	rsvg_defs_set (ctx->defs, id, &image->super.super);
	
	image->super.parent = (RsvgDefsDrawable *)ctx->current_defs_group;
	if (image->super.parent != NULL)
		rsvg_defs_drawable_group_pack((RsvgDefsDrawableGroup *)image->super.parent, 
									  &image->super);

}
