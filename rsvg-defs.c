/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
#include "rsvg-gz.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"

#include <glib/ghash.h>
#include <glib/gmem.h>
#include <glib/gstrfuncs.h>
#include <glib/gmessages.h>

struct _RsvgDefs {
	GHashTable *hash;
	GPtrArray *unnamed;
	GHashTable *externs;
	gchar * base_uri;
};

RsvgDefs *
rsvg_defs_new (void)
{
	RsvgDefs *result = g_new (RsvgDefs, 1);
	
	result->hash = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
	result->externs = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, NULL);
	result->unnamed = g_ptr_array_new ();
	result->base_uri = NULL;

	return result;
}

void
rsvg_defs_set_base_uri (RsvgDefs * self, gchar * base_uri)
{
	self->base_uri = base_uri;
}


#define SVG_BUFFER_SIZE (1024 * 8)

static int
rsvg_defs_load_extern(const RsvgDefs *defs, const char *name)
{
	RsvgHandle * handle;
	guchar chars[SVG_BUFFER_SIZE];
	int result;
	gchar * filename;
	if (defs->base_uri)
		filename = rsvg_get_file_path(name,defs->base_uri); 
	else
		filename = g_strdup(name);

	FILE *f = fopen (filename, "rb");
	if (!f)
		{
			printf("file: %s not found\n", filename );
			g_free(filename);
			return 1;
		}
	result = fread (chars, 1, SVG_BUFFER_SIZE, f);

	/* test for GZ marker */
	if ((result >= 2) && (chars[0] == (guchar)0x1f) && (chars[1] == (guchar)0x8b))
		handle = rsvg_handle_new_gz ();
	else
		handle = rsvg_handle_new ();

	if (!handle)
		{
			g_free(filename);
			return 1;
		}

	rsvg_handle_set_base_uri (handle, name);

	rsvg_handle_write (handle, chars, result, NULL);

	while (!feof(f) && ((result = fread (chars, 1, SVG_BUFFER_SIZE, f)) > 0))
		rsvg_handle_write (handle, chars, result, NULL);
	
	rsvg_handle_close (handle, NULL);
	
	g_hash_table_insert (defs->externs, g_strdup (name), handle);

	g_free(filename);
	return 0;
}

static RsvgDefVal *
rsvg_defs_extern_lookup (const RsvgDefs *defs, const char *filename, const char *name)
{
	RsvgHandle * file;
	file = (RsvgHandle *)g_hash_table_lookup (defs->externs, filename);
	if (file == NULL)
		{
			if (rsvg_defs_load_extern(defs, filename))
				return NULL;
			file = (RsvgHandle *)g_hash_table_lookup (defs->externs, filename);
		}
	return (RsvgDefVal *)g_hash_table_lookup (file->defs->hash, name);
}

RsvgDefVal *
rsvg_defs_lookup (const RsvgDefs *defs, const char *name)
{
	char * hashpos;
	hashpos = g_strrstr (name, "#");
	if (!hashpos)
		return NULL;
	if (hashpos == name)
		return (RsvgDefVal *)g_hash_table_lookup (defs->hash, name+1);
	else
		{
			gchar ** splitbits;
			RsvgDefVal * toreturn;
			splitbits = g_strsplit (name, "#", 2);
			toreturn = rsvg_defs_extern_lookup(defs, splitbits[0], splitbits[1]);
			g_strfreev(splitbits);
			return toreturn;
		}
}

void
rsvg_defs_set (RsvgDefs *defs, const char *name, RsvgDefVal *val)
{
	if (name == NULL)
		;
	else if (name[0] == '\0')
		;
	else
		g_hash_table_insert (defs->hash, g_strdup (name), val);
	g_ptr_array_add(defs->unnamed, val);
}

void
rsvg_defs_free (RsvgDefs *defs)
{
	guint i;

	g_hash_table_destroy (defs->hash);

	for (i = 0; i < defs->unnamed->len; i++)
		((RsvgDefVal *)g_ptr_array_index(defs->unnamed, i))->free(g_ptr_array_index(defs->unnamed, i));
	g_ptr_array_free(defs->unnamed, TRUE);

	g_free (defs);
}
