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
#include <pango/pango.h>
#include <libart_lgpl/art_rect.h>
#include <libart_lgpl/art_svp.h>
#include <glib/gslist.h>

G_BEGIN_DECLS

typedef struct RsvgSaxHandler RsvgSaxHandler;
typedef struct RsvgDrawingCtx RsvgDrawingCtx;
typedef struct RsvgRender RsvgRender;
typedef struct RsvgDimensionData RsvgDimensionData;
typedef struct _RsvgPropertyBag RsvgPropertyBag;
typedef struct _RsvgState RsvgState;
typedef struct _RsvgDefs RsvgDefs;
typedef struct _RsvgDefVal RsvgDefVal;
typedef struct _RsvgFilter RsvgFilter;

/* prepare for gettext */
#ifndef _
#define _(X) X
#endif

#ifndef N_
#define N_(X) X
#endif

struct RsvgSaxHandler {
	void (*free) (RsvgSaxHandler *self);
	void (*start_element) (RsvgSaxHandler *self, const xmlChar *name, RsvgPropertyBag *atts);
	void (*end_element) (RsvgSaxHandler *self, const xmlChar *name);
	void (*characters) (RsvgSaxHandler *self, const xmlChar *ch, int len);
};

/* Contextual information for the parsing phase*/

struct RsvgHandle {
	RsvgSizeFunc size_func;
	gpointer user_data;
	GDestroyNotify user_data_destroy;
	ArtIRect bbox;

	/* stack; there is a state for each element */
	
	RsvgDefs *defs;
	guint nest_level;
	void *current_defs_group;
	/* this is the root level of the displayable tree, essentially what the
	   file is converted into at the end */
	void *treebase;

	guint in_switch;
	GHashTable *css_props;
	
	/* not a handler stack. each nested handler keeps
	 * track of its parent
	 */
	RsvgSaxHandler *handler;
	int handler_nest;
	
	GHashTable *entities; /* g_malloc'd string -> xmlEntityPtr */
	
	xmlParserCtxtPtr ctxt;
	GError **error;
	
	int width;
	int height;
	int new_width;
	int new_height;
	double dpi_x;
	double dpi_y;

	GSList * dimensions;
	
	GString * title;
	GString * desc;
	GString * metadata;
	
	gchar * base_uri;

	gboolean finished;

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

/*Contextual information for the drawing phase*/

struct RsvgDrawingCtx {
	RsvgRender *render;
	ArtIRect bbox;
	GSList * state;
	GError **error;
	RsvgDefs *defs;
	gchar * base_uri;
	GMemChunk * state_allocator;
	PangoContext *pango_context;
	double dpi_x;
	double dpi_y;
};

/*Abstract base class for context for our backends (one as yet)*/

struct RsvgRender {
	void (* render_path) (RsvgDrawingCtx *ctx, const char *d);
	void (* pop_discrete_layer) (RsvgDrawingCtx *ctx);
	void (* push_discrete_layer) (RsvgDrawingCtx *ctx);
};

struct RsvgDimensionData {
	int width;
	int height;
	gdouble em, ex;
};

void rsvg_linear_gradient_free (RsvgDefVal *self);
void rsvg_radial_gradient_free (RsvgDefVal *self);
void rsvg_pattern_free (RsvgDefVal *self);

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

	gboolean keep_aspect_ratio;
};

struct _RsvgPropertyBag
{
	GHashTable * props;
};

typedef void (*RsvgPropertyBagEnumFunc) (const char * key,
										 const char * value,
										 gpointer user_data);

RsvgPropertyBag *
rsvg_property_bag_new (const xmlChar **atts);

void
rsvg_property_bag_free (RsvgPropertyBag *bag);

G_CONST_RETURN char *
rsvg_property_bag_lookup (RsvgPropertyBag *bag, const char * key);

guint
rsvg_property_bag_size (RsvgPropertyBag *bag);

void 
rsvg_property_bag_enumerate (RsvgPropertyBag * bag, RsvgPropertyBagEnumFunc func, gpointer user_data);

GdkPixbuf *
rsvg_pixbuf_from_data_with_size_data (const guchar * buff,
									  size_t len,
									  struct RsvgSizeCallbackData * data,
									  const char * base_uri,
									  GError ** error);

gboolean 
rsvg_eval_switch_attributes (RsvgPropertyBag *atts, gboolean * p_has_cond);

GdkPixbuf *
_rsvg_pixbuf_new_cleared (GdkColorspace colorspace, gboolean has_alpha, int bits_per_sample,
						  int width, int height);

gchar *
rsvg_get_base_uri_from_filename(const gchar * file_name);

GByteArray *
_rsvg_acquire_xlink_href_resource (const char *href,
								   const char *base_uri,
								   GError **err);

void rsvg_pop_discrete_layer(RsvgDrawingCtx *ctx);
void rsvg_push_discrete_layer (RsvgDrawingCtx *ctx);
void rsvg_render_path (RsvgDrawingCtx *ctx, const char *d);

G_END_DECLS

#endif
