/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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
    GHashTable *externs;
    RsvgHandle *handle;
};

RsvgDefs *
rsvg_defs_new (RsvgHandle *handle)
{
    RsvgDefs *result = g_new0 (RsvgDefs, 1);

    result->hash = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, (GDestroyNotify) rsvg_node_unref);
    result->externs =
        g_hash_table_new_full (g_str_hash, g_str_equal, g_free, (GDestroyNotify) g_object_unref);
    result->handle = handle; /* no need to take a ref here */

    return result;
}

static RsvgNode *
rsvg_defs_extern_lookup (const RsvgDefs * defs, const char *possibly_relative_uri, const char *name)
{
    RsvgHandle *handle;
    char *uri;

    uri = rsvg_handle_resolve_uri (defs->handle, possibly_relative_uri);
    if (!uri)
        return NULL;

    handle = (RsvgHandle *) g_hash_table_lookup (defs->externs, uri);
    if (handle == NULL) {
        handle = rsvg_handle_load_extern (defs->handle, uri);
        if (handle != NULL) {
            g_hash_table_insert (defs->externs, g_strdup (uri), handle);
        }
    }

    if (handle != NULL) {
        RsvgDefs *ext_defs = rsvg_handle_get_defs (handle);
        return g_hash_table_lookup (ext_defs->hash, name);
    }

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
rsvg_defs_register_node_by_id (RsvgDefs *defs, const char *id, RsvgNode *node)
{
    g_assert (defs != NULL);
    g_assert (id != NULL);
    g_assert (node != NULL);

    if (g_hash_table_lookup (defs->hash, id))
        return;

    g_hash_table_insert (defs->hash, g_strdup (id), rsvg_node_ref (node));
}

void
rsvg_defs_free (RsvgDefs * defs)
{
    g_hash_table_destroy (defs->hash);
    defs->hash = NULL;

    g_hash_table_destroy (defs->externs);
    defs->externs = NULL;

    g_free (defs);
}

