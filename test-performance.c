/* -*- Mode: C; indent-tabs-mode: t; c-basic-offset: 8; tab-width: 8 -*-

   test-performance.c: performance tests.
 
   Copyright (C) 2002 Ximian, Inc.
  
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
  
   Author: Michael Meeks <michael@ximian.com>
*/

#include <config.h>
#include <glib.h>
#include <stdio.h>
#include <stdlib.h>

#include "rsvg.h"

int
main (int argc, const char **argv)
{
	int         i, count = 10;
	GTimer     *timer;
	GdkPixbuf  *pixbuf;
	const char *fname;

	g_type_init ();

	fname = argv [argc - 1];
	fprintf (stderr, "File '%s'\n", fname);

	timer = g_timer_new ();
	g_timer_start (timer);

	for (i = 0; i < count; i++) {
		pixbuf = rsvg_pixbuf_from_file_at_zoom (
			fname, 1.5, 1.5, NULL);
		g_object_unref (pixbuf);
	}

	fprintf (stderr, "Scaling took %g(s)\n",
		 g_timer_elapsed (timer, NULL) / count);

	return 0;
}
