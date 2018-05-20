/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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
#include "rsvg-attributes.h"

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>
#include <pango/pango.h>
#include <glib.h>
#include <glib-object.h>
#include <math.h>

#if defined(HAVE_FLOAT_H)
# include <float.h>
#endif

#if defined(HAVE_PANGOFT2)
#include <pango/pangofc-fontmap.h>
#endif

G_BEGIN_DECLS 

typedef struct RsvgSaxHandler RsvgSaxHandler;
typedef struct _RsvgCairoRender RsvgCairoRender;
typedef struct RsvgDrawingCtx RsvgDrawingCtx;

/* Opaque; defined in rsvg_internals/src/state.rs */
typedef struct RsvgState RsvgState;

typedef void   *RsvgPropertyBag;
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

/* Reading state for an RsvgHandle */
typedef enum {
    RSVG_HANDLE_STATE_START,
    RSVG_HANDLE_STATE_LOADING,
    RSVG_HANDLE_STATE_CLOSED_OK,
    RSVG_HANDLE_STATE_CLOSED_ERROR
} RsvgHandleState;

typedef struct RsvgLoad RsvgLoad;

struct RsvgHandlePrivate {
    RsvgHandleFlags flags;

    RsvgHandleState hstate;

    RsvgLoad *load;

    gboolean is_disposed;

    RsvgSizeFunc size_func;
    gpointer user_data;
    GDestroyNotify user_data_destroy;

    GPtrArray *all_nodes;

    RsvgDefs *defs; /* lookup table for nodes that have an id="foo" attribute */
    /* this is the root level of the displayable tree, essentially what the
       file is converted into at the end */
    RsvgNode *treebase;

    GHashTable *css_props;

    GCancellable *cancellable;

    double dpi_x;
    double dpi_y;

    gchar *base_uri;
    GFile *base_gfile;

    gboolean in_loop;		/* see get_dimension() */

    gboolean is_testing; /* Are we being run from the test suite? */
};

/* Keep this in sync with rust/src/viewbox.rs::RsvgViewBox */
typedef struct {
    cairo_rectangle_t rect;
    gboolean active;
} RsvgViewBox;

/* Opaque; defined in rsvg_internals/src/bbox.rs */
typedef struct RsvgBbox RsvgBbox;

/* Keep this in sync with rust/src/length.rs:LengthUnit */
typedef enum {
    LENGTH_UNIT_DEFAULT,
    LENGTH_UNIT_PERCENT,
    LENGTH_UNIT_FONT_EM,
    LENGTH_UNIT_FONT_EX,
    LENGTH_UNIT_INCH,
    LENGTH_UNIT_RELATIVE_LARGER,
    LENGTH_UNIT_RELATIVE_SMALLER
} LengthUnit;

/* Keep this in sync with rust/src/length.rs:LengthDir */
typedef enum {
    LENGTH_DIR_HORIZONTAL,
    LENGTH_DIR_VERTICAL,
    LENGTH_DIR_BOTH
} LengthDir;

/* Keep this in sync with rust/src/length.rs:RsvgLength */
typedef struct {
    double length;
    LengthUnit unit;
    LengthDir dir;
} RsvgLength;

typedef enum {
    userSpaceOnUse,
    objectBoundingBox
} RsvgCoordUnits;

/* Keep this in sync with rust/src/node.rs:NodeType */
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
    RSVG_NODE_TYPE_LINK,
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
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_FIRST,              /* just a marker; not a valid type */
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_BLEND,
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_COLOR_MATRIX,
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
    RSVG_NODE_TYPE_FILTER_PRIMITIVE_LAST                /* just a marker; not a valid type */
} RsvgNodeType;

typedef void (* CNodeSetAtts) (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag pbag);
typedef void (* CNodeDraw) (RsvgNode *node,
                            gpointer impl,
                            RsvgDrawingCtx *ctx,
                            RsvgState *state,
                            int dominate,
                            gboolean clipping);
typedef void (* CNodeFree) (gpointer impl);

/* Implemented in rust/src/node.rs */
/* Call node = rsvg_node_unref (node) when you are done with the node */
G_GNUC_INTERNAL
RsvgNode *rsvg_rust_cnode_new (RsvgNodeType  node_type,
                               RsvgNode     *parent,
                               gpointer      impl,
                               CNodeSetAtts  set_atts_fn,
                               CNodeDraw     draw_fn,
                               CNodeFree     free_fn) G_GNUC_WARN_UNUSED_RESULT;

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
gpointer rsvg_rust_cnode_get_impl (RsvgNode *node);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
RsvgNodeType rsvg_node_get_type (RsvgNode *node);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
gboolean rsvg_node_is_same (RsvgNode *node1, RsvgNode *node2);

/* Implemented in rust/src/node.rs */
/* Call this as newref = rsvg_node_ref (node);  You don't own the node anymore, just the newref! */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_ref (RsvgNode *node) G_GNUC_WARN_UNUSED_RESULT;

/* Implemented in rust/src/node.rs */
/* Call this as node = rsvg_node_unref (node);  Then node will be NULL and you don't own it anymore! */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_unref (RsvgNode *node) G_GNUC_WARN_UNUSED_RESULT;

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
RsvgState *rsvg_node_get_state (RsvgNode *node);

/* Implemented in rust/src/node.rs
 *
 * Returns a new strong reference to the parent (or NULL); use rsvg_node_unref()
 * when you are done.
 */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_get_parent (RsvgNode *node) G_GNUC_WARN_UNUSED_RESULT;

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_add_child (RsvgNode *node, RsvgNode *child);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_set_atts (RsvgNode *node, RsvgHandle *handle, RsvgPropertyBag atts);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_draw (RsvgNode *node, RsvgDrawingCtx *draw, int dominate, gboolean clipping);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_set_attribute_parse_error (RsvgNode *node, const char *attr_name, const char *description);

typedef struct RsvgNodeChildrenIter *RsvgNodeChildrenIter;

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
RsvgNodeChildrenIter *rsvg_node_children_iter_begin (RsvgNode *node);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
gboolean rsvg_node_children_iter_next (RsvgNodeChildrenIter *iter,
                                       RsvgNode **out_child);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
gboolean rsvg_node_children_iter_next_back (RsvgNodeChildrenIter *iter,
                                            RsvgNode **out_child);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_children_iter_end (RsvgNodeChildrenIter *iter);

/* generic function for drawing all of the children of a particular node */

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_draw_children (RsvgNode *node, RsvgDrawingCtx *ctx, int dominate, gboolean clipping);

typedef void (*RsvgPropertyBagEnumFunc) (const char *key, const char *value, gpointer user_data);

/* Implemented in rust/src/property_bag.rs */
G_GNUC_INTERNAL
RsvgPropertyBag	    rsvg_property_bag_new       (const char **atts);

/* Implemented in rust/src/property_bag.rs */
G_GNUC_INTERNAL
void                 rsvg_property_bag_free      (RsvgPropertyBag bag);

typedef struct RsvgPropertyBagIter *RsvgPropertyBagIter;

/* Implemented in rust/src/property_bag.rs */
G_GNUC_INTERNAL
RsvgPropertyBagIter *rsvg_property_bag_iter_begin (RsvgPropertyBag bag);

/* Implemented in rust/src/property_bag.rs */
G_GNUC_INTERNAL
gboolean rsvg_property_bag_iter_next (RsvgPropertyBagIter *iter,
                                      const char **out_key,
                                      RsvgAttribute *out_attr,
                                      const char **out_value);

/* Implemented in rust/src/property_bag.rs */
G_GNUC_INTERNAL
void rsvg_property_bag_iter_end (RsvgPropertyBagIter *iter);

/* for some reason this one's public... */
GdkPixbuf *rsvg_pixbuf_from_data_with_size_data (const guchar * buff,
                                                 size_t len,
                                                 gpointer data,
                                                 const char *base_uri, GError ** error);

/* Implemented in rust/src/cond.rs */
G_GNUC_INTERNAL
gboolean rsvg_cond_check_required_features (const char *value);

/* Implemented in rust/src/cond.rs */
G_GNUC_INTERNAL
gboolean rsvg_cond_check_required_extensions (const char *value);

/* Implemented in rust/src/cond.rs */
G_GNUC_INTERNAL
gboolean rsvg_cond_check_system_language (const char *value);

G_GNUC_INTERNAL
cairo_surface_t *rsvg_cairo_surface_from_pixbuf (const GdkPixbuf *pixbuf);
G_GNUC_INTERNAL
GdkPixbuf *rsvg_cairo_surface_to_pixbuf (cairo_surface_t *surface);

G_GNUC_INTERNAL
cairo_surface_t *rsvg_cairo_surface_new_from_href (RsvgHandle *handle, const char *href, GError ** error);

/* Implemented in rust/src/bbox.rs */
G_GNUC_INTERNAL
RsvgBbox *rsvg_bbox_new (cairo_matrix_t *matrix, cairo_rectangle_t *rect, cairo_rectangle_t *ink_rect);

/* Implemented in rust/src/bbox.rs */
void rsvg_bbox_free (RsvgBbox *bbox);

/* Implemented in rust/src/bbox.rs */
RsvgBbox *rsvg_bbox_clone (RsvgBbox *bbox);

/* Implemented in rust/src/bbox.rs */
G_GNUC_INTERNAL
void rsvg_bbox_insert (RsvgBbox *dst, RsvgBbox *src);

/* Implemented in rust/src/bbox.rs */
G_GNUC_INTERNAL
void rsvg_bbox_clip (RsvgBbox *dst, RsvgBbox *src);

/* Implemented in rust/src/bbox.rs */
G_GNUC_INTERNAL
void rsvg_bbox_get_rect (RsvgBbox *bbox, cairo_rectangle_t *rect, cairo_rectangle_t *ink_rect);

/* This is implemented in rust/src/length.rs */
G_GNUC_INTERNAL
double rsvg_length_normalize (const RsvgLength *length, RsvgDrawingCtx * ctx);

/* This is implemented in rust/src/length.rs */
G_GNUC_INTERNAL
double rsvg_length_hand_normalize (const RsvgLength *length,
                                   double pixels_per_inch,
                                   double width_or_height,
                                   double font_size);

/* Implemented in rust/src/length.rs */
G_GNUC_INTERNAL
RsvgLength rsvg_length_parse (const char *str, LengthDir dir);

G_GNUC_INTERNAL
void rsvg_return_if_fail_warning (const char *pretty_function,
                                  const char *expression, GError ** error);

G_GNUC_INTERNAL
RsvgNode *rsvg_load_destroy (RsvgLoad *load) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
void rsvg_add_node_to_handle (RsvgHandle *handle, RsvgNode *node);

G_GNUC_INTERNAL
char *rsvg_handle_resolve_uri (RsvgHandle *handle,
                               const char *uri);

G_GNUC_INTERNAL
gboolean rsvg_allow_load (GFile       *base_gfile,
                          const char  *uri,
                          GError     **error);

G_GNUC_INTERNAL
char *_rsvg_handle_acquire_data (RsvgHandle *handle,
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
