/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*- */
/* GdkPixbuf library - SVG image loader
 *
 * Copyright (C) 2002 Matthias Clasen
 * Copyright (C) 2002 Dom Lachowicz
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
        context->size_func     = size_func;

        context->prepared_func = prepared_func;
        context->updated_func  = updated_func;
        context->user_data     = user_data;

        return context;
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

                if (!context->handle)
                        return FALSE;

                rsvg_handle_set_size_callback (context->handle, context->size_func, context->user_data, NULL);
        }

        if (!rsvg_handle_write (context->handle, buf, size, error)) {
                rsvg_propegate_error (error, _("Error writing"), ERROR_WRITING);
                return FALSE;
        }

        context->pixbuf = rsvg_handle_get_pixbuf (context->handle);
  
        if (context->pixbuf != NULL && context->prepared_func != NULL)
                (* context->prepared_func) (context->pixbuf, NULL, context->user_data);        
  
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

        if (context->pixbuf == NULL) {
                context->pixbuf = rsvg_handle_get_pixbuf (context->handle);
    
                if (context->pixbuf != NULL && context->prepared_func != NULL)
                        (* context->prepared_func) (context->pixbuf, NULL, context->user_data);
        }

        if (context->pixbuf != NULL && context->updated_func != NULL)
                (* context->updated_func) (context->pixbuf, 
                                           0, 0, 
                                           gdk_pixbuf_get_width (context->pixbuf), 
                                           gdk_pixbuf_get_height (context->pixbuf), 
                                           context->user_data);
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
        module->begin_load     = gdk_pixbuf__svg_image_begin_load;
        module->stop_load      = gdk_pixbuf__svg_image_stop_load;
        module->load_increment = gdk_pixbuf__svg_image_load_increment;
}

void
fill_info (GdkPixbufFormat *info)
{
        static GdkPixbufModulePattern signature[] = {
                { "<?xml", NULL, 50 },
                { "<svg", NULL, 100 },
                { "<!DOCTYPE svg", NULL, 100 },
                { NULL, NULL, 0 }
        };
        static gchar *mime_types[] = { 
                "image/svg", 
                "image/svg+xml",
                NULL 
        };
        static gchar *extensions[] = { 
                "svg", 
                "svgz",
                NULL 
        };
        
        info->name        = "svg";
        info->signature   = signature;
        info->description = _("Scalable Vector Graphics");
        info->mime_types  = mime_types;
        info->extensions  = extensions;
        info->flags       = 0;
}
