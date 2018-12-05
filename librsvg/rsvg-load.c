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

#include "rsvg-attributes.h"
#include "rsvg-load.h"

typedef enum {
    LOAD_STATE_START,
    LOAD_STATE_EXPECTING_GZ_1,
    LOAD_STATE_READING_COMPRESSED,
    LOAD_STATE_READING,
    LOAD_STATE_CLOSED
} LoadState;

/* Implemented in rsvg_internals/src/xml.rs */
typedef struct RsvgXmlState RsvgXmlState;

/* Implemented in rsvg_internals/src/xml.rs */
extern RsvgXmlState *rsvg_xml_state_new (RsvgHandle *handle);
extern void rsvg_xml_state_free (RsvgXmlState *xml);
extern gboolean rsvg_xml_state_tree_is_valid(RsvgXmlState *xml, GError **error);
extern void rsvg_xml_state_error(RsvgXmlState *xml, const char *msg);

/* Implemented in rsvg_internals/src/handle.rs */
extern void rsvg_handle_rust_steal_result (RsvgHandleRust *raw_handle, RsvgXmlState *xml);

/* Implemented in rsvg_internals/src/xml2_load.rs */
extern xmlParserCtxtPtr rsvg_create_xml_stream_parser (RsvgXmlState  *xml,
                                                       gboolean       unlimited_size,
                                                       GInputStream  *stream,
                                                       GCancellable  *cancellable,
                                                       GError       **error);
extern xmlParserCtxtPtr rsvg_create_xml_push_parser (RsvgXmlState *xml,
                                                     gboolean unlimited_size,
                                                     const char *base_uri,
                                                     GError **error);


/* Holds the XML parsing state */
typedef struct {
    xmlParserCtxtPtr ctxt;

    RsvgXmlState *rust_state;
} XmlState;

/* Holds the GIO and loading state for compressed data */
struct RsvgLoad {
    RsvgHandle *handle;
    gboolean unlimited_size;

    LoadState state;

    GCancellable *cancellable;

    GError **error;

    GInputStream *compressed_input_stream; /* for rsvg_handle_write of svgz data */

    XmlState xml;
};

RsvgLoad *
rsvg_load_new (RsvgHandle *handle, gboolean unlimited_size)
{
    RsvgLoad *load = g_new0 (RsvgLoad, 1);

    load->handle = handle;
    load->unlimited_size = unlimited_size;
    load->state = LOAD_STATE_START;
    load->cancellable = NULL;
    load->error = NULL;
    load->compressed_input_stream = NULL;

    load->xml.ctxt = NULL;
    load->xml.rust_state = rsvg_xml_state_new (handle);

    return load;
}

static xmlParserCtxtPtr
free_xml_parser_and_doc (xmlParserCtxtPtr ctxt) G_GNUC_WARN_UNUSED_RESULT;

/* Frees the ctxt and its ctxt->myDoc - libxml2 doesn't free them together
 * http://xmlsoft.org/html/libxml-parser.html#xmlFreeParserCtxt
 *
 * Returns NULL.
 */
static xmlParserCtxtPtr
free_xml_parser_and_doc (xmlParserCtxtPtr ctxt)
{
    if (ctxt) {
        if (ctxt->myDoc) {
            xmlFreeDoc (ctxt->myDoc);
            ctxt->myDoc = NULL;
        }

        xmlFreeParserCtxt (ctxt);
    }

    return NULL;
}

void
rsvg_load_free (RsvgLoad *load)
{
    load->xml.ctxt = free_xml_parser_and_doc (load->xml.ctxt);

    g_clear_object (&load->compressed_input_stream);
    g_clear_pointer (&load->xml.rust_state, rsvg_xml_state_free);
    g_free (load);
}

gboolean
rsvg_load_finish_load (RsvgLoad *load, GError **error)
{
    gboolean was_successful = rsvg_xml_state_tree_is_valid(load->xml.rust_state, error);

    if (was_successful) {
        rsvg_handle_rust_steal_result (load->handle->priv->rust_handle, load->xml.rust_state);
    }

    return was_successful;
}

gboolean
rsvg_load_handle_xml_xinclude (RsvgHandle *handle, const char *href)
{
    GInputStream *stream;
    GError *err = NULL;
    xmlParserCtxtPtr xml_parser;

    g_assert (handle->priv->load != NULL);

    stream = rsvg_handle_acquire_stream (handle, href, NULL);

    if (stream) {
        gboolean success = FALSE;

        xml_parser = rsvg_create_xml_stream_parser (handle->priv->load->xml.rust_state,
                                                    handle->priv->load->unlimited_size,
                                                    stream,
                                                    handle->priv->cancellable,
                                                    &err);

        g_object_unref (stream);

        if (xml_parser) {
            success = xmlParseDocument (xml_parser) == 0;

            xml_parser = free_xml_parser_and_doc (xml_parser);
        }

        g_clear_error (&err);

        return success;
    } else {
        return FALSE;
    }
}

/* end xinclude */

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

static void
set_error_from_xml (GError **error, xmlParserCtxtPtr ctxt)
{
    xmlErrorPtr xerr;

    xerr = xmlCtxtGetLastError (ctxt);
    if (xerr) {
        g_set_error (error, rsvg_error_quark (), 0,
                     _("Error domain %d code %d on line %d column %d of %s: %s"),
                     xerr->domain, xerr->code,
                     xerr->line, xerr->int2,
                     xerr->file ? xerr->file : "data",
                     xerr->message ? xerr->message: "-");
    } else {
        g_set_error (error, rsvg_error_quark (), 0, _("Error parsing XML data"));
    }
}

static gboolean
write_impl (RsvgLoad *load, const guchar * buf, gsize count, GError **error)
{
    GError *real_error = NULL;
    int result;

    load->error = &real_error;

    if (load->xml.ctxt == NULL) {
        load->xml.ctxt = rsvg_create_xml_push_parser (load->xml.rust_state,
                                                      load->unlimited_size,
                                                      rsvg_handle_get_base_uri (load->handle),
                                                      &real_error);
    }

    if (load->xml.ctxt != NULL) {
        result = xmlParseChunk (load->xml.ctxt, (char *) buf, count, 0);
        if (result != 0) {
            set_error_from_xml (error, load->xml.ctxt);
            return FALSE;
        }
    } else {
        g_assert (real_error != NULL);
    }

    load->error = NULL;

    if (real_error != NULL) {
        g_propagate_error (error, real_error);
        return FALSE;
    }

    return TRUE;
}

static gboolean
close_impl (RsvgLoad *load, GError ** error)
{
    GError *real_error = NULL;

    load->error = &real_error;

    if (load->xml.ctxt != NULL) {
        int result;

        result = xmlParseChunk (load->xml.ctxt, "", 0, TRUE);
        if (result != 0) {
            set_error_from_xml (error, load->xml.ctxt);
            load->xml.ctxt = free_xml_parser_and_doc (load->xml.ctxt);
            return FALSE;
        }

        load->xml.ctxt = free_xml_parser_and_doc (load->xml.ctxt);
    }

    load->error = NULL;

    if (real_error != NULL) {
        g_propagate_error (error, real_error);
        return FALSE;
    }

    return TRUE;
}

#define GZ_MAGIC_0 ((guchar) 0x1f)
#define GZ_MAGIC_1 ((guchar) 0x8b)

/* Implemented in rsvg_internals/src/io.rs */
extern GInputStream *
rsvg_get_input_stream_for_loading (GInputStream *stream,
                                   GCancellable *cancellable,
                                   GError      **error);

gboolean
rsvg_load_read_stream_sync (RsvgLoad     *load,
                            GInputStream *stream,
                            GCancellable *cancellable,
                            GError      **error)
{
    GError *err = NULL;
    gboolean res = FALSE;

    stream = rsvg_get_input_stream_for_loading (stream, cancellable, error);
    if (stream == NULL) {
        load->state = LOAD_STATE_CLOSED;
        return FALSE;
    }

    load->error = &err;
    load->cancellable = cancellable ? g_object_ref (cancellable) : NULL;

    g_assert (load->xml.ctxt == NULL);
    load->xml.ctxt = rsvg_create_xml_stream_parser (load->xml.rust_state,
                                                    load->unlimited_size,
                                                    stream,
                                                    cancellable,
                                                    &err);

    if (!load->xml.ctxt) {
        g_assert (err != NULL);
        g_propagate_error (error, err);

        goto out;
    }

    if (xmlParseDocument (load->xml.ctxt) != 0) {
        if (err) {
            g_propagate_error (error, err);
        } else {
            set_error_from_xml (error, load->xml.ctxt);
        }

        goto out;
    }

    if (err != NULL) {
        g_propagate_error (error, err);
        goto out;
    }

    res = TRUE;

  out:

    load->xml.ctxt = free_xml_parser_and_doc (load->xml.ctxt);

    g_object_unref (stream);

    load->error = NULL;
    g_clear_object (&load->cancellable);

    return res;
}

/* Creates handle->priv->compressed_input_stream and adds the gzip header data
 * to it.  We implicitly consume the header data from the caller in
 * rsvg_handle_write(); that's why we add it back here.
 */
static void
create_compressed_input_stream (RsvgLoad *load)
{
    static const guchar gz_magic[2] = { GZ_MAGIC_0, GZ_MAGIC_1 };

    g_assert (load->compressed_input_stream == NULL);

    load->compressed_input_stream = g_memory_input_stream_new ();
    g_memory_input_stream_add_data (G_MEMORY_INPUT_STREAM (load->compressed_input_stream),
                                    gz_magic, 2, NULL);
}

gboolean
rsvg_load_write (RsvgLoad *load, const guchar *buf, gsize count, GError **error)
{
    g_assert (load->state == LOAD_STATE_START
              || load->state == LOAD_STATE_EXPECTING_GZ_1
              || load->state == LOAD_STATE_READING_COMPRESSED
              || load->state == LOAD_STATE_READING);

    while (count > 0) {
        switch (load->state) {
        case LOAD_STATE_START:
            if (buf[0] == GZ_MAGIC_0) {
                load->state = LOAD_STATE_EXPECTING_GZ_1;
                buf++;
                count--;
            } else {
                load->state = LOAD_STATE_READING;
                return write_impl (load, buf, count, error);
            }

            break;

        case LOAD_STATE_EXPECTING_GZ_1:
            if (buf[0] == GZ_MAGIC_1) {
                load->state = LOAD_STATE_READING_COMPRESSED;
                create_compressed_input_stream (load);
                buf++;
                count--;
            } else {
                load->state = LOAD_STATE_READING;
                return write_impl (load, buf, count, error);
            }

            break;

        case LOAD_STATE_READING_COMPRESSED:
            g_memory_input_stream_add_data (G_MEMORY_INPUT_STREAM (load->compressed_input_stream),
                                            g_memdup (buf, count), count, (GDestroyNotify) g_free);
            return TRUE;

        case LOAD_STATE_READING:
            return write_impl (load, buf, count, error);

        default:
            g_assert_not_reached ();
        }
    }

    return TRUE;
}

gboolean
rsvg_load_close (RsvgLoad *load, GError **error)
{
    gboolean res;

    if (load->state == LOAD_STATE_READING_COMPRESSED) {

        /* FIXME: when using rsvg_handle_write()/rsvg_handle_close(), as opposed to using the
         * stream functions, for compressed SVGs we buffer the whole compressed file in memory
         * and *then* uncompress/parse it here.
         *
         * We should make it so that the incoming data is decompressed and parsed on the fly.
         */
        load->state = LOAD_STATE_START;
        res = rsvg_load_read_stream_sync (load, load->compressed_input_stream, NULL, error);
        g_clear_object (&load->compressed_input_stream);
    } else {
        res = close_impl (load, error);
    }

    if (!res) {
        g_clear_pointer (&load->xml.rust_state, rsvg_xml_state_free);
    }

    load->state = LOAD_STATE_CLOSED;

    return res;
}
