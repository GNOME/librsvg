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

#include <gsf/gsf-input-gzip.h>
#include <gsf/gsf-input-memory.h>
#include <gsf/gsf-output-memory.h>

/* TODO: this could probably be done about a billion times better */

struct RsvgHandleGz
{
	RsvgHandle super;
	GsfOutput * mem;
};

typedef struct RsvgHandleGz RsvgHandleGz;

static gboolean
rsvg_handle_gz_write_impl (RsvgHandle    *handle,
						   const guchar  *buf,
						   gsize          num_bytes,
						   GError       **error)
{
	RsvgHandleGz * me = (RsvgHandleGz*)handle;
	return gsf_output_write (me->mem, num_bytes, buf);
}

static gboolean
rsvg_handle_gz_close_impl (RsvgHandle  *handle,
						   GError     **error)
{
	RsvgHandleGz * me = (RsvgHandleGz*)handle;
	GsfInput * gzip;
	const guchar * bytes;
	gsize size;

	bytes = gsf_output_memory_get_bytes (GSF_OUTPUT_MEMORY (me->mem));
	size = gsf_output_size (me->mem);

	gzip = GSF_INPUT (gsf_input_gzip_new (gsf_input_memory_new (bytes, size, FALSE), error));
	while (TRUE) {
		size = MIN (gsf_input_remaining (gzip), 1024);
		if (size == 0) break;

		/* write to parent */
		rsvg_handle_write_impl (&(me->super),
								gsf_input_read (gzip, size, NULL),
								size, error);
	}
	gsf_input_close (gzip);
	g_object_unref (gzip);

	/* close parent */
	gsf_output_close (me->mem);
	return rsvg_handle_close_impl (handle, error);
}

static void
rsvg_handle_gz_free_impl (RsvgHandle *handle)
{
	RsvgHandleGz * me = (RsvgHandleGz*)handle;
	g_object_unref (G_OBJECT (me->mem));

	/* free parent */
	rsvg_handle_free_impl (handle);
}

/**
 * See rsvg_handle_new, except that this will handle GZipped SVGs (svgz)
 * Use the returned handle identically to how you use a handle returned
 * from rsvg_handle_new()
 */
RsvgHandle *
rsvg_handle_new_gz (void)
{
	RsvgHandleGz * me = g_new0 (RsvgHandleGz, 1);

	/* init parent */
	rsvg_handle_init (&me->super);
	me->mem = gsf_output_memory_new ();

	me->super.write = rsvg_handle_gz_write_impl;
	me->super.close = rsvg_handle_gz_close_impl;
	me->super.free  = rsvg_handle_gz_free_impl;

	return (RsvgHandle*)me;
}
