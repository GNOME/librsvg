/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-private.h: Internals of RSVG

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

#ifndef RSVG_PRIVATE_H
#define RSVG_PRIVATE_H

#include "rsvg.h"

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>
#include <pango/pangoft2.h>

G_BEGIN_DECLS

typedef struct RsvgSaxHandler RsvgSaxHandler;
typedef struct _RsvgPropertyBag RsvgPropertyBag;
typedef struct _RsvgState RsvgState;
typedef struct _RsvgDefs RsvgDefs;
typedef struct _RsvgDefVal RsvgDefVal;
typedef struct _RsvgFilter RsvgFilter;

struct RsvgSaxHandler {
	void (*free) (RsvgSaxHandler *self);
	void (*start_element) (RsvgSaxHandler *self, const xmlChar *name, RsvgPropertyBag *atts);
	void (*end_element) (RsvgSaxHandler *self, const xmlChar *name);
	void (*characters) (RsvgSaxHandler *self, const xmlChar *ch, int len);
};

struct RsvgHandle {
	RsvgSizeFunc size_func;
	gpointer user_data;
	GDestroyNotify user_data_destroy;
	GdkPixbuf *pixbuf;
	
	/* stack; there is a state for each element */
	RsvgState *state;
	int n_state;
	int n_state_max;
	
	RsvgDefs *defs;
	gboolean in_defs;
	void *current_defs_group;

	GHashTable *css_props;
	
	/* not a handler stack. each nested handler keeps
	 * track of its parent
	 */
	RsvgSaxHandler *handler;
	int handler_nest;
	
	GHashTable *entities; /* g_malloc'd string -> xmlEntityPtr */
	
	PangoContext *pango_context;
	xmlParserCtxtPtr ctxt;
	GError **error;
	
	int width;
	int height;
	double dpi;
	
	GString * title;
	GString * desc;

	void * currentfilter;
	void * currentsubfilter;

	/* virtual fns */
	gboolean (* write) (RsvgHandle    *handle,
						const guchar  *buf,
						gsize          count,
						GError       **error);
	
	gboolean (* close) (RsvgHandle  *handle,
						GError     **error);

	void (* free) (RsvgHandle * handle);
};

void rsvg_linear_gradient_free (RsvgDefVal *self);
void rsvg_radial_gradient_free (RsvgDefVal *self);

/* "super"/parent calls */
void rsvg_handle_init (RsvgHandle * handle);
gboolean rsvg_handle_write_impl (RsvgHandle    *handle,
								 const guchar  *buf,
								 gsize          count,
								 GError       **error);
gboolean rsvg_handle_close_impl (RsvgHandle  *handle, 
								 GError     **error);
void rsvg_handle_free_impl (RsvgHandle *handle);

typedef enum {
	RSVG_SIZE_ZOOM,
	RSVG_SIZE_WH,
	RSVG_SIZE_WH_MAX,
	RSVG_SIZE_ZOOM_MAX
} RsvgSizeType;

typedef enum {
	objectBoundingBox, userSpaceOnUse
} RsvgCoordUnits;

struct RsvgSizeCallbackData
{
	RsvgSizeType type;
	double x_zoom;
	double y_zoom;
	gint width;
	gint height;
};

/* private */
GdkPixbuf *
rsvg_pixbuf_from_file_with_size_data (const gchar * file_name,
									  struct RsvgSizeCallbackData * data,
									  GError ** error);
/* private */
GdkPixbuf *
rsvg_pixbuf_from_stdio_file_with_size_data(FILE * f,
										   struct RsvgSizeCallbackData * data,
										   GError ** error);

struct _RsvgPropertyBag
{
	GHashTable * props;
};

RsvgPropertyBag *
rsvg_property_bag_new (const xmlChar **atts);

void
rsvg_property_bag_free (RsvgPropertyBag *bag);

const char *
rsvg_property_bag_lookup (RsvgPropertyBag *bag, const char * key);

guint
rsvg_property_bag_size (RsvgPropertyBag *bag);

G_END_DECLS

#endif
