/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-gz.c: SAX-based renderer for SVGZ files into a GdkPixbuf.

   Copyright (C) 2003 Dom Lachowicz <cinamod@hotmail.com>

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

#include "config.h"
#include "rsvg-gz.h"
#include "rsvg-private.h"

/**
 * rsvg_handle_new_gz
 *
 * DEPRECATED. Please use rsvg_handle_new () instead.
 *
 * Returns: a new SVGZ handle
 */
RsvgHandle *
rsvg_handle_new_gz (void)
{
	return rsvg_handle_new ();
}
