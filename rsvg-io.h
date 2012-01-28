/*
   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002, 2003, 2004, 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003, 2004, 2005 Caleb Moore <c.moore@student.unsw.edu.au>
   Copyright © 2011, 2012 Christian Persch

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

#ifndef RSVG_IO_H
#define RSVG_IO_H

#include <glib.h>
#include <gio/gio.h>

guint8* _rsvg_io_acquire_data (const char *uri,
                               const char *base_uri,
                               char **mime_type,
                               gsize *len,
                               GCancellable *cancellable,
                               GError **error);

GInputStream *_rsvg_io_acquire_stream (const char *uri,
                                       const char *base_uri,
                                       char **mime_type,
                                       GCancellable *cancellable,
                                       GError **error);

#endif /* RSVG_IO_H */
