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

#include "config.h"

#include <libxml/uri.h>
#include <libxml/parser.h>
#include <libxml/parserInternals.h>
#include <string.h>
#include <glib/gprintf.h>

#include "rsvg-load.h"

typedef enum {
    LOAD_STATE_START,
    LOAD_STATE_READING,
    LOAD_STATE_CLOSED
} LoadState;

/* Implemented in rsvg_internals/src/xml.rs */
extern RsvgXmlState *rsvg_xml_state_new (RsvgHandle *handle);
extern void rsvg_xml_state_error(RsvgXmlState *xml, const char *msg);

/* Implemented in rsvg_internals/src/xml2_load.rs */
extern gboolean rsvg_xml_state_load_from_possibly_compressed_stream (RsvgXmlState *xml,
                                                                     gboolean      unlimited_size,
                                                                     GInputStream *stream,
                                                                     GCancellable *cancellable,
                                                                     GError      **error);

/* Holds the GIO and loading state for compressed data */
struct RsvgLoad {
    RsvgHandle *handle;

    LoadState state;
    GByteArray *buffer;

    RsvgXmlState *rust_state;
};

RsvgLoad *
rsvg_load_new (RsvgHandle *handle)
{
    RsvgLoad *load = g_new0 (RsvgLoad, 1);

    load->handle = handle;
    load->state = LOAD_STATE_START;
    load->buffer = NULL;

    load->rust_state = rsvg_xml_state_new (handle);

    return load;
}

RsvgXmlState *
rsvg_load_free (RsvgLoad *load)
{
    RsvgXmlState *rust_state;

    if (load->buffer) {
        g_byte_array_free (load->buffer, TRUE);
    }

    rust_state = load->rust_state;
    g_free (load);

    return rust_state;
}

/* This one is defined in the C code, because the prototype has varargs
 * and we can't handle those from Rust :(
 */
G_GNUC_INTERNAL void rsvg_sax_error_cb (void *data, const char *msg, ...);

void
rsvg_sax_error_cb (void *data, const char *msg, ...)
{
    RsvgXmlState *xml = data;
    va_list args;
    char *buf;

    va_start (args, msg);
    g_vasprintf (&buf, msg, args);
    va_end (args);

    rsvg_xml_state_error (xml, buf);

    g_free (buf);
}

gboolean
rsvg_load_read_stream_sync (RsvgLoad     *load,
                            GInputStream *stream,
                            GCancellable *cancellable,
                            GError      **error)
{
    gboolean res = FALSE;
    gboolean unlimited_size = (rsvg_handle_get_flags (load->handle) && RSVG_HANDLE_FLAG_UNLIMITED) != 0;

    res = rsvg_xml_state_load_from_possibly_compressed_stream (load->rust_state,
                                                               unlimited_size,
                                                               stream,
                                                               cancellable,
                                                               error);
    load->state = LOAD_STATE_CLOSED;

    return res;
}

gboolean
rsvg_load_write (RsvgLoad *load, const guchar *buf, gsize count, GError **error)
{
    switch (load->state) {
    case LOAD_STATE_START:
        g_assert (load->buffer == NULL);

        load->buffer = g_byte_array_new();
        g_byte_array_append (load->buffer, buf, count);

        load->state = LOAD_STATE_READING;
        break;

    case LOAD_STATE_READING:
        g_byte_array_append (load->buffer, buf, count);
        break;

    default:
        g_assert_not_reached ();
    }

    return TRUE;
}

gboolean
rsvg_load_close (RsvgLoad *load, GError **error)
{
    gboolean res;

    switch (load->state) {
    case LOAD_STATE_START:
    case LOAD_STATE_CLOSED:
        return TRUE;

    case LOAD_STATE_READING: {
        GInputStream *stream;
        GBytes *bytes;

        bytes = g_byte_array_free_to_bytes (load->buffer);
        load->buffer = NULL;

        stream = g_memory_input_stream_new_from_bytes (bytes);
        g_bytes_unref (bytes);
        
        res = rsvg_load_read_stream_sync (load, stream, NULL, error);
        g_clear_object (&stream);
        break;
    }

    default:
        g_assert_not_reached();
    }

    load->state = LOAD_STATE_CLOSED;

    return res;
}
