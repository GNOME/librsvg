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

#include <pango/pangocairo.h>
#ifdef HAVE_PANGOFT2
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

/* Reading state for an RsvgHandle */
typedef enum {
    RSVG_HANDLE_STATE_START,
    RSVG_HANDLE_STATE_LOADING,
    RSVG_HANDLE_STATE_CLOSED_OK,
    RSVG_HANDLE_STATE_CLOSED_ERROR
} RsvgHandleState;

typedef struct RsvgLoad RsvgLoad;

typedef struct RsvgTree RsvgTree;

typedef struct RsvgCssStyles RsvgCssStyles;

struct RsvgHandlePrivate {
    RsvgHandleFlags flags;

    RsvgHandleState hstate;

    RsvgLoad *load;

    RsvgSizeFunc size_func;
    gpointer user_data;
    GDestroyNotify user_data_destroy;

    RsvgTree *tree;

    RsvgDefs *defs; /* lookup table for nodes that have an id="foo" attribute */

    RsvgCssStyles *css_styles;

    GCancellable *cancellable;

    double dpi_x;
    double dpi_y;

    gchar *base_uri;
    GFile *base_gfile;

    gboolean in_loop;		/* see get_dimension() */

    gboolean is_testing; /* Are we being run from the test suite? */

#ifdef HAVE_PANGOFT2
    FcConfig *font_config_for_testing;
    PangoFontMap *font_map_for_testing;
#endif
};

/* Implemented in rust/src/node.rs */
/* Call this as newref = rsvg_node_ref (node);  You don't own the node anymore, just the newref! */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_ref (RsvgNode *node) G_GNUC_WARN_UNUSED_RESULT;

/* Implemented in rust/src/node.rs */
/* Call this as node = rsvg_node_unref (node);  Then node will be NULL and you don't own it anymore! */
G_GNUC_INTERNAL
RsvgNode *rsvg_node_unref (RsvgNode *node) G_GNUC_WARN_UNUSED_RESULT;

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
void rsvg_node_set_overridden_properties (RsvgNode *node);

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

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
RsvgTree *rsvg_tree_new (RsvgNode *root);

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
void rsvg_tree_free (RsvgTree *tree);

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
RsvgNode *rsvg_tree_get_root (RsvgTree *tree);

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
gboolean rsvg_tree_is_root (RsvgTree *tree, RsvgNode *node);

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
gboolean rsvg_tree_root_is_svg (RsvgTree *tree);

/* Implemented in rsvg_internals/src/tree.rs */
G_GNUC_INTERNAL
void rsvg_tree_cascade (RsvgTree *tree);

/* Implemented in rsvg_internals/src/css.rs */
G_GNUC_INTERNAL
RsvgCssStyles *rsvg_css_styles_new (void);

/* Implemented in rsvg_internals/src/css.rs */
G_GNUC_INTERNAL
void rsvg_css_styles_free (RsvgCssStyles *styles);

/* Implemented in rsvg_internals/src/css.rs */
G_GNUC_INTERNAL
void rsvg_css_parse_into_handle (RsvgHandle *handle,
                                 const char *buf,
                                 gsize len);

/* Implemented in rsvg_internals/src/structure.rs */
G_GNUC_INTERNAL
gboolean rsvg_node_svg_get_size (RsvgNode *node, double dpi_x, double dpi_y, int *out_width, int *out_height);

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
GdkPixbuf *rsvg_cairo_surface_to_pixbuf (cairo_surface_t *surface);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
RsvgDrawingCtx *rsvg_drawing_ctx_new (cairo_t *cr,
                                      guint width,
                                      guint height,
                                      double vb_width,
                                      double vb_height,
                                      double dpi_x,
                                      double dpi_y,
                                      RsvgDefs *defs,
                                      gboolean testing);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
void rsvg_drawing_ctx_free (RsvgDrawingCtx *draw_ctx);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
void rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx,
                                                       RsvgNode        *node);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_draw_node_from_stack (RsvgDrawingCtx *ctx, RsvgTree *tree) G_GNUC_WARN_UNUSED_RESULT;;

/* Defined in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
gboolean rsvg_drawing_ctx_get_ink_rect (RsvgDrawingCtx *ctx, cairo_rectangle_t *ink_rect);

/* Implemented in rust/src/node.rs */
G_GNUC_INTERNAL
void rsvg_root_node_cascade(RsvgNode *node);

G_GNUC_INTERNAL
void rsvg_return_if_fail_warning (const char *pretty_function,
                                  const char *expression, GError ** error);

G_GNUC_INTERNAL
RsvgNode *rsvg_load_destroy (RsvgLoad *load) G_GNUC_WARN_UNUSED_RESULT;

/* Defined in rsvg_internals/src/defs.rs */
G_GNUC_INTERNAL
RsvgDefs *rsvg_defs_new (RsvgHandle *handle);

/* Defined in rsvg_internals/src/defs.rs */
G_GNUC_INTERNAL
void rsvg_defs_free (RsvgDefs *defs);

/* Defined in rsvg_internals/src/defs.rs */
/* for some reason this one's public... */
RsvgNode *rsvg_defs_lookup (const RsvgDefs * defs, const char *name);

G_GNUC_INTERNAL
RsvgDefs *rsvg_handle_get_defs (RsvgHandle *handle);

G_GNUC_INTERNAL
RsvgCssStyles *rsvg_handle_get_css_styles (RsvgHandle *handle);

G_GNUC_INTERNAL
char *rsvg_handle_resolve_uri (RsvgHandle *handle,
                               const char *uri);

G_GNUC_INTERNAL
RsvgHandle *rsvg_handle_load_extern (RsvgHandle *handle,
                                     const char *uri);

G_GNUC_INTERNAL
gboolean rsvg_handle_keep_image_data (RsvgHandle *handle);

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
