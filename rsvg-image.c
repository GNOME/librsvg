/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

#include "config.h"

#include "rsvg-image.h"
#include <string.h>
#include <math.h>
#include <errno.h>
#include "rsvg-css.h"
#include <gio/gio.h>

static GByteArray *
rsvg_acquire_base64_resource (const char *data, GError ** error)
{
    GByteArray *array = NULL;
    gsize data_len, written_len;
    int state = 0;
    guint save = 0;

    rsvg_return_val_if_fail (data != NULL, NULL, error);

    while (*data)
        if (*data++ == ',')
            break;

    data_len = strlen (data);
    array = g_byte_array_sized_new (data_len / 4 * 3);
    written_len = g_base64_decode_step (data, data_len, array->data,
                                        &state, &save);
    g_byte_array_set_size (array, written_len);

    return array;
}

gchar *
rsvg_get_file_path (const gchar * filename, const gchar * base_uri)
{
    gchar *absolute_filename;

    if (g_file_test (filename, G_FILE_TEST_EXISTS) || g_path_is_absolute (filename)) {
        absolute_filename = g_strdup (filename);
    } else {
        gchar *tmpcdir;
        gchar *base_filename;

        if (base_uri) {
            base_filename = g_filename_from_uri (base_uri, NULL, NULL);
            if (base_filename != NULL) {
                tmpcdir = g_path_get_dirname (base_filename);
                g_free (base_filename);
            } else 
                return NULL;
        } else
            tmpcdir = g_get_current_dir ();

        absolute_filename = g_build_filename (tmpcdir, filename, NULL);
        g_free (tmpcdir);
    }

    return absolute_filename;
}

static GByteArray *
rsvg_acquire_file_resource (const char *filename, const char *base_uri, GError ** error)
{
    GByteArray *array;
    gchar *path;
    gchar *data = NULL;
    gsize length;

    rsvg_return_val_if_fail (filename != NULL, NULL, error);

    path = rsvg_get_file_path (filename, base_uri);
    if (path == NULL)
        return NULL;

    if (!g_file_get_contents (path, &data, &length, error)) {
        g_free (path);
        return NULL;
    }

    array = g_byte_array_new ();

    g_byte_array_append (array, (guint8 *)data, length);
    g_free (data);
    g_free (path);

    return array;
}

static GByteArray *
rsvg_acquire_vfs_resource (const char *filename, const char *base_uri, GError ** error)
{
    GByteArray *array;

    GFile *file;
    char *data;
    gsize size;
    gboolean res = FALSE;

    rsvg_return_val_if_fail (filename != NULL, NULL, error);

    file = g_file_new_for_uri (filename);

    if (!(res = g_file_load_contents (file, NULL, &data, &size, NULL, error))) {
        if (base_uri != NULL) {
            GFile *base;

            g_clear_error (error);

            g_object_unref (file);

            base = g_file_new_for_uri (base_uri);
            file = g_file_resolve_relative_path (base, filename);
            g_object_unref (base);

            res = g_file_load_contents (file, NULL, &data, &size, NULL, error);
        }
    }

    g_object_unref (file);

    if (res) {
        array = g_byte_array_new ();

        g_byte_array_append (array, (guint8 *)data, size);
        g_free (data);
    } else {
        return NULL;
    }

    return array;
}

GByteArray *
_rsvg_acquire_xlink_href_resource (const char *href, const char *base_uri, GError ** err)
{
    GByteArray *arr = NULL;

    if (!(href && *href))
        return NULL;

    if (!strncmp (href, "data:", 5))
        arr = rsvg_acquire_base64_resource (href, NULL);

    if (!arr)
        arr = rsvg_acquire_file_resource (href, base_uri, NULL);

    if (!arr)
        arr = rsvg_acquire_vfs_resource (href, base_uri, NULL);

    return arr;
}

GdkPixbuf *
rsvg_pixbuf_new_from_href (const char *href, const char *base_uri, GError ** error)
{
    GByteArray *arr;

    arr = _rsvg_acquire_xlink_href_resource (href, base_uri, error);
    if (arr) {
        GdkPixbufLoader *loader;
        GdkPixbuf *pixbuf = NULL;
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
                         _
                         ("Failed to load image '%s': reason not known, probably a corrupt image file"),
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
rsvg_preserve_aspect_ratio (unsigned int aspect_ratio, double width,
                            double height, double *w, double *h, double *x, double *y)
{
    double neww, newh;
    if (aspect_ratio & ~RSVG_ASPECT_RATIO_SLICE) {
        neww = *w;
        newh = *h;
        if ((height * *w > width * *h) == ((aspect_ratio & RSVG_ASPECT_RATIO_SLICE) == 0)) {
            neww = width * *h / height;
        } else {
            newh = height * *w / width;
        }

        if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMAX) {
        } else if (aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMID ||
                   aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMAX)
            *x -= (neww - *w) / 2;
        else
            *x -= neww - *w;

        if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMID_YMIN ||
            aspect_ratio & RSVG_ASPECT_RATIO_XMAX_YMIN) {
        } else if (aspect_ratio & RSVG_ASPECT_RATIO_XMIN_YMID ||
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
    RsvgNodeImage *z = (RsvgNodeImage *) self;
    rsvg_state_finalize (z->super.state);
    g_free (z->super.state);
    z->super.state = NULL;
    if (z->img)
        g_object_unref (z->img);
    _rsvg_node_free(self);
}

static void
rsvg_node_image_draw (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate)
{
    RsvgNodeImage *z = (RsvgNodeImage *) self;
    unsigned int aspect_ratio = z->preserve_aspect_ratio;
    GdkPixbuf *img = z->img;
    gdouble x, y, w, h;

    if (img == NULL)
        return;

    x = _rsvg_css_normalize_length (&z->x, ctx, 'h');
    y = _rsvg_css_normalize_length (&z->y, ctx, 'v');
    w = _rsvg_css_normalize_length (&z->w, ctx, 'h');
    h = _rsvg_css_normalize_length (&z->h, ctx, 'v');

    rsvg_state_reinherit_top (ctx, z->super.state, dominate);

    rsvg_push_discrete_layer (ctx);

    if (!rsvg_current_state (ctx)->overflow && (aspect_ratio & RSVG_ASPECT_RATIO_SLICE)) {
        rsvg_add_clipping_rect (ctx, x, y, w, h);
    }

    rsvg_preserve_aspect_ratio (aspect_ratio, (double) gdk_pixbuf_get_width (img),
                                (double) gdk_pixbuf_get_height (img), &w, &h, &x, &y);

    rsvg_render_image (ctx, img, x, y, w, h);

    rsvg_pop_discrete_layer (ctx);
}

static void
rsvg_node_image_set_atts (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    const char *klazz = NULL, *id = NULL, *value;
    RsvgNodeImage *image = (RsvgNodeImage *) self;

    if (rsvg_property_bag_size (atts)) {
        if ((value = rsvg_property_bag_lookup (atts, "x")))
            image->x = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "y")))
            image->y = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "width")))
            image->w = _rsvg_css_parse_length (value);
        if ((value = rsvg_property_bag_lookup (atts, "height")))
            image->h = _rsvg_css_parse_length (value);
        /* path is used by some older adobe illustrator versions */
        if ((value = rsvg_property_bag_lookup (atts, "path"))
            || (value = rsvg_property_bag_lookup (atts, "xlink:href"))) {
            image->img = rsvg_pixbuf_new_from_href (value, rsvg_handle_get_base_uri (ctx), NULL);

            if (!image->img) {
#ifdef G_ENABLE_DEBUG
                g_warning (_("Couldn't load image: %s\n"), value);
#endif
            }
        }
        if ((value = rsvg_property_bag_lookup (atts, "class")))
            klazz = value;
        if ((value = rsvg_property_bag_lookup (atts, "id"))) {
            id = value;
            rsvg_defs_register_name (ctx->priv->defs, id, &image->super);
        }
        if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
            image->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);

        rsvg_parse_style_attrs (ctx, image->super.state, "image", klazz, id, atts);
    }
}

RsvgNode *
rsvg_new_image (void)
{
    RsvgNodeImage *image;
    image = g_new (RsvgNodeImage, 1);
    _rsvg_node_init (&image->super);
    g_assert (image->super.state);
    image->img = NULL;
    image->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
    image->x = image->y = image->w = image->h = _rsvg_css_parse_length ("0");
    image->super.free = rsvg_node_image_free;
    image->super.draw = rsvg_node_image_draw;
    image->super.set_atts = rsvg_node_image_set_atts;
    return &image->super;
}
