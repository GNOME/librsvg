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
typedef struct _RsvgNode RsvgNode;
typedef struct _RsvgFilter RsvgFilter;

/* prepare for gettext */
#ifndef _
#define _(X) X
#endif

#ifndef N_
#define N_(X) X
#endif

G_GNUC_INTERNAL
double rsvg_get_default_dpi_x (void);

G_GNUC_INTERNAL
double rsvg_get_default_dpi_y (void);

/* Reading state for an RsvgHandle */
typedef enum {
    RSVG_HANDLE_STATE_START,
    RSVG_HANDLE_STATE_LOADING,
    RSVG_HANDLE_STATE_CLOSED_OK,
    RSVG_HANDLE_STATE_CLOSED_ERROR
} RsvgHandleState;

typedef struct RsvgLoad RsvgLoad;

/* Defined in rsvg_internals/src/handle.rs */
typedef struct RsvgHandleRust RsvgHandleRust;

struct RsvgHandlePrivate {
    RsvgLoad *load;

    RsvgSizeFunc size_func;
    gpointer user_data;
    GDestroyNotify user_data_destroy;

    gchar *base_uri; // Keep this here; since rsvg_handle_get_base_uri() returns a const char *

    gboolean in_loop;		/* see get_dimension() */

    gboolean is_testing; /* Are we being run from the test suite? */

#ifdef HAVE_PANGOFT2
    FcConfig *font_config_for_testing;
    PangoFontMap *font_map_for_testing;
#endif

    RsvgHandleRust *rust_handle;
};

/* Implemented in rsvg_internals/src/xml.rs */
typedef struct RsvgXmlState RsvgXmlState;

/* Implemented in rsvg_internals/src/xml.rs */
G_GNUC_INTERNAL
RsvgXmlState *rsvg_xml_state_new (RsvgHandle *handle);

G_GNUC_INTERNAL
void rsvg_xml_state_error(RsvgXmlState *xml, const char *msg);

/* Implemented in rsvg_internals/src/xml2_load.rs */
G_GNUC_INTERNAL
gboolean rsvg_xml_state_load_from_possibly_compressed_stream (RsvgXmlState *xml,
                                                              guint         flags,
                                                              GInputStream *stream,
                                                              GCancellable *cancellable,
                                                              GError      **error);

G_GNUC_INTERNAL
GdkPixbuf *rsvg_cairo_surface_to_pixbuf (cairo_surface_t *surface);

G_GNUC_INTERNAL
void rsvg_return_if_fail_warning (const char *pretty_function,
                                  const char *expression, GError ** error);

G_GNUC_INTERNAL
RsvgHandleRust *rsvg_handle_get_rust (RsvgHandle *handle);

/* Implemented in rsvg_internals/src/handle.rs */
G_GNUC_INTERNAL
char *rsvg_handle_acquire_data (RsvgHandle *handle,
                                const char *href,
                                gsize *len,
                                GError **error);

/* Implemented in rsvg_internals/src/handle.rs */
G_GNUC_INTERNAL
GInputStream *rsvg_handle_acquire_stream (RsvgHandle *handle,
                                          const char *href,
                                          GError **error);

#define rsvg_return_if_fail(expr, error)    G_STMT_START{           \
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
