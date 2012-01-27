/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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

#include <cairo.h>

#include "rsvg.h"

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>
#include <pango/pango.h>
#include <glib.h>
#include <glib-object.h>
#include <math.h>

#if defined(HAVE_FLOAT_H)
# include <float.h>
#endif

G_BEGIN_DECLS 

typedef struct RsvgSaxHandler RsvgSaxHandler;
typedef struct RsvgDrawingCtx RsvgDrawingCtx;
typedef struct RsvgRender RsvgRender;
typedef GHashTable RsvgPropertyBag;
typedef struct _RsvgState RsvgState;
typedef struct _RsvgDefs RsvgDefs;
typedef struct _RsvgNode RsvgNode;
typedef struct _RsvgFilter RsvgFilter;
typedef struct _RsvgNodeChars RsvgNodeChars;

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
#  endif                        /* G_PI */
#endif                          /*  M_PI  */

#ifndef DBL_EPSILON
/* 1e-7 is a conservative value.  it's less than 2^(1-24) which is
 * the epsilon value for a 32-bit float.  The regular value for this
 * with 64-bit doubles is 2^(1-53) or approximately 1e-16.
 */
# define DBL_EPSILON 1e-7
#endif

/* RSVG_ONE_MINUS_EPSILON:
 *
 * DBL_EPSILON is the difference between 1 and the least value greater
 * than 1 that is representable in the given floating-point type.  Then
 * 1.0+DBL_EPSILON looks like:
 *
 *         1.00000000000...0000000001 * 2**0
 *
 * while 1.0-DBL_EPSILON looks like:
 *
 *         0.11111111111...1111111111 * 2**0
 *
 * and so represented as:
 *
 *         1.1111111111...11111111110 * 2**-1
 *
 * so, in fact, 1.0-(DBL_EPSILON*.5) works too, but I don't think it
 * really matters.  So, I'll go with the simple 1.0-DBL_EPSILON here.
 *
 * The following python session shows these observations:
 *
 *         >>> 1.0 + 2**(1-53)
 *         1.0000000000000002
 *         >>> 1.0 + 2**(1-54)
 *         1.0
 *         >>> 1.0 - 2**(1-53)
 *         0.99999999999999978
 *         >>> 1.0 - 2**(1-54)
 *         0.99999999999999989
 *         >>> 1.0 - 2**(1-53)*.5
 *         0.99999999999999989
 *         >>> 1.0 - 2**(1-55)
 *         1.0
 */
#define RSVG_ONE_MINUS_EPSILON (1.0 - DBL_EPSILON)

struct RsvgSaxHandler {
    void (*free) (RsvgSaxHandler * self);
    void (*start_element) (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts);
    void (*end_element) (RsvgSaxHandler * self, const char *name);
    void (*characters) (RsvgSaxHandler * self, const char *ch, int len);
};

typedef enum {
    RSVG_LOAD_POLICY_ALL_PERMISSIVE
} RsvgLoadPolicy;

#define RSVG_LOAD_POLICY_DEFAULT (RSVG_LOAD_POLICY_ALL_PERMISSIVE)

struct RsvgHandlePrivate {
    RsvgHandleFlags flags;

    RsvgLoadPolicy load_policy;

    gboolean is_disposed;
    gboolean is_closed;

    RsvgSizeFunc size_func;
    gpointer user_data;
    GDestroyNotify user_data_destroy;

    /* stack; there is a state for each element */

    RsvgDefs *defs;
    guint nest_level;
    RsvgNode *currentnode;
    /* this is the root level of the displayable tree, essentially what the
       file is converted into at the end */
    RsvgNode *treebase;

    GHashTable *css_props;

    /* not a handler stack. each nested handler keeps
     * track of its parent
     */
    RsvgSaxHandler *handler;
    int handler_nest;

    GHashTable *entities;       /* g_malloc'd string -> xmlEntityPtr */

    xmlParserCtxtPtr ctxt;
    GError **error;
    GCancellable *cancellable;

    double dpi_x;
    double dpi_y;

    GString *title;
    GString *desc;
    GString *metadata;

    gchar *base_uri;
    GFile *base_gfile;

    gboolean finished;

    gboolean in_loop;		/* see get_dimension() */

    gboolean first_write;
    GInputStream *data_input_stream; /* for rsvg_handle_write of svgz data */
};

typedef struct {
    cairo_rectangle_t rect;
    gboolean active;
} RsvgViewBox;

/*Contextual information for the drawing phase*/

struct RsvgDrawingCtx {
    RsvgRender *render;
    RsvgState *state;
    GError **error;
    RsvgDefs *defs;
    gchar *base_uri;
    PangoContext *pango_context;
    double dpi_x, dpi_y;
    RsvgViewBox vb;
    GSList *vb_stack;
    GSList *drawsub_stack;
    GSList *ptrs;
};

/*Abstract base class for context for our backends (one as yet)*/

typedef enum {
  RSVG_RENDER_TYPE_INVALID,

  RSVG_RENDER_TYPE_BASE,

  RSVG_RENDER_TYPE_CAIRO = 8,
  RSVG_RENDER_TYPE_CAIRO_CLIP
} RsvgRenderType;

struct RsvgRender {
    RsvgRenderType type;

    void (*free) (RsvgRender * self);

    PangoContext    *(*create_pango_context)    (RsvgDrawingCtx * ctx);
    void             (*render_pango_layout)	    (RsvgDrawingCtx * ctx, PangoLayout *layout,
                                                 double x, double y);
    void             (*render_path)             (RsvgDrawingCtx * ctx, const cairo_path_t *path);
    void             (*render_surface)          (RsvgDrawingCtx * ctx, cairo_surface_t *surface,
                                                 double x, double y, double w, double h);
    void             (*pop_discrete_layer)      (RsvgDrawingCtx * ctx);
    void             (*push_discrete_layer)     (RsvgDrawingCtx * ctx);
    void             (*add_clipping_rect)       (RsvgDrawingCtx * ctx, double x, double y,
                                                 double w, double h);
    cairo_surface_t *(*get_surface_of_node)     (RsvgDrawingCtx * ctx, RsvgNode * drawable,
                                                 double w, double h);
};

static inline RsvgRender *
_rsvg_render_check_type (RsvgRender *render,
                         RsvgRenderType type)
{
  g_assert ((render->type & type) == type);
  return render;
}

#define _RSVG_RENDER_CIC(render, render_type, RenderCType) \
  ((RenderCType*) _rsvg_render_check_type ((render), (render_type)))

typedef struct {
    double length;
    char factor;
} RsvgLength;

typedef struct {
    cairo_rectangle_t rect;
    cairo_matrix_t affine;
    gboolean virgin;
} RsvgBbox;

typedef enum {
    objectBoundingBox, userSpaceOnUse
} RsvgCoordUnits;

typedef enum {
    RSVG_NODE_TYPE_INVALID = 0,

    RSVG_NODE_TYPE_CHARS,
    RSVG_NODE_TYPE_CIRCLE,
    RSVG_NODE_TYPE_CLIP_PATH,
    RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION,
    RSVG_NODE_TYPE_DEFS,
    RSVG_NODE_TYPE_ELLIPSE,
    RSVG_NODE_TYPE_FILTER,
    RSVG_NODE_TYPE_GROUP,
    RSVG_NODE_TYPE_IMAGE,
    RSVG_NODE_TYPE_LIGHT_SOURCE,
    RSVG_NODE_TYPE_LINE,
    RSVG_NODE_TYPE_LINEAR_GRADIENT,
    RSVG_NODE_TYPE_MARKER,
    RSVG_NODE_TYPE_MASK,
    RSVG_NODE_TYPE_PATH,
    RSVG_NODE_TYPE_PATTERN,
    RSVG_NODE_TYPE_POLYGON,
    RSVG_NODE_TYPE_POLYLINE,
    RSVG_NODE_TYPE_RADIAL_GRADIENT,
    RSVG_NODE_TYPE_RECT,
    RSVG_NODE_TYPE_STOP,
    RSVG_NODE_TYPE_SVG,
    RSVG_NODE_TYPE_SWITCH,
    RSVG_NODE_TYPE_SYMBOL,
    RSVG_NODE_TYPE_TEXT,
    RSVG_NODE_TYPE_TREF,
    RSVG_NODE_TYPE_TSPAN,
    RSVG_NODE_TYPE_USE,

    /* Filter primitives */
    RSVG_NODE_TYPE_FILTER_PRIMITIVE = 64,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_BLEND,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_COLOUR_MATRIX,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPONENT_TRANSFER,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPOSITE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_CONVOLVE_MATRIX,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_DIFFUSE_LIGHTING,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_DISPLACEMENT_MAP,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_ERODE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_FLOOD,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_GAUSSIAN_BLUR,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_IMAGE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_MERGE_NODE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_OFFSET,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_SPECULAR_LIGHTING,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_TILE,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_TURBULENCE,

} RsvgNodeType;

struct _RsvgNode {
    RsvgState *state;
    RsvgNode *parent;
    GPtrArray *children;
    RsvgNodeType type;
    const char *name; /* owned by the xmlContext, invalid after parsing! */
    void (*free) (RsvgNode * self);
    void (*draw) (RsvgNode * self, RsvgDrawingCtx * ctx, int dominate);
    void (*set_atts) (RsvgNode * self, RsvgHandle * ctx, RsvgPropertyBag *);
};

#define RSVG_NODE_TYPE(node)                ((node)->type)
#define RSVG_NODE_IS_FILTER_PRIMITIVE(node) (RSVG_NODE_TYPE((node)) & RSVG_NODE_TYPE_FILTER_PRIMITIVE)

struct _RsvgNodeChars {
    RsvgNode super;
    GString *contents;
};

typedef void (*RsvgPropertyBagEnumFunc) (const char *key, const char *value, gpointer user_data);

G_GNUC_INTERNAL
RsvgPropertyBag	    *rsvg_property_bag_new       (const char **atts);
G_GNUC_INTERNAL
RsvgPropertyBag	    *rsvg_property_bag_dup       (RsvgPropertyBag * bag);
G_GNUC_INTERNAL
void                 rsvg_property_bag_free      (RsvgPropertyBag * bag);
G_GNUC_INTERNAL
const char          *rsvg_property_bag_lookup    (RsvgPropertyBag * bag, const char *key);
G_GNUC_INTERNAL
guint                rsvg_property_bag_size	     (RsvgPropertyBag * bag);
G_GNUC_INTERNAL
void                 rsvg_property_bag_enumerate (RsvgPropertyBag * bag, RsvgPropertyBagEnumFunc func,
                                                  gpointer user_data);
/* for some reason this one's public... */
GdkPixbuf *rsvg_pixbuf_from_data_with_size_data (const guchar * buff,
                                                 size_t len,
                                                 gpointer data,
                                                 const char *base_uri, GError ** error);
G_GNUC_INTERNAL
gboolean     rsvg_eval_switch_attributes	(RsvgPropertyBag * atts, gboolean * p_has_cond);
G_GNUC_INTERNAL
gchar       *rsvg_get_base_uri_from_filename    (const gchar * file_name);
G_GNUC_INTERNAL
void rsvg_pop_discrete_layer    (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_push_discrete_layer   (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_render_path           (RsvgDrawingCtx * ctx, const cairo_path_t *path);
G_GNUC_INTERNAL
void rsvg_render_surface        (RsvgDrawingCtx * ctx, cairo_surface_t *surface,
                                 double x, double y, double w, double h);
G_GNUC_INTERNAL
void rsvg_render_free           (RsvgRender * render);
G_GNUC_INTERNAL
void rsvg_add_clipping_rect     (RsvgDrawingCtx * ctx, double x, double y, double w, double h);
G_GNUC_INTERNAL
cairo_surface_t *rsvg_cairo_surface_from_pixbuf (const GdkPixbuf *pixbuf);
G_GNUC_INTERNAL
GdkPixbuf *rsvg_cairo_surface_to_pixbuf (cairo_surface_t *surface);
G_GNUC_INTERNAL
cairo_surface_t *rsvg_get_surface_of_node (RsvgDrawingCtx * ctx, RsvgNode * drawable, double w, double h);
G_GNUC_INTERNAL
void rsvg_node_set_atts (RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts);
G_GNUC_INTERNAL
void rsvg_drawing_ctx_free (RsvgDrawingCtx * handle);
G_GNUC_INTERNAL
void rsvg_bbox_init     (RsvgBbox * self, cairo_matrix_t *matrix);
G_GNUC_INTERNAL
void rsvg_bbox_insert   (RsvgBbox * dst, RsvgBbox * src);
G_GNUC_INTERNAL
void rsvg_bbox_clip     (RsvgBbox * dst, RsvgBbox * src);
G_GNUC_INTERNAL
double _rsvg_css_normalize_length       (const RsvgLength * in, RsvgDrawingCtx * ctx, char dir);
G_GNUC_INTERNAL
double _rsvg_css_hand_normalize_length  (const RsvgLength * in, gdouble pixels_per_inch,
                                         gdouble width_or_height, gdouble font_size);
double _rsvg_css_normalize_font_size    (RsvgState * state, RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
RsvgLength _rsvg_css_parse_length (const char *str);
G_GNUC_INTERNAL
void _rsvg_push_view_box    (RsvgDrawingCtx * ctx, double w, double h);
G_GNUC_INTERNAL
void _rsvg_pop_view_box	    (RsvgDrawingCtx * ctx);
G_GNUC_INTERNAL
void rsvg_SAX_handler_struct_init (void);
G_GNUC_INTERNAL
char *rsvg_get_url_string (const char *str);
G_GNUC_INTERNAL
void rsvg_return_if_fail_warning (const char *pretty_function,
                                  const char *expression, GError ** error);

G_GNUC_INTERNAL
guint8* _rsvg_handle_acquire_data (RsvgHandle *handle,
                                   const char *uri,
                                   char **content_type,
                                   gsize *len,
                                   GError **error);
G_GNUC_INTERNAL
GInputStream *_rsvg_handle_acquire_stream (RsvgHandle *handle,
                                           const char *uri,
                                           char **content_type,
                                           GError **error);


#define rsvg_return_if_fail(expr, error)    G_STMT_START{			\
     if G_LIKELY(expr) { } else                                     \
       {                                                            \
           rsvg_return_if_fail_warning (G_STRFUNC,                  \
                                        #expr, error);              \
           return;                                                  \
       };				}G_STMT_END

#define rsvg_return_val_if_fail(expr,val,error)	G_STMT_START{       \
     if G_LIKELY(expr) { } else                                     \
       {                                                            \
           rsvg_return_if_fail_warning (G_STRFUNC,                  \
                                        #expr, error);              \
           return (val);                                            \
       };				}G_STMT_END

G_END_DECLS

#endif
