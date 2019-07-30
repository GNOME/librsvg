/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/* 
   rsvg-cairo.h: SAX-based renderer for SVG files using cairo
 
   Copyright (C) 2005 Red Hat, Inc.
  
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
  
   Author: Carl Worth <cworth@cworth.org>
*/

#if !defined (__RSVG_RSVG_H_INSIDE__) && !defined (RSVG_COMPILATION)
#warning "Including <librsvg/rsvg-cairo.h> directly is deprecated."
#endif

#ifndef RSVG_CAIRO_H
#define RSVG_CAIRO_H

#include <cairo.h>

G_BEGIN_DECLS 

RSVG_API
gboolean    rsvg_handle_render_cairo     (RsvgHandle *handle, cairo_t *cr);
RSVG_API
gboolean    rsvg_handle_render_cairo_sub (RsvgHandle *handle, cairo_t *cr, const char *id);

RSVG_API
gboolean rsvg_handle_render_document (RsvgHandle           *handle,
                                      cairo_t              *cr,
                                      const RsvgRectangle  *viewport,
                                      GError              **error);

RSVG_API
gboolean rsvg_handle_get_geometry_for_layer (RsvgHandle     *handle,
                                             const char     *id,
                                             const RsvgRectangle *viewport,
                                             RsvgRectangle  *out_ink_rect,
                                             RsvgRectangle  *out_logical_rect,
                                             GError        **error);

RSVG_API
gboolean rsvg_handle_render_layer (RsvgHandle           *handle,
                                   cairo_t              *cr,
                                   const char           *id,
                                   const RsvgRectangle  *viewport,
                                   GError              **error);

RSVG_API
gboolean rsvg_handle_get_geometry_for_element (RsvgHandle     *handle,
                                               const char     *id,
                                               RsvgRectangle  *out_ink_rect,
                                               RsvgRectangle  *out_logical_rect,
                                               GError        **error);

RSVG_API
gboolean rsvg_handle_render_element (RsvgHandle           *handle,
                                     cairo_t              *cr,
                                     const char           *id,
                                     const RsvgRectangle  *element_viewport,
                                     GError              **error);

G_END_DECLS

#endif
