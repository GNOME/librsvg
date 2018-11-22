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

#include "rsvg-attributes.h"
#include "rsvg-load.h"

typedef enum {
    LOAD_STATE_START,
    LOAD_STATE_EXPECTING_GZ_1,
    LOAD_STATE_READING_COMPRESSED,
    LOAD_STATE_READING,
    LOAD_STATE_CLOSED
} LoadState;

/* Implemented in rsvg_internals/src/load.rs */
G_GNUC_INTERNAL
void rsvg_load_set_svg_node_atts (RsvgHandle *handle, RsvgNode *node);

/* Implemented in rsvg_internals/src/node.rs */
G_GNUC_INTERNAL
void rsvg_node_register_in_defs(RsvgNode *node, RsvgDefs *defs);

/* Implemented in rsvg_internals/src/xml.rs */
typedef struct RsvgXmlState RsvgXmlState;

/* Implemented in rsvg_internals/src/xml.rs */
extern RsvgXmlState *rsvg_xml_state_new ();
extern void rsvg_xml_state_free (RsvgXmlState *xml);
extern void rsvg_xml_state_steal_result(RsvgXmlState *xml,
                                        RsvgTree **out_tree,
                                        RsvgDefs **out_defs);
extern void rsvg_xml_state_start_element(RsvgXmlState *xml, RsvgHandle *handle, const char *name, RsvgPropertyBag atts);
extern void rsvg_xml_state_end_element(RsvgXmlState *xml, RsvgHandle *handle, const char *name);
extern void rsvg_xml_state_characters(RsvgXmlState *xml, const char *unterminated_text, gsize len);


/* Holds the XML parsing state */
typedef struct {
    GHashTable *entities;       /* g_malloc'd string -> xmlEntityPtr */

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

static xmlSAXHandler get_xml2_sax_handler (void);

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

    load->xml.entities = g_hash_table_new_full (g_str_hash,
                                                g_str_equal,
                                                g_free,
                                                (GDestroyNotify) xmlFreeNode);
    load->xml.ctxt = NULL;
    load->xml.rust_state = rsvg_xml_state_new ();

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
    g_hash_table_destroy (load->xml.entities);

    load->xml.ctxt = free_xml_parser_and_doc (load->xml.ctxt);

    g_clear_object (&load->compressed_input_stream);
    g_clear_pointer (&load->xml.rust_state, rsvg_xml_state_free);
    g_free (load);
}

void
rsvg_load_steal_result (RsvgLoad *load,
                        RsvgTree **out_tree,
                        RsvgDefs **out_defs)
{
    rsvg_xml_state_steal_result (load->xml.rust_state, out_tree, out_defs);
}

static void
set_xml_parse_options(xmlParserCtxtPtr xml_parser,
                      gboolean unlimited_size)
{
    int options;

    options = (XML_PARSE_NONET |
               XML_PARSE_BIG_LINES);

    if (unlimited_size) {
        options |= XML_PARSE_HUGE;
    }

    xmlCtxtUseOptions (xml_parser, options);

    /* if false, external entities work, but internal ones don't. if true, internal entities
       work, but external ones don't. favor internal entities, in order to not cause a
       regression */
    xml_parser->replaceEntities = TRUE;
}

static xmlParserCtxtPtr
create_xml_push_parser (RsvgLoad *load,
                        const char *base_uri)
{
    xmlParserCtxtPtr parser;
    xmlSAXHandler sax_handler = get_xml2_sax_handler ();

    parser = xmlCreatePushParserCtxt (&sax_handler, load, NULL, 0, base_uri);
    set_xml_parse_options (parser, load->unlimited_size);

    return parser;
}

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

static xmlParserCtxtPtr
create_xml_stream_parser (RsvgLoad      *load,
                          GInputStream  *stream,
                          GCancellable  *cancellable,
                          GError       **error)
{
    RsvgXmlInputStreamContext *context;
    xmlParserCtxtPtr parser;
    xmlSAXHandler sax_handler = get_xml2_sax_handler ();

    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error != NULL, NULL);

    context = g_slice_new (RsvgXmlInputStreamContext);
    context->stream = g_object_ref (stream);
    context->cancellable = cancellable ? g_object_ref (cancellable) : NULL;
    context->error = error;

    parser = xmlCreateIOParserCtxt (&sax_handler,
                                    load,
                                    context_read,
                                    context_close,
                                    context,
                                    XML_CHAR_ENCODING_NONE);

    if (!parser) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error creating XML parser"));

        /* on error, xmlCreateIOParserCtxt() frees our context via the context_close function */
    } else {
        set_xml_parse_options (parser, load->unlimited_size);
    }

    return parser;
}

gboolean
rsvg_load_handle_xml_xinclude (RsvgHandle *handle, const char *url)
{
    GInputStream *stream;
    GError *err = NULL;
    xmlParserCtxtPtr xml_parser;
    gchar *mime_type;

    g_assert (handle->priv->load != NULL);

    stream = _rsvg_handle_acquire_stream (handle, url, &mime_type, NULL);

    g_free (mime_type);

    if (stream) {
        gboolean success = FALSE;

        xml_parser = create_xml_stream_parser (handle->priv->load,
                                               stream,
                                               NULL, /* cancellable */
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

static void
sax_start_element_cb (void *data, const xmlChar * name, const xmlChar ** atts)
{
    RsvgPropertyBag bag;
    RsvgLoad *load = data;
    const char *tempname;

    bag = rsvg_property_bag_new ((const char **) atts);

    for (tempname = (const char *) name; *tempname != '\0'; tempname++) {
        if (*tempname == ':') {
            name = (const xmlChar *) (tempname + 1);
        }
    }

    rsvg_xml_state_start_element (load->xml.rust_state,
                                  load->handle,
                                  (const char *) name,
                                  bag);

    rsvg_property_bag_free (bag);
}

static void
sax_end_element_cb (void *data, const xmlChar * xmlname)
{
    RsvgLoad *load =  data;
    const char *name = (const char *) xmlname;
    const char *tempname;

    for (tempname = name; *tempname != '\0'; tempname++) {
        if (*tempname == ':') {
            name = tempname + 1;
        }
    }

    rsvg_xml_state_end_element (load->xml.rust_state, load->handle, name);
}

static void
sax_characters_cb (void *data, const xmlChar * ch, int len)
{
    RsvgLoad *load = data;

    rsvg_xml_state_characters (load->xml.rust_state, (const char *) ch, (gsize) len);
}

static xmlEntityPtr
sax_get_entity_cb (void *data, const xmlChar * name)
{
    RsvgLoad *load = data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (load->xml.entities, name);

    return entity;
}

static void
sax_entity_decl_cb (void *data, const xmlChar * name, int type,
                    const xmlChar * publicId, const xmlChar * systemId, xmlChar * content)
{
    RsvgLoad *load = data;
    xmlEntityPtr entity;
    xmlChar *resolvedSystemId = NULL, *resolvedPublicId = NULL;

    if (systemId)
        resolvedSystemId = xmlBuildRelativeURI (systemId, (xmlChar*) rsvg_handle_get_base_uri (load->handle));
    else if (publicId)
        resolvedPublicId = xmlBuildRelativeURI (publicId, (xmlChar*) rsvg_handle_get_base_uri (load->handle));

    if (type == XML_EXTERNAL_PARAMETER_ENTITY && !content) {
        char *entity_data;
        gsize entity_data_len;

        if (systemId)
            entity_data = _rsvg_handle_acquire_data (load->handle,
                                                     (const char *) systemId,
                                                     NULL,
                                                     &entity_data_len,
                                                     NULL);
        else if (publicId)
            entity_data = _rsvg_handle_acquire_data (load->handle,
                                                     (const char *) publicId,
                                                     NULL,
                                                     &entity_data_len,
                                                     NULL);
        else
            entity_data = NULL;

        if (entity_data) {
            content = xmlCharStrndup (entity_data, entity_data_len);
            g_free (entity_data);
        }
    }

    entity = xmlNewEntity(NULL, name, type, resolvedPublicId, resolvedSystemId, content);

    xmlFree(resolvedPublicId);
    xmlFree(resolvedSystemId);

    g_hash_table_insert (load->xml.entities, g_strdup ((const char*) name), entity);
}

static void
sax_unparsed_entity_decl_cb (void *data,
                             const xmlChar * name,
                             const xmlChar * publicId,
                             const xmlChar * systemId, const xmlChar * notationName)
{
    sax_entity_decl_cb (data, name, XML_INTERNAL_GENERAL_ENTITY, publicId, systemId, NULL);
}

static xmlEntityPtr
sax_get_parameter_entity_cb (void *data, const xmlChar * name)
{
    RsvgLoad *load = data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (load->xml.entities, name);

    return entity;
}

static void
sax_error_cb (void *data, const char *msg, ...)
{
#ifdef G_ENABLE_DEBUG
    va_list args;

    va_start (args, msg);
    vfprintf (stderr, msg, args);
    va_end (args);
#endif
}

static void
xml_noerror (void *data, xmlErrorPtr error)
{
}

/* This is quite hacky and not entirely correct, but apparently
 * libxml2 has NO support for parsing pseudo attributes as defined
 * by the xml-styleheet spec.
 */
static char **
parse_xml_attribute_string (const char *attribute_string)
{
    xmlSAXHandler handler;
    xmlParserCtxtPtr parser;
    xmlDocPtr doc;
    xmlNodePtr node;
    xmlAttrPtr attr;
    char *tag;
    GPtrArray *attributes;
    char **retval = NULL;

    tag = g_strdup_printf ("<rsvg-hack %s />\n", attribute_string);

    memset (&handler, 0, sizeof (handler));
    xmlSAX2InitDefaultSAXHandler (&handler, 0);
    handler.serror = xml_noerror;
    parser = xmlCreatePushParserCtxt (&handler, NULL, tag, strlen (tag) + 1, NULL);
    parser->options |= XML_PARSE_NONET;

    if (xmlParseDocument (parser) != 0)
        goto done;

    if ((doc = parser->myDoc) == NULL ||
        (node = doc->children) == NULL ||
        strcmp ((const char *) node->name, "rsvg-hack") != 0 ||
        node->next != NULL ||
        node->properties == NULL)
          goto done;

    attributes = g_ptr_array_new ();
    for (attr = node->properties; attr; attr = attr->next) {
        xmlNodePtr content = attr->children;

        g_ptr_array_add (attributes, g_strdup ((char *) attr->name));
        if (content)
            g_ptr_array_add (attributes, g_strdup ((char *) content->content));
        else
            g_ptr_array_add (attributes, g_strdup (""));
    }

    g_ptr_array_add (attributes, NULL);
    retval = (char **) g_ptr_array_free (attributes, FALSE);

  done:
    if (parser->myDoc)
        xmlFreeDoc (parser->myDoc);
    xmlFreeParserCtxt (parser);
    g_free (tag);

    return retval;
}

static void
sax_processing_instruction_cb (void *user_data, const xmlChar * target, const xmlChar * data)
{
    /* http://www.w3.org/TR/xml-stylesheet/ */
    RsvgLoad *load = user_data;

    if (!strcmp ((const char *) target, "xml-stylesheet")) {
        RsvgPropertyBag *atts;
        char **xml_atts;

        xml_atts = parse_xml_attribute_string ((const char *) data);

        if (xml_atts) {
            const char *alternate = NULL;
            const char *type = NULL;
            const char *href = NULL;
            RsvgPropertyBagIter *iter;
            const char *key;
            RsvgAttribute attr;
            const char *value;

            atts = rsvg_property_bag_new ((const char **) xml_atts);

            iter = rsvg_property_bag_iter_begin (atts);

            while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
                switch (attr) {
                case RSVG_ATTRIBUTE_ALTERNATE:
                    alternate = value;
                    break;

                case RSVG_ATTRIBUTE_TYPE:
                    type = value;
                    break;

                case RSVG_ATTRIBUTE_HREF:
                    href = value;
                    break;

                default:
                    break;
                }
            }

            rsvg_property_bag_iter_end (iter);

            if ((!alternate || strcmp (alternate, "no") != 0)
                && type && strcmp (type, "text/css") == 0
                && href) {
                char *style_data;
                gsize style_data_len;
                char *mime_type = NULL;

                style_data = _rsvg_handle_acquire_data (load->handle,
                                                        href,
                                                        &mime_type,
                                                        &style_data_len,
                                                        NULL);
                if (style_data &&
                    mime_type &&
                    strcmp (mime_type, "text/css") == 0) {
                    rsvg_css_parse_into_handle (load->handle, style_data, style_data_len);
                }

                g_free (mime_type);
                g_free (style_data);
            }

            rsvg_property_bag_free (atts);
            g_strfreev (xml_atts);
        }
    }
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
        load->xml.ctxt = create_xml_push_parser (load, rsvg_handle_get_base_uri (load->handle));
    }

    result = xmlParseChunk (load->xml.ctxt, (char *) buf, count, 0);
    if (result != 0) {
        set_error_from_xml (error, load->xml.ctxt);
        return FALSE;
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

gboolean
rsvg_load_read_stream_sync (RsvgLoad     *load,
                            GInputStream *stream,
                            GCancellable *cancellable,
                            GError      **error)
{
    GError *err = NULL;
    gboolean res = FALSE;
    const guchar *buf;
    gssize num_read;

    /* detect zipped streams */
    stream = g_buffered_input_stream_new (stream);
    num_read = g_buffered_input_stream_fill (G_BUFFERED_INPUT_STREAM (stream), 2, cancellable, error);
    if (num_read < 2) {
        g_object_unref (stream);
        if (num_read < 0) {
            g_assert (error == NULL || *error != NULL);
        } else {
            g_set_error (error, rsvg_error_quark (), RSVG_ERROR_FAILED,
                         _("Input file is too short"));
        }

        load->state = LOAD_STATE_CLOSED;
        return res;
    }

    buf = g_buffered_input_stream_peek_buffer (G_BUFFERED_INPUT_STREAM (stream), NULL);
    if ((buf[0] == GZ_MAGIC_0) && (buf[1] == GZ_MAGIC_1)) {
        GConverter *converter;
        GInputStream *conv_stream;

        converter = G_CONVERTER (g_zlib_decompressor_new (G_ZLIB_COMPRESSOR_FORMAT_GZIP));
        conv_stream = g_converter_input_stream_new (stream, converter);
        g_object_unref (converter);
        g_object_unref (stream);

        stream = conv_stream;
    }

    load->error = &err;
    load->cancellable = cancellable ? g_object_ref (cancellable) : NULL;

    g_assert (load->xml.ctxt == NULL);
    load->xml.ctxt = create_xml_stream_parser (load,
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

static xmlSAXHandler
get_xml2_sax_handler (void)
{
    xmlSAXHandler sax_handler;

    memset (&sax_handler, 0, sizeof (sax_handler));

    sax_handler.getEntity = sax_get_entity_cb;
    sax_handler.entityDecl = sax_entity_decl_cb;
    sax_handler.unparsedEntityDecl = sax_unparsed_entity_decl_cb;
    sax_handler.getParameterEntity = sax_get_parameter_entity_cb;
    sax_handler.characters = sax_characters_cb;
    sax_handler.error = sax_error_cb;
    sax_handler.cdataBlock = sax_characters_cb;
    sax_handler.startElement = sax_start_element_cb;
    sax_handler.endElement = sax_end_element_cb;
    sax_handler.processingInstruction = sax_processing_instruction_cb;

    return sax_handler;
}
