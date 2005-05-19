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
#include "rsvg-bpath-util.h"

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>
#include <pango/pango.h>
#include <glib/gslist.h>
#include <math.h>

G_BEGIN_DECLS

typedef struct RsvgSaxHandler RsvgSaxHandler;
typedef struct RsvgDrawingCtx RsvgDrawingCtx;
typedef struct RsvgRender RsvgRender;
typedef struct RsvgDimensionData RsvgDimensionData;
typedef struct _RsvgPropertyBag RsvgPropertyBag;
typedef struct _RsvgState RsvgState;
typedef struct _RsvgDefs RsvgDefs;
typedef struct _RsvgNode RsvgNode;
typedef struct _RsvgFilter RsvgFilter;

/* prepare for gettext */
#ifndef _
#define _(X) X
#endif

#ifndef N_
#define N_(X) X
#endif

#ifndef M_PI
#  ifdef G_PI
#    define M_PI G_PI
#  else
#    define M_PI 3.14159265358979323846
#  endif /* G_PI */
#endif /*  M_PI  */


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

	/* stack; there is a state for each element */
	
	RsvgDefs *defs;
	guint nest_level;
	RsvgNode *currentnode;
	/* this is the root level of the displayable tree, essentially what the
	   file is converted into at the end */
	void *treebase;

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
	double dpi_x;
	double dpi_y;

	GSList * dimensions;
	
	GString * title;
	GString * desc;
	GString * metadata;
	
	gchar * base_uri;

	gboolean finished;

	gboolean first_write;
	gboolean is_gzipped;
	void * gzipped_data; /* really a GsfOutput */
};

/*Contextual information for the drawing phase*/

struct RsvgDrawingCtx {
	RsvgRender *render;
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
	void (* free) (RsvgRender * self);

	void (* render_path) (RsvgDrawingCtx *ctx, const RsvgBpathDef * path);
	void (* render_image) (RsvgDrawingCtx *ctx, const GdkPixbuf * pixbuf,
						   double x, double y, double w, double h);
	void (* pop_discrete_layer) (RsvgDrawingCtx *ctx);
	void (* push_discrete_layer) (RsvgDrawingCtx *ctx);
	void (* add_clipping_rect) (RsvgDrawingCtx *ctx,
								double x, double y, double w, double h);
};

struct RsvgDimensionData {
	int width;
	int height;
	gdouble em, ex;
};

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

typedef enum {
	RSVG_NODE_LINGRAD,
	RSVG_NODE_RADGRAD,
	RSVG_NODE_PATTERN,
	RSVG_NODE_PATH,
	RSVG_NODE_FILTER,
	RSVG_NODE_FILTER_PRIMITIVE,
	RSVG_NODE_FILTER_PRIMITIVE_MERGE_NODE,
	RSVG_NODE_MASK,
	RSVG_NODE_MARKER,
	RSVG_NODE_SYMBOL,
	RSVG_NODE_CLIP_PATH
} RsvgNodeType;

struct _RsvgNode {
	RsvgNodeType type;
	RsvgState * state;
	RsvgNode * parent;
 	GPtrArray *children;
	void (*free) (RsvgNode *self);
	void (*draw) (RsvgNode * self, RsvgDrawingCtx *ctx, int dominate);
	void (*set_atts) (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag*);
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
void rsvg_render_image (RsvgDrawingCtx *ctx, GdkPixbuf * pb, 
						double x, double y, double w, double h);
void rsvg_render_free (RsvgRender * render);
void rsvg_add_clipping_rect (RsvgDrawingCtx *ctx, double x, double y, 
							 double w, double h);


void
_rsvg_affine_invert (double dst_affine[6], const double src_affine[6]);

/* flip the matrix, FALSE, FALSE is a simple copy operation, and
   TRUE, TRUE equals a rotation by 180 degrees */
void
_rsvg_affine_flip (double dst_affine[6], const double src_affine[6],
                 int horz, int vert);

void
_rsvg_affine_multiply (double dst[6],
		     const double src1[6], const double src2[6]);

/* set up the identity matrix */
void
_rsvg_affine_identity (double dst[6]);

/* set up a scaling matrix */
void
_rsvg_affine_scale (double dst[6], double sx, double sy);

/* set up a rotation matrix; theta is given in degrees */
void
_rsvg_affine_rotate (double dst[6], double theta);

/* set up a shearing matrix; theta is given in degrees */
void
_rsvg_affine_shear (double dst[6], double theta);

/* set up a translation matrix */
void
_rsvg_affine_translate (double dst[6], double tx, double ty);


/* find the affine's "expansion factor", i.e. the scale amount */
double
_rsvg_affine_expansion (const double src[6]);

/* Determine whether the affine transformation is rectilinear,
   i.e. whether a rectangle aligned to the grid is transformed into
   another rectangle aligned to the grid. */
int
_rsvg_affine_rectilinear (const double src[6]);

/* Determine whether two affine transformations are equal within grid allignment */
int
_rsvg_affine_equal (double matrix1[6], double matrix2[6]);

void
rsvg_node_set_atts(RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts);

G_END_DECLS

#endif
