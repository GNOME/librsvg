/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
 * Copyright © 2010 Christian Persch
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public License as
 * published by the Free Software Foundation; either version 2.1 of the
 * License, or (at your option) any later version.
  
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
  
 * You should have received a copy of the GNU Lesser General Public
 * License along with this program; if not, write to the
 * Free Software Foundation, Inc., 59 Temple Place - Suite 330,
 * Boston, MA 02111-1307, USA.
*/

#include "config.h"

#include "rsvg-private.h"
#include "rsvg-xml.h"

typedef struct {
    GInputStream *stream;
    GCancellable *cancellable;
    GError      **error;
} RsvgXmlInputStreamContext;

/* this should use gsize, but libxml2 is borked */
static int
context_read (void *data,
              char *buffer,
              int   len)
{
    RsvgXmlInputStreamContext *context = data;
    gssize n_read;

    if (*(context->error))
        return -1;

    n_read = g_input_stream_read (context->stream, buffer, (gsize) len,
                                  context->cancellable,
                                  context->error);
    if (n_read < 0)
        return -1;

    return (int) n_read;
}

static int
context_close (void *data)
{
    RsvgXmlInputStreamContext *context = data;
    gboolean ret;

    /* Don't overwrite a previous error */
    ret = g_input_stream_close (context->stream, context->cancellable,
                                *(context->error) == NULL ? context->error : NULL);

    g_object_unref (context->stream);
    if (context->cancellable)
        g_object_unref (context->cancellable);
    g_slice_free (RsvgXmlInputStreamContext, context);

    return ret ? 0 : -1;
}

xmlParserCtxtPtr rsvg_create_xml_parser_from_stream (xmlSAXHandlerPtr sax,
						     void            *sax_user_data,
						     GInputStream    *stream,
						     GCancellable    *cancellable,
						     GError          **error)
{
    RsvgXmlInputStreamContext *context;
    xmlParserCtxtPtr parser;

    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error != NULL, NULL);

    context = g_slice_new (RsvgXmlInputStreamContext);
    context->stream = g_object_ref (stream);
    context->cancellable = cancellable ? g_object_ref (cancellable) : NULL;
    context->error = error;

    parser = xmlCreateIOParserCtxt (sax,
                                    sax_user_data,
                                    context_read,
                                    context_close,
                                    context,
                                    XML_CHAR_ENCODING_NONE);

    if (!parser) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error creating XML parser"));

        /* on error, xmlCreateIOParserCtxt() frees our context via the context_close function */
    }

    return parser;
}
