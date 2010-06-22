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

#include "rsvg-xml.h"

typedef struct {
    GInputStream *stream;
    GCancellable *cancellable;
    GError      **error;
} RsvgXmlInputStreamContext;

/* this should use gsize, but libxml2 is borked */
static int
context_read (RsvgXmlInputStreamContext *context,
              char *buffer,
              int   len)
{
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
context_close (RsvgXmlInputStreamContext *context)
{
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

/**
 * _rsvg_xml_input_buffer_new_from_stream:
 * @context: a #xmlParserCtxtPtr
 * @input_stream: a #GInputStream
 *
 * Returns: a new #xmlParserInputPtr wrapping @input_stream
 */
xmlParserInputBufferPtr
_rsvg_xml_input_buffer_new_from_stream (GInputStream   *stream,
                                        GCancellable   *cancellable,
                                        xmlCharEncoding enc,
                                        GError        **error)

{
    RsvgXmlInputStreamContext *context;

    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error != NULL, NULL);

    context = g_slice_new (RsvgXmlInputStreamContext);
    context->stream = g_object_ref (stream);
    context->cancellable = cancellable ? g_object_ref (cancellable) : NULL;
    context->error = error;

    return xmlParserInputBufferCreateIO ((xmlInputReadCallback) context_read,
                                         (xmlInputCloseCallback) context_close,
                                         context,
                                         enc);
}
