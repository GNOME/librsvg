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
#include "rsvg-styles.h"

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>

G_BEGIN_DECLS

typedef struct RsvgSaxHandler RsvgSaxHandler;

struct RsvgSaxHandler {
  void (*free) (RsvgSaxHandler *self);
  void (*start_element) (RsvgSaxHandler *self, const xmlChar *name, const xmlChar **atts);
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
};

G_END_DECLS

#endif
