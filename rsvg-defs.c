/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/* 
   rsvg-defs.c: Manage SVG defs and references.
 
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

#include "config.h"
#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "rsvg-styles.h"
#include "rsvg-io.h"

#include <glib.h>

struct _RsvgDefs {
    GHashTable *hash;
    GPtrArray *unnamed;
    GHashTable *externs;
    RsvgHandle *ctx;
};

RsvgDefs *
rsvg_defs_new (RsvgHandle *handle)
{
    RsvgDefs *result = g_new (RsvgDefs, 1);

    result->hash = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
    result->externs =
        g_hash_table_new_full (g_str_hash, g_str_equal, g_free, (GDestroyNotify) g_object_unref);
    result->unnamed = g_ptr_array_new ();
    result->ctx = handle; /* no need to take a ref here */

    return result;
}

static RsvgHandle *
rsvg_defs_load_extern (const RsvgDefs * defs, const char *uri)
{
    RsvgHandle *handle = NULL;
    char *data;
    gsize data_len;

    data = _rsvg_handle_acquire_data (defs->ctx, uri, NULL, &data_len, NULL);

    if (data) {
        handle = rsvg_handle_new ();
        rsvg_handle_set_base_uri (handle, uri);

        if (rsvg_handle_write (handle, (guchar *) data, data_len, NULL)
            && rsvg_handle_close (handle, NULL)) {
            g_hash_table_insert (defs->externs, g_strdup (uri), handle);
        } else {
            g_object_unref (handle);
            handle = NULL;
        }

        g_free (data);
    }

    return handle;
}

static RsvgNode *
rsvg_defs_extern_lookup (const RsvgDefs * defs, const char *possibly_relative_uri, const char *name)
{
    RsvgHandle *handle;
    char *uri;

    uri = rsvg_handle_resolve_uri (defs->ctx, possibly_relative_uri);
    if (!uri)
        return NULL;

    handle = (RsvgHandle *) g_hash_table_lookup (defs->externs, uri);
    if (handle == NULL) {
        handle = rsvg_defs_load_extern (defs, uri);
    }

    if (handle != NULL)
        return g_hash_table_lookup (handle->priv->defs->hash, name);
    else
        return NULL;
}

RsvgNode *
rsvg_defs_lookup (const RsvgDefs * defs, const char *name)
{
    char *hashpos;
    hashpos = g_strrstr (name, "#");
    if (!hashpos) {
        return NULL;
    }
    if (hashpos == name) {
        return g_hash_table_lookup (defs->hash, name + 1);
    } else {
        gchar **splitbits;
        RsvgNode *toreturn;
        splitbits = g_strsplit (name, "#", 2);
        toreturn = rsvg_defs_extern_lookup (defs, splitbits[0], splitbits[1]);
        g_strfreev (splitbits);
        return toreturn;
    }
}

void
rsvg_defs_set (RsvgDefs * defs, const char *name, RsvgNode * val)
{
    if (name == NULL);
    else if (name[0] == '\0');
    else
        rsvg_defs_register_name (defs, name, val);
    rsvg_defs_register_memory (defs, val);
}

void
rsvg_defs_register_name (RsvgDefs * defs, const char *name, RsvgNode * val)
{
    if (g_hash_table_lookup (defs->hash, name))
        return;

    g_hash_table_insert (defs->hash, g_strdup (name), val);
}

void
rsvg_defs_register_memory (RsvgDefs * defs, RsvgNode * val)
{
    g_ptr_array_add (defs->unnamed, val);
}

void
rsvg_defs_free (RsvgDefs * defs)
{
    guint i;

    g_hash_table_destroy (defs->hash);

    for (i = 0; i < defs->unnamed->len; i++)
        ((RsvgNode *) g_ptr_array_index (defs->unnamed, i))->
            free (g_ptr_array_index (defs->unnamed, i));
    g_ptr_array_free (defs->unnamed, TRUE);

    g_hash_table_destroy (defs->externs);

    g_free (defs);
}

