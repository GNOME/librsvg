/* -*- Mode: C; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 8 -*- */
/* GdkPixbuf library - SVG image loader
 *
 * Copyright (C) 2002 Matthias Clasen
 *
 * Authors: Matthias Clasen <maclas@gmx.de>
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

#include <config.h>
#include <stdio.h>
#include <stdlib.h>
#include <rsvg.h>
#include <gdk-pixbuf/gdk-pixbuf.h>
#include <gdk-pixbuf/gdk-pixbuf-io.h>

typedef struct {
        RsvgHandle                 *handle;
        GdkPixbuf                  *pixbuf;
        GdkPixbufModuleUpdatedFunc  updated_func;
        GdkPixbufModulePreparedFunc prepared_func;
        gpointer                    user_data;
} SvgContext;

G_MODULE_EXPORT void fill_vtable (GdkPixbufModule *module);
G_MODULE_EXPORT void fill_info (GdkPixbufFormat *info);

static gpointer
gdk_pixbuf__svg_image_begin_load (GdkPixbufModuleSizeFunc size_func,
                                  GdkPixbufModulePreparedFunc prepared_func, 
                                  GdkPixbufModuleUpdatedFunc  updated_func,
                                  gpointer user_data,
                                  GError **error)
{
        SvgContext *context = g_new0 (SvgContext, 1);

        context->handle = rsvg_handle_new ();
        rsvg_handle_set_size_callback (context->handle, size_func, user_data, NULL);
        context->prepared_func = prepared_func;
        context->updated_func = updated_func;
        context->user_data = user_data;

        return context;
}


static gboolean
gdk_pixbuf__svg_image_load_increment (gpointer data,
				      const guchar *buf, guint size,
				      GError **error)
{
        SvgContext *context = (SvgContext *)data;
        gboolean result;
  
        result = rsvg_handle_write (context->handle, buf, size, error);
  
        context->pixbuf = rsvg_handle_get_pixbuf (context->handle);
  
        if (context->pixbuf != NULL && context->prepared_func != NULL) {
                (* context->prepared_func) (context->pixbuf, NULL, context->user_data);
        }
  
        return result;
}


static gboolean
gdk_pixbuf__svg_image_stop_load (gpointer data, GError **error)
{
        SvgContext *context = (SvgContext *)data;  
        rsvg_handle_close (context->handle, error);

        if (context->pixbuf == NULL) {
                context->pixbuf = rsvg_handle_get_pixbuf (context->handle);
    
                if (context->pixbuf != NULL && context->prepared_func != NULL) {
                        (* context->prepared_func) (context->pixbuf, NULL, context->user_data);
                }
        }

        if (context->pixbuf != NULL && context->updated_func != NULL) {
                (* context->updated_func) (context->pixbuf, 
                                           0, 0, 
                                           gdk_pixbuf_get_width (context->pixbuf), 
                                           gdk_pixbuf_get_height (context->pixbuf), 
                                           context->user_data);
        }

        rsvg_handle_free (context->handle);
        g_object_unref (context->pixbuf);
        g_free (context);

        return TRUE;
}

void
fill_vtable (GdkPixbufModule *module)
{
        module->begin_load = gdk_pixbuf__svg_image_begin_load;
        module->stop_load = gdk_pixbuf__svg_image_stop_load;
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
                NULL 
        };
        
        info->name = "svg";
        info->signature = signature;
        info->description = "Scalable Vector Graphics";
        info->mime_types = mime_types;
        info->extensions = extensions;
        info->flags = 0;
}
