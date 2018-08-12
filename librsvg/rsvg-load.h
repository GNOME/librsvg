/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-load.h: Loading code for librsvg

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
*/

#ifndef RSVG_LOAD_H
#define RSVG_LOAD_H

#include <glib.h>
#include "rsvg-private.h"

G_GNUC_INTERNAL
RsvgLoad *rsvg_load_new (RsvgHandle *handle, gboolean unlimited_size) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
void rsvg_load_free (RsvgLoad *load);

G_GNUC_INTERNAL
RsvgTree *rsvg_load_steal_tree (RsvgLoad *load) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
gboolean rsvg_load_write (RsvgLoad *load, const guchar *buf, gsize count, GError **error) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
gboolean rsvg_load_close (RsvgLoad *load, GError **error) G_GNUC_WARN_UNUSED_RESULT;

G_GNUC_INTERNAL
gboolean rsvg_load_read_stream_sync (RsvgLoad *load, GInputStream *stream, GCancellable *cancellable, GError **error) G_GNUC_WARN_UNUSED_RESULT;

#endif
