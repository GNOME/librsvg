/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*- */
/* GdkPixbuf library - SVG image loader
 *
 * Copyright (C) 2002 Matthias Clasen
 * Copyright (C) 2002-2004 Dom Lachowicz
 *
 * Authors: Matthias Clasen <maclas@gmx.de>
 *          Dom Lachowicz <cinamod@hotmail.com>
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more  * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the
 * Free Software Foundation, Inc., 59 Temple Place - Suite 330,
 * Boston, MA 02111-1307, USA.
 */

#include <rsvg.h>
#include <stdlib.h>
#include <gdk-pixbuf/gdk-pixbuf.h>
#include <gdk-pixbuf/gdk-pixbuf-io.h>
#include <rsvg-gz.h>
#include "rsvg-private.h"

typedef struct {
        RsvgHandle                 *handle;
        GdkPixbuf                  *pixbuf;

        GdkPixbufModuleUpdatedFunc  updated_func;
        GdkPixbufModulePreparedFunc prepared_func;
        GdkPixbufModuleSizeFunc     size_func;

        gboolean                    first_write;
        gboolean                    emitted_prepared;

        gpointer                    user_data;
} SvgContext;

G_MODULE_EXPORT void fill_vtable (GdkPixbufModule *module);
G_MODULE_EXPORT void fill_info (GdkPixbufFormat *info);

enum {
        ERROR_WRITING = 1,
        ERROR_DISPLAYING_IMAGE
} RsvgLoaderErrorReasons;

static void
rsvg_propegate_error (GError ** err,
                      const char * reason,
                      gint code)
{
        if (err) {
                *err = NULL;
                g_set_error (err, rsvg_error_quark (), code, reason);
        }
}

static gpointer
gdk_pixbuf__svg_image_begin_load (GdkPixbufModuleSizeFunc size_func,
                                  GdkPixbufModulePreparedFunc prepared_func,
                                  GdkPixbufModuleUpdatedFunc  updated_func,
                                  gpointer user_data,
                                  GError **error)
{
        SvgContext *context    = g_new0 (SvgContext, 1);

        if (error)
                *error = NULL;

        context->first_write   = TRUE;
        context->emitted_prepared = FALSE;
        context->size_func     = size_func;

        context->prepared_func = prepared_func;
        context->updated_func  = updated_func;
        context->user_data     = user_data;

        return context;
}

static void
emit_updated (SvgContext *context)
{
        if (context->pixbuf != NULL && context->updated_func != NULL)
                (* context->updated_func) (context->pixbuf,
                                           0, 0,
                                           gdk_pixbuf_get_width (context->pixbuf),
                                           gdk_pixbuf_get_height (context->pixbuf),
                                           context->user_data);
}

static void
emit_prepared (SvgContext *context)
{
        if (context->pixbuf != NULL && context->prepared_func != NULL && !context->emitted_prepared) {
                (* context->prepared_func) (context->pixbuf, NULL, context->user_data);
                context->emitted_prepared = TRUE;
        }
}

static void
maybe_update (SvgContext *context)
{
        /* TODO: not sure whether we want to emit an "updated" signal every time someone
         * TODO: writes to the loader. yes, the image might have changed some, but does that
         * TODO: warrant emitting progressive drawing signals? */
        if (FALSE)
                emit_updated (context);
}

static gboolean
gdk_pixbuf__svg_image_load_increment (gpointer data,
				      const guchar *buf, guint size,
				      GError **error)
{
        SvgContext *context = (SvgContext *)data;

        if (error)
                *error = NULL;

        if (context->first_write == TRUE) {
                context->first_write = FALSE;

                /* lazy create a SVGZ or SVG loader */
                if ((size >= 2) && (buf[0] == (guchar)0x1f) && (buf[1] == (guchar)0x8b))
                        context->handle = rsvg_handle_new_gz ();
                else
                        context->handle = rsvg_handle_new ();

                if (!context->handle) {
                        rsvg_propegate_error (error, _("Error displaying image"), ERROR_DISPLAYING_IMAGE);
                        return FALSE;
                }

                rsvg_handle_set_size_callback (context->handle, context->size_func, context->user_data, NULL);
        }

        if (!context->handle) {
                rsvg_propegate_error (error, _("Error displaying image"), ERROR_DISPLAYING_IMAGE);
                return FALSE;
        }

        if (!rsvg_handle_write (context->handle, buf, size, error)) {
                rsvg_propegate_error (error, _("Error writing"), ERROR_WRITING);
                return FALSE;
        }

        context->pixbuf = rsvg_handle_get_pixbuf (context->handle);

        emit_prepared (context);
        maybe_update (context);

        return TRUE;
}

static gboolean
gdk_pixbuf__svg_image_stop_load (gpointer data, GError **error)
{
        SvgContext *context = (SvgContext *)data;
        gboolean result = TRUE;

        if (error)
                *error = NULL;

        if (!context->handle) {
                rsvg_propegate_error (error, _("Error displaying image"), ERROR_DISPLAYING_IMAGE);
                return FALSE;
        }

        rsvg_handle_close (context->handle, error);

        if (context->pixbuf == NULL)
                context->pixbuf = rsvg_handle_get_pixbuf (context->handle);

        if (context->pixbuf != NULL) {
                emit_prepared (context);
                emit_updated (context);
        }
        else if (!context->pixbuf) {
                rsvg_propegate_error (error, _("Error displaying image"), ERROR_DISPLAYING_IMAGE);
                result = FALSE;
        }

        rsvg_handle_free (context->handle);
        if (context->pixbuf)
                g_object_unref (context->pixbuf);
        g_free (context);

        return TRUE;
}

void
fill_vtable (GdkPixbufModule *module)
{
        rsvg_init ();

        module->begin_load     = gdk_pixbuf__svg_image_begin_load;
        module->stop_load      = gdk_pixbuf__svg_image_stop_load;
        module->load_increment = gdk_pixbuf__svg_image_load_increment;
}

/* this is present only in GTK+ 2.4 and later. we want librsvg to work with older versions too */
#ifndef GDK_PIXBUF_FORMAT_SCALABLE
#define GDK_PIXBUF_FORMAT_SCALABLE (1 << 1)
#endif

/* this is present only in GTK+ 2.6 and later. we want librsvg to work with older versions too */
#ifndef GDK_PIXBUF_FORMAT_THREADSAFE
#define GDK_PIXBUF_FORMAT_THREADSAFE (1 << 2)
#endif

void
fill_info (GdkPixbufFormat *info)
{
        static GdkPixbufModulePattern signature[] = {
                { "<?xml", NULL, 50 },
                { "<svg", NULL, 100 },
                { "<!DOCTYPE svg", NULL, 100 },
                { NULL, NULL, 0 }
        };
        static gchar *mime_types[] = { /* yes folks, i actually have run into all of these in the wild... */
                "image/svg+xml",
                "image/svg",
                "image/svg-xml",
                "image/vnd.adobe.svg+xml",
                "text/xml-svg",
                NULL
        };
        static gchar *extensions[] = {
                "svg",
                "svgz",
                "svg.gz",
                NULL
        };

        info->name        = "svg";
        info->signature   = signature;
        info->description = _("Scalable Vector Graphics");
        info->mime_types  = mime_types;
        info->extensions  = extensions;
        info->flags       = GDK_PIXBUF_FORMAT_SCALABLE | GDK_PIXBUF_FORMAT_THREADSAFE;
}
