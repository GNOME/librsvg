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

#include <glib/ghash.h>
#include <glib/gmem.h>
#include <glib/gstrfuncs.h>
#include <glib/gmessages.h>

struct _RsvgDefs {
	GHashTable *hash;
	GPtrArray *unnamed;
};

static void
rsvg_defs_free_value (gpointer value)
{
	RsvgDefVal *def_val = (RsvgDefVal *)value;
	def_val->free (def_val);
}

RsvgDefs *
rsvg_defs_new (void)
{
	RsvgDefs *result = g_new (RsvgDefs, 1);
	
	result->hash = g_hash_table_new_full (g_str_hash, g_str_equal, g_free, rsvg_defs_free_value);
	result->unnamed = g_ptr_array_new ();
	
	return result;
}

RsvgDefVal *
rsvg_defs_lookup (const RsvgDefs *defs, const char *name)
{
	return (RsvgDefVal *)g_hash_table_lookup (defs->hash, name);
}

void
rsvg_defs_set (RsvgDefs *defs, const char *name, RsvgDefVal *val)
{
	if (name == NULL)
		g_ptr_array_add(defs->unnamed, val);
	else if (name[0] == '\0')
		g_ptr_array_add(defs->unnamed, val);
	else
		g_hash_table_insert (defs->hash, g_strdup (name), val);
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
