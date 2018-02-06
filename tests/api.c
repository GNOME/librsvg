/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include "config.h"

#include <stdio.h>
#include <glib.h>
#include "rsvg.h"
#include "test-utils.h"

/*
RSVG_G_TYPE_INIT
rsvg_init
rsvg_term
rsvg_cleanup
rsvg_error_quark
rsvg_handle_free
rsvg_handle_close
rsvg_handle_get_dimensions
rsvg_handle_get_dimensions_sub
rsvg_handle_get_position_sub
rsvg_handle_get_pixbuf
rsvg_handle_get_pixbuf_sub
rsvg_handle_get_base_uri
rsvg_handle_set_base_uri
rsvg_handle_set_size_callback
rsvg_handle_has_sub
rsvg_handle_internal_set_testing
rsvg_handle_new_from_file
rsvg_handle_new_from_gfile_sync
rsvg_handle_new_with_flags
rsvg_handle_new_from_stream_sync
rsvg_handle_new_from_data
rsvg_handle_render_cairo
rsvg_handle_set_base_gfile
rsvg_handle_write
rsvg_handle_read_stream_sync
rsvg_set_default_dpi
rsvg_set_default_dpi_x_y
rsvg_handle_set_dpi
rsvg_handle_set_dpi_x_y
rsvg_pixbuf_from_file
rsvg_pixbuf_from_file_at_zoom
rsvg_pixbuf_from_file_at_size
rsvg_pixbuf_from_file_at_max_size
rsvg_pixbuf_from_file_at_zoom_with_max
rsvg_handle_get_title
rsvg_handle_get_desc
rsvg_handle_get_metadata
rsvg_handle_render_cairo_sub

RSVG_TYPE_ERROR -> rsvg_error_get_type
RSVG_TYPE_HANDLE_FLAGS -> rsvg_handle_flags_get_type
*/

static void
handle_has_gtype (void)
{
    RsvgHandle *handle;

    handle = rsvg_handle_new();
    g_assert (G_OBJECT_TYPE (handle) == rsvg_handle_get_type ());
    g_object_unref (handle);
}

int
main (int argc, char **argv)
{
    g_test_init (&argc, &argv, NULL);

    g_test_add_func ("/api/handle_has_gtype", handle_has_gtype);

    return g_test_run ();
}
