/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

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

   Author: Raph Levien <raph@artofcode.com>
*/

#include "config.h"
#define _GNU_SOURCE 1

#include "rsvg.h"
#include "rsvg-compat.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-structure.h"
#include "rsvg-image.h"
#include "rsvg-io.h"
#include "rsvg-text.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"
#include "rsvg-cairo-render.h"

#include <libxml/uri.h>
#include <libxml/parser.h>
#include <libxml/parserInternals.h>

#include <gio/gio.h>

#include <math.h>
#include <string.h>
#include <stdarg.h>
#include <limits.h>
#include <stdlib.h>

#include "rsvg-path-builder.h"
#include "rsvg-paint-server.h"
#include "rsvg-xml.h"

#ifdef G_OS_WIN32
static char *
rsvg_realpath_utf8 (const char *filename, const char *unused)
{
    wchar_t *wfilename;
    wchar_t *wfull;
    char *full;

    wfilename = g_utf8_to_utf16 (filename, -1, NULL, NULL, NULL);
    if (!wfilename)
        return NULL;

    wfull = _wfullpath (NULL, wfilename, 0);
    g_free (wfilename);
    if (!wfull)
        return NULL;

    full = g_utf16_to_utf8 (wfull, -1, NULL, NULL, NULL);
    free (wfull);

    if (!full)
        return NULL;

    return full;
}

#define realpath(a,b) rsvg_realpath_utf8 (a, b)
#endif

/*
 * This is configurable at runtime
 */
#define RSVG_DEFAULT_DPI_X 90.0
#define RSVG_DEFAULT_DPI_Y 90.0

G_GNUC_INTERNAL
double rsvg_internal_dpi_x = RSVG_DEFAULT_DPI_X;
G_GNUC_INTERNAL
double rsvg_internal_dpi_y = RSVG_DEFAULT_DPI_Y;

static xmlSAXHandler rsvgSAXHandlerStruct;
static gboolean rsvgSAXHandlerStructInited = FALSE;

typedef struct _RsvgSaxHandlerDefs {
    RsvgSaxHandler super;
    RsvgHandle *ctx;
} RsvgSaxHandlerDefs;

typedef struct _RsvgSaxHandlerStyle {
    RsvgSaxHandler super;
    RsvgSaxHandlerDefs *parent;
    RsvgHandle *ctx;
    GString *style;
    gboolean is_text_css;
} RsvgSaxHandlerStyle;

typedef struct {
    RsvgSaxHandler super;
    RsvgHandle *ctx;
    const char *name;
    GString *string;
    GString **stringptr;
} RsvgSaxHandlerExtra;

/* hide this fact from the general public */
typedef RsvgSaxHandlerExtra RsvgSaxHandlerTitle;
typedef RsvgSaxHandlerExtra RsvgSaxHandlerDesc;
typedef RsvgSaxHandlerExtra RsvgSaxHandlerMetadata;

static void
rsvg_style_handler_free (RsvgSaxHandler * self)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    RsvgHandle *ctx = z->ctx;

    if (z->is_text_css)
        rsvg_parse_cssbuffer (ctx, z->style->str, z->style->len);

    g_string_free (z->style, TRUE);
    g_free (z);
}

static void
rsvg_style_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    g_string_append_len (z->style, ch, len);
}

static void
rsvg_style_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
}

static void
rsvg_style_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    RsvgHandle *ctx = z->ctx;
    RsvgSaxHandler *prev = &z->parent->super;

    if (!strcmp (name, "style")) {
        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = prev;
        }
    }
}

static void
rsvg_start_style (RsvgHandle * ctx, RsvgPropertyBag *atts)
{
    RsvgSaxHandlerStyle *handler = g_new0 (RsvgSaxHandlerStyle, 1);
    const char *type;

    type = rsvg_property_bag_lookup (atts, "type");

    handler->super.free = rsvg_style_handler_free;
    handler->super.characters = rsvg_style_handler_characters;
    handler->super.start_element = rsvg_style_handler_start;
    handler->super.end_element = rsvg_style_handler_end;
    handler->ctx = ctx;

    handler->style = g_string_new (NULL);
    handler->is_text_css = type && g_ascii_strcasecmp (type, "text/css") == 0;

    handler->parent = (RsvgSaxHandlerDefs *) ctx->priv->handler;
    ctx->priv->handler = &handler->super;
}

static void
add_node_to_handle (RsvgHandle *ctx, RsvgNode *node)
{
    g_assert (ctx != NULL);
    g_assert (node != NULL);

    g_ptr_array_add (ctx->priv->all_nodes, rsvg_node_ref (node));
}

static void
register_node_in_defs (RsvgHandle *ctx, RsvgNode *node, RsvgPropertyBag *atts)
{
    const char *id;

    id = rsvg_property_bag_lookup (atts, "id");
    if (id) {
        rsvg_defs_register_node_by_id (ctx->priv->defs, id, node);
    }
}

static void
push_element_name (RsvgHandle *ctx, const char *name)
{
    /* libxml holds on to the name while parsing; we won't dup the name here */
    ctx->priv->element_name_stack = g_slist_prepend (ctx->priv->element_name_stack, (void *) name);
}

static gboolean
topmost_element_name_is (RsvgHandle *ctx, const char *name)
{
    if (ctx->priv->element_name_stack) {
        const char *name_in_stack = ctx->priv->element_name_stack->data;

        return strcmp (name, name_in_stack) == 0;
    } else
        return FALSE;
}

static void
pop_element_name (RsvgHandle *ctx)
{
    ctx->priv->element_name_stack = g_slist_delete_link (ctx->priv->element_name_stack, ctx->priv->element_name_stack);
}

static void
free_element_name_stack (RsvgHandle *ctx)
{
    g_slist_free (ctx->priv->element_name_stack);
    ctx->priv->element_name_stack = NULL;
}

typedef RsvgNode *(* CreateNodeFn) (const char *element_name, RsvgNode *parent);

typedef struct {
    const char   *element_name;
    gboolean      supports_class_attribute; /* from https://www.w3.org/TR/SVG/attindex.html#RegularAttributes */
    CreateNodeFn  create_fn;
} NodeCreator;

/* Keep these sorted by element_name!
 *
 * Lines in comments are elements that we don't support.
 */
static const NodeCreator node_creators[] = {
    { "a",                   TRUE,  rsvg_new_group },    /* treat anchors as groups for now */
    /* "altGlyph",           TRUE,  */
    /* "altGlyphDef",        FALSE, */
    /* "altGlyphItem",       FALSE, */
    /* "animate",            FALSE, */
    /* "animateColor",       FALSE, */
    /* "animateMotion",      FALSE, */
    /* "animateTransform",   FALSE, */
    { "circle",              TRUE,  rsvg_new_circle },
    { "clipPath",            TRUE,  rsvg_new_clip_path },
    /* "color-profile",      FALSE, */
    { "conicalGradient",     TRUE,  rsvg_new_radial_gradient },
    /* "cursor",             FALSE, */
    { "defs",                TRUE,  rsvg_new_defs },
    /* "desc",               TRUE,  */
    { "ellipse",             TRUE,  rsvg_new_ellipse },
    { "feBlend",             TRUE,  rsvg_new_filter_primitive_blend },
    { "feColorMatrix",       TRUE,  rsvg_new_filter_primitive_color_matrix },
    { "feComponentTransfer", TRUE,  rsvg_new_filter_primitive_component_transfer },
    { "feComposite",         TRUE,  rsvg_new_filter_primitive_composite },
    { "feConvolveMatrix",    TRUE,  rsvg_new_filter_primitive_convolve_matrix },
    { "feDiffuseLighting",   TRUE,  rsvg_new_filter_primitive_diffuse_lighting },
    { "feDisplacementMap",   TRUE,  rsvg_new_filter_primitive_displacement_map },
    { "feDistantLight",      FALSE, rsvg_new_node_light_source },
    { "feFlood",             TRUE,  rsvg_new_filter_primitive_flood },
    { "feFuncA",             FALSE, rsvg_new_node_component_transfer_function },
    { "feFuncB",             FALSE, rsvg_new_node_component_transfer_function },
    { "feFuncG",             FALSE, rsvg_new_node_component_transfer_function },
    { "feFuncR",             FALSE, rsvg_new_node_component_transfer_function },
    { "feGaussianBlur",      TRUE,  rsvg_new_filter_primitive_gaussian_blur },
    { "feImage",             TRUE,  rsvg_new_filter_primitive_image },
    { "feMerge",             TRUE,  rsvg_new_filter_primitive_merge },
    { "feMergeNode",         FALSE, rsvg_new_filter_primitive_merge_node },
    { "feMorphology",        TRUE,  rsvg_new_filter_primitive_erode },
    { "feOffset",            TRUE,  rsvg_new_filter_primitive_offset },
    { "fePointLight",        FALSE, rsvg_new_node_light_source },
    { "feSpecularLighting",  TRUE,  rsvg_new_filter_primitive_specular_lighting },
    { "feSpotLight",         FALSE, rsvg_new_node_light_source },
    { "feTile",              TRUE,  rsvg_new_filter_primitive_tile },
    { "feTurbulence",        TRUE,  rsvg_new_filter_primitive_turbulence },
    { "filter",              TRUE,  rsvg_new_filter },
    /* "font",               TRUE,  */
    /* "font-face",          FALSE, */
    /* "font-face-format",   FALSE, */
    /* "font-face-name",     FALSE, */
    /* "font-face-src",      FALSE, */
    /* "font-face-uri",      FALSE, */
    /* "foreignObject",      TRUE,  */
    { "g",                   TRUE,  rsvg_new_group },
    /* "glyph",              TRUE,  */
    /* "glyphRef",           TRUE,  */
    /* "hkern",              FALSE, */
    { "image",               TRUE,  rsvg_new_image },
    { "line",                TRUE,  rsvg_new_line },
    { "linearGradient",      TRUE,  rsvg_new_linear_gradient },
    { "marker",              TRUE,  rsvg_new_marker },
    { "mask",                TRUE,  rsvg_new_mask },
    /* "metadata",           FALSE, */
    /* "missing-glyph",      TRUE,  */
    /* "mpath"               FALSE, */
    { "multiImage",          FALSE, rsvg_new_switch }, /* hack to make multiImage sort-of work */
    { "path",                TRUE,  rsvg_new_path },
    { "pattern",             TRUE,  rsvg_new_pattern },
    { "polygon",             TRUE,  rsvg_new_polygon },
    { "polyline",            TRUE,  rsvg_new_polyline },
    { "radialGradient",      TRUE,  rsvg_new_radial_gradient },
    { "rect",                TRUE,  rsvg_new_rect },
    /* "script",             FALSE, */
    /* "set",                FALSE, */
    { "stop",                TRUE,  rsvg_new_stop },
    /* "style",              FALSE, */
    { "subImage",            FALSE, rsvg_new_group },
    { "subImageRef",         FALSE, rsvg_new_image },
    { "svg",                 TRUE,  rsvg_new_svg },
    { "switch",              TRUE,  rsvg_new_switch },
    { "symbol",              TRUE,  rsvg_new_symbol },
    { "text",                TRUE,  rsvg_new_text },
    /* "textPath",           TRUE,  */
    /* "title",              TRUE,  */
    { "tref",                TRUE,  rsvg_new_tref },
    { "tspan",               TRUE,  rsvg_new_tspan },
    { "use",                 TRUE,  rsvg_new_use },
    /* "view",               FALSE, */
    /* "vkern",              FALSE, */
};

/* hack for bug 401115. whenever we encounter a node we don't understand, push it into a group.
 * this will allow us to handle things like conditionals properly.
 */
static const NodeCreator default_node_creator = { NULL, TRUE, rsvg_new_group };

/* Used from bsearch() */
static int
compare_node_creators_fn (const void *a, const void *b)
{
    const NodeCreator *na = a;
    const NodeCreator *nb = b;

    return strcmp (na->element_name, nb->element_name);
}

static const NodeCreator *
get_node_creator_for_element_name (const char *name)
{
    NodeCreator key;
    const NodeCreator *result;

    key.element_name = name;
    key.supports_class_attribute = FALSE;
    key.create_fn = NULL;

    result = bsearch (&key,
                      node_creators,
                      G_N_ELEMENTS (node_creators),
                      sizeof (NodeCreator),
                      compare_node_creators_fn);

    if (result == NULL)
        result = &default_node_creator;

    return result;
}

static void
node_set_atts (RsvgNode * node, RsvgHandle * ctx, const NodeCreator *creator, RsvgPropertyBag * atts)
{
    if (rsvg_property_bag_size (atts) > 0) {
        const char *id;
        const char *klazz;

        rsvg_node_set_atts (node, ctx, atts);

        /* The "svg" node is special; it will load its id/class
         * attributes until the end, when rsvg_end_element() calls
         * _rsvg_node_svg_apply_atts()
         */
        if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_SVG) {
            id = rsvg_property_bag_lookup (atts, "id");

            if (creator->supports_class_attribute)
                klazz = rsvg_property_bag_lookup (atts, "class");
            else
                klazz = NULL;

            rsvg_parse_style_attrs (ctx, rsvg_node_get_state (node), creator->element_name, klazz, id, atts);
        }
    }
}

static void
rsvg_standard_element_start (RsvgHandle * ctx, const char *name, RsvgPropertyBag * atts)
{
    const NodeCreator *creator;
    RsvgNode *newnode = NULL;

    creator = get_node_creator_for_element_name (name);
    g_assert (creator != NULL && creator->create_fn != NULL);

    newnode = creator->create_fn (name, ctx->priv->currentnode);

    if (newnode) {
        g_assert (rsvg_node_get_type (newnode) != RSVG_NODE_TYPE_INVALID);

        push_element_name (ctx, name);

        add_node_to_handle (ctx, newnode);
        register_node_in_defs (ctx, newnode, atts);

        if (ctx->priv->currentnode) {
            rsvg_node_add_child (ctx->priv->currentnode, newnode);
            ctx->priv->currentnode = rsvg_node_unref (ctx->priv->currentnode);
        } else if (rsvg_node_get_type (newnode) == RSVG_NODE_TYPE_SVG) {
            ctx->priv->treebase = rsvg_node_ref (newnode);
        }

        ctx->priv->currentnode = rsvg_node_ref (newnode);

        node_set_atts (newnode, ctx, creator, atts);

        newnode = rsvg_node_unref (newnode);
    }
}

/* extra (title, desc, metadata) */

static void
rsvg_extra_handler_free (RsvgSaxHandler * self)
{
    RsvgSaxHandlerExtra *z = (RsvgSaxHandlerExtra *) self;

    if (z->stringptr) {
        if (*z->stringptr)
            g_string_free (*z->stringptr, TRUE);
        *z->stringptr = z->string;
    } else if (z->string) {
        g_string_free (z->string, TRUE);
    }

    g_free (self);
}

static void
rsvg_extra_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerExtra *z = (RsvgSaxHandlerExtra *) self;

    /* This isn't quite the correct behavior - in theory, any graphics
       element may contain a title, desc, or metadata element */

    if (!z->string)
        return;

    if (!ch || !len)
        return;

    if (!g_utf8_validate ((char *) ch, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
        g_string_append (z->string, utf8);
        g_free (utf8);
    } else {
        g_string_append_len (z->string, (char *) ch, len);
    }
}

static void
rsvg_extra_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
}

static void
rsvg_extra_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerExtra *z = (RsvgSaxHandlerExtra *) self;
    RsvgHandle *ctx = z->ctx;

    if (!strcmp (name, z->name)) {
        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }
    }
}

static RsvgSaxHandlerExtra *
rsvg_start_extra (RsvgHandle * ctx,
                  const char *name,
                  GString **stringptr)
{
    RsvgSaxHandlerExtra *handler = g_new0 (RsvgSaxHandlerExtra, 1);
    RsvgNode *treebase = ctx->priv->treebase;
    RsvgNode *currentnode = ctx->priv->currentnode;
    gboolean do_care;

    /* only parse <extra> for the <svg> node.
     * This isn't quite the correct behavior - any graphics
     * element may contain a <extra> element.
     */
    do_care = treebase != NULL && rsvg_node_is_same (treebase, currentnode);

    handler->super.free = rsvg_extra_handler_free;
    handler->super.characters = rsvg_extra_handler_characters;
    handler->super.start_element = rsvg_extra_handler_start;
    handler->super.end_element = rsvg_extra_handler_end;
    handler->ctx = ctx;
    handler->name = name; /* interned */
    handler->string = do_care ? g_string_new (NULL) : NULL;
    handler->stringptr = do_care ? stringptr : NULL;

    ctx->priv->handler = &handler->super;

    return handler;
}

/* start desc */

static void
rsvg_start_desc (RsvgHandle * ctx)
{
    rsvg_start_extra (ctx, "desc", &ctx->priv->desc);
}

/* end desc */

/* start title */

static void
rsvg_start_title (RsvgHandle * ctx)
{
    rsvg_start_extra (ctx, "title", &ctx->priv->title);
}

/* end title */

/* start metadata */

static void
rsvg_metadata_props_enumerate (const char *key, const char *value, gpointer user_data)
{
    GString *metadata = (GString *) user_data;
    g_string_append_printf (metadata, "%s=\"%s\" ", key, value);
}

static void
rsvg_metadata_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *) self;

    rsvg_extra_handler_start (self, name, atts);

    if (!z->string)
        return;

    g_string_append_printf (z->string, "<%s ", name);
    rsvg_property_bag_enumerate (atts, rsvg_metadata_props_enumerate, z->string);
    g_string_append (z->string, ">\n");
}

static void
rsvg_metadata_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *) self;

    if (strcmp (name, z->name) != 0) {
        if (z->string)
            g_string_append_printf (z->string, "</%s>\n", name);
    } else {
        rsvg_extra_handler_end (self, name);
    }
}

static void
rsvg_start_metadata (RsvgHandle * ctx)
{
    RsvgSaxHandlerMetadata *handler = rsvg_start_extra (ctx, "metadata", &ctx->priv->metadata);

    handler->super.start_element = rsvg_metadata_handler_start;
    handler->super.end_element = rsvg_metadata_handler_end;
}

/* end metadata */

/* start xinclude */

typedef struct _RsvgSaxHandlerXinclude {
    RsvgSaxHandler super;

    RsvgSaxHandler *prev_handler;
    RsvgHandle *ctx;
    gboolean success;
    gboolean in_fallback;
} RsvgSaxHandlerXinclude;

static void
 rsvg_start_xinclude (RsvgHandle * ctx, RsvgPropertyBag * atts);
static void
 rsvg_characters_impl (RsvgHandle * ctx, const xmlChar * ch, int len);

static void
rsvg_xinclude_handler_free (RsvgSaxHandler * self)
{
    g_free (self);
}

static void
rsvg_xinclude_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;

    if (z->in_fallback) {
        rsvg_characters_impl (z->ctx, (const xmlChar *) ch, len);
    }
}

static void
rsvg_xinclude_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;

    if (!z->success) {
        if (z->in_fallback) {
            if (!strcmp (name, "xi:include"))
                rsvg_start_xinclude (z->ctx, atts);
            else
                rsvg_standard_element_start (z->ctx, (const char *) name, atts);
        } else if (!strcmp (name, "xi:fallback")) {
            z->in_fallback = TRUE;
        }
    }
}

static void
rsvg_xinclude_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;
    RsvgHandle *ctx = z->ctx;

    if (!strcmp (name, "include") || !strcmp (name, "xi:include")) {
        if (ctx->priv->handler != NULL) {
            RsvgSaxHandler *previous_handler;

            previous_handler = z->prev_handler;
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = previous_handler;
        }
    } else if (z->in_fallback) {
        if (!strcmp (name, "xi:fallback"))
            z->in_fallback = FALSE;
    }
}

static void
_rsvg_set_xml_parse_options(xmlParserCtxtPtr xml_parser,
                            RsvgHandle *ctx)
{
    xml_parser->options |= XML_PARSE_NONET;

    if (ctx->priv->flags & RSVG_HANDLE_FLAG_UNLIMITED) {
#if LIBXML_VERSION > 20632
        xml_parser->options |= XML_PARSE_HUGE;
#endif
    }

#if LIBXML_VERSION > 20800
    xml_parser->options |= XML_PARSE_BIG_LINES;
#endif
}

/* http://www.w3.org/TR/xinclude/ */
static void
rsvg_start_xinclude (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerXinclude *handler;
    const char *href, *parse;
    gboolean success = FALSE;

    href = rsvg_property_bag_lookup (atts, "href");
    if (href == NULL)
        goto fallback;

    parse = rsvg_property_bag_lookup (atts, "parse");
    if (parse && !strcmp (parse, "text")) {
        char *data;
        gsize data_len;
        const char *encoding;

        data = _rsvg_handle_acquire_data (ctx, href, NULL, &data_len, NULL);
        if (data == NULL)
            goto fallback;

        encoding = rsvg_property_bag_lookup (atts, "encoding");
        if (encoding && g_ascii_strcasecmp (encoding, "UTF-8") != 0) {
            char *text_data;
            gsize text_data_len;

            text_data = g_convert (data, data_len, "utf-8", encoding, NULL,
                                   &text_data_len, NULL);
            g_free (data);

            data = text_data;
            data_len = text_data_len;
        }

        rsvg_characters_impl (ctx, (xmlChar *) data, data_len);

        g_free (data);
    } else {
        /* xml */
        GInputStream *stream;
        GError *err = NULL;
        xmlDocPtr xml_doc;
        xmlParserCtxtPtr xml_parser;
        xmlParserInputBufferPtr buffer;
        xmlParserInputPtr input;

        stream = _rsvg_handle_acquire_stream (ctx, href, NULL, NULL);
        if (stream == NULL)
            goto fallback;

        xml_parser = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, ctx, NULL, 0, NULL);
        _rsvg_set_xml_parse_options(xml_parser, ctx);

        buffer = _rsvg_xml_input_buffer_new_from_stream (stream, NULL /* cancellable */, XML_CHAR_ENCODING_NONE, &err);
        g_object_unref (stream);

        input = xmlNewIOInputStream (xml_parser, buffer /* adopts */, XML_CHAR_ENCODING_NONE);

        if (xmlPushInput (xml_parser, input) < 0) {
            g_clear_error (&err);
            xmlFreeInputStream (input);
            xmlFreeParserCtxt (xml_parser);
            goto fallback;
        }

        (void) xmlParseDocument (xml_parser);

        xml_doc = xml_parser->myDoc;
        xmlFreeParserCtxt (xml_parser);
        if (xml_doc)
            xmlFreeDoc (xml_doc);

        g_clear_error (&err);
    }

    success = TRUE;

  fallback:

    /* needed to handle xi:fallback */
    handler = g_new0 (RsvgSaxHandlerXinclude, 1);

    handler->super.free = rsvg_xinclude_handler_free;
    handler->super.characters = rsvg_xinclude_handler_characters;
    handler->super.start_element = rsvg_xinclude_handler_start;
    handler->super.end_element = rsvg_xinclude_handler_end;
    handler->prev_handler = ctx->priv->handler;
    handler->ctx = ctx;
    handler->success = success;

    ctx->priv->handler = &handler->super;
}

/* end xinclude */

static void
rsvg_start_element (void *data, const xmlChar * name, const xmlChar ** atts)
{
    RsvgPropertyBag *bag;
    RsvgHandle *ctx = (RsvgHandle *) data;

    bag = rsvg_property_bag_new ((const char **) atts);

    if (ctx->priv->handler) {
        ctx->priv->handler_nest++;
        if (ctx->priv->handler->start_element != NULL)
            ctx->priv->handler->start_element (ctx->priv->handler, (const char *) name, bag);
    } else {
        const char *tempname;
        for (tempname = (const char *) name; *tempname != '\0'; tempname++)
            if (*tempname == ':')
                name = (const xmlChar *) (tempname + 1);

        if (!strcmp ((const char *) name, "style"))
            rsvg_start_style (ctx, bag);
        else if (!strcmp ((const char *) name, "title"))
            rsvg_start_title (ctx);
        else if (!strcmp ((const char *) name, "desc"))
            rsvg_start_desc (ctx);
        else if (!strcmp ((const char *) name, "metadata"))
            rsvg_start_metadata (ctx);
        else if (!strcmp ((const char *) name, "include"))      /* xi:include */
            rsvg_start_xinclude (ctx, bag);
        else
            rsvg_standard_element_start (ctx, (const char *) name, bag);
    }

    rsvg_property_bag_free (bag);
}

static void
rsvg_end_element (void *data, const xmlChar * xmlname)
{
    RsvgHandle *ctx = (RsvgHandle *) data;
    const char *name = (const char *) xmlname;

    if (ctx->priv->handler_nest > 0 && ctx->priv->handler != NULL) {
        if (ctx->priv->handler->end_element != NULL)
            ctx->priv->handler->end_element (ctx->priv->handler, name);
        ctx->priv->handler_nest--;
    } else {
        const char *tempname;
        for (tempname = name; *tempname != '\0'; tempname++)
            if (*tempname == ':')
                name = tempname + 1;

        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }

        if (ctx->priv->currentnode && topmost_element_name_is (ctx, name)) {
            RsvgNode *parent;

            parent = rsvg_node_get_parent (ctx->priv->currentnode);
            ctx->priv->currentnode = rsvg_node_unref (ctx->priv->currentnode);
            ctx->priv->currentnode = parent;
            pop_element_name (ctx);
        }

        /* FIXMEchpe: shouldn't this check that currentnode == treebase or sth like that? */
        if (ctx->priv->treebase && !strcmp (name, "svg")) {
            RsvgNodeSvg *svg;

            g_assert (rsvg_node_get_type (ctx->priv->treebase) == RSVG_NODE_TYPE_SVG);
            svg = rsvg_rust_cnode_get_impl (ctx->priv->treebase);

            _rsvg_node_svg_apply_atts (ctx->priv->treebase, svg, ctx);
        }
    }
}

static void
rsvg_node_chars_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag * atts)
{
    /* nothing */
}

static void
rsvg_node_chars_draw (RsvgNode *node, gpointer impl, RsvgDrawingCtx *ctx, int dominate)
{
    /* nothing */
}

static void
rsvg_node_chars_free (gpointer impl)
{
    RsvgNodeChars *self = impl;
    g_string_free (self->contents, TRUE);
    g_free (self);
}

static RsvgNode *
rsvg_new_node_chars (const char *text,
                     int len,
                     RsvgNode *parent)
{
    RsvgNodeChars *self;
    RsvgState *state;

    self = g_new0 (RsvgNodeChars, 1);

    if (!g_utf8_validate (text, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 (text, len);
        self->contents = g_string_new (utf8);
        g_free (utf8);
    } else {
        self->contents = g_string_new_len (text, len);
    }

    state = rsvg_state_new ();
    state->cond_true = FALSE;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_CHARS,
                                parent,
                                state,
                                self,
                                rsvg_node_chars_set_atts,
                                rsvg_node_chars_draw,
                                rsvg_node_chars_free);
}

static gboolean
find_last_chars_node (RsvgNode *node, gpointer data)
{
    RsvgNode **dest;

    dest = data;

    if (rsvg_node_get_type (node) == RSVG_NODE_TYPE_CHARS) {
        *dest = node;
    } else if (rsvg_node_get_type (node) == RSVG_NODE_TYPE_TSPAN) {
        *dest = NULL;
    }

    return TRUE;
}

static void
rsvg_characters_impl (RsvgHandle * ctx, const xmlChar * ch, int len)
{
    RsvgNode *node;

    if (!ch || !len)
        return;

    if (ctx->priv->currentnode) {
        RsvgNodeType type = rsvg_node_get_type (ctx->priv->currentnode);
        if (type == RSVG_NODE_TYPE_TSPAN || type == RSVG_NODE_TYPE_TEXT) {
            RsvgNodeChars *self;

            /* find the last CHARS node in the text or tspan node, so that we
               can coalesce the text, and thus avoid screwing up the Pango layouts */
            node = NULL;
            rsvg_node_foreach_child (ctx->priv->currentnode,
                                     find_last_chars_node,
                                     &node);

            if (node) {
                g_assert (rsvg_node_get_type (node) == RSVG_NODE_TYPE_CHARS);
                self = rsvg_rust_cnode_get_impl (node);

                if (!g_utf8_validate ((char *) ch, len, NULL)) {
                    char *utf8;
                    utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
                    g_string_append (self->contents, utf8);
                    g_free (utf8);
                } else {
                    g_string_append_len (self->contents, (char *)ch, len);
                }

                return;
            }
        }
    }

    node = rsvg_new_node_chars ((char *) ch, len, ctx->priv->currentnode);

    add_node_to_handle (ctx, node);

    if (ctx->priv->currentnode)
        rsvg_node_add_child (ctx->priv->currentnode, node);
}

static void
rsvg_characters (void *data, const xmlChar * ch, int len)
{
    RsvgHandle *ctx = (RsvgHandle *) data;

    if (ctx->priv->handler && ctx->priv->handler->characters != NULL) {
        ctx->priv->handler->characters (ctx->priv->handler, (const char *) ch, len);
        return;
    }

    rsvg_characters_impl (ctx, ch, len);
}

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar * name)
{
    RsvgHandle *ctx = (RsvgHandle *) data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (ctx->priv->entities, name);

    return entity;
}

static void
rsvg_entity_decl (void *data, const xmlChar * name, int type,
                  const xmlChar * publicId, const xmlChar * systemId, xmlChar * content)
{
    RsvgHandle *ctx = (RsvgHandle *) data;
    GHashTable *entities = ctx->priv->entities;
    xmlEntityPtr entity;
    xmlChar *resolvedSystemId = NULL, *resolvedPublicId = NULL;

    if (systemId)
        resolvedSystemId = xmlBuildRelativeURI (systemId, (xmlChar*) rsvg_handle_get_base_uri (ctx));
    else if (publicId)
        resolvedPublicId = xmlBuildRelativeURI (publicId, (xmlChar*) rsvg_handle_get_base_uri (ctx));

    if (type == XML_EXTERNAL_PARAMETER_ENTITY && !content) {
        char *entity_data;
        gsize entity_data_len;

        if (systemId)
            entity_data = _rsvg_handle_acquire_data (ctx,
                                                     (const char *) systemId,
                                                     NULL,
                                                     &entity_data_len,
                                                     NULL);
        else if (publicId)
            entity_data = _rsvg_handle_acquire_data (ctx,
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

    g_hash_table_insert (entities, g_strdup ((const char*) name), entity);
}

static void
rsvg_unparsed_entity_decl (void *ctx,
                           const xmlChar * name,
                           const xmlChar * publicId,
                           const xmlChar * systemId, const xmlChar * notationName)
{
    rsvg_entity_decl (ctx, name, XML_INTERNAL_GENERAL_ENTITY, publicId, systemId, NULL);
}

static xmlEntityPtr
rsvg_get_parameter_entity (void *data, const xmlChar * name)
{
    RsvgHandle *ctx = (RsvgHandle *) data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (ctx->priv->entities, name);

    return entity;
}

static void
rsvg_error_cb (void *data, const char *msg, ...)
{
#ifdef G_ENABLE_DEBUG
    va_list args;

    va_start (args, msg);
    vfprintf (stderr, msg, args);
    va_end (args);
#endif
}

static void
rsvg_processing_instruction (void *ctx, const xmlChar * target, const xmlChar * data)
{
    /* http://www.w3.org/TR/xml-stylesheet/ */
    RsvgHandle *handle = (RsvgHandle *) ctx;

    if (!strcmp ((const char *) target, "xml-stylesheet")) {
        RsvgPropertyBag *atts;
        char **xml_atts;

        xml_atts = rsvg_css_parse_xml_attribute_string ((const char *) data);

        if (xml_atts) {
            const char *value;

            atts = rsvg_property_bag_new ((const char **) xml_atts);
            value = rsvg_property_bag_lookup (atts, "alternate");
            if (!value || !value[0] || (strcmp (value, "no") != 0)) {
                value = rsvg_property_bag_lookup (atts, "type");
                if (value && strcmp (value, "text/css") == 0) {
                    value = rsvg_property_bag_lookup (atts, "href");
                    if (value && value[0]) {
                        char *style_data;
                        gsize style_data_len;
                        char *mime_type = NULL;

                        style_data = _rsvg_handle_acquire_data (handle,
                                                                value,
                                                                &mime_type,
                                                                &style_data_len,
                                                                NULL);
                        if (style_data &&
                            mime_type &&
                            strcmp (mime_type, "text/css") == 0) {
                            rsvg_parse_cssbuffer (handle, style_data, style_data_len);
                        }

                        g_free (mime_type);
                        g_free (style_data);
                    }
                }
            }

            rsvg_property_bag_free (atts);
            g_strfreev (xml_atts);
        }
    }
}

void
rsvg_SAX_handler_struct_init (void)
{
    if (!rsvgSAXHandlerStructInited) {
        rsvgSAXHandlerStructInited = TRUE;

        memset (&rsvgSAXHandlerStruct, 0, sizeof (rsvgSAXHandlerStruct));

        rsvgSAXHandlerStruct.getEntity = rsvg_get_entity;
        rsvgSAXHandlerStruct.entityDecl = rsvg_entity_decl;
        rsvgSAXHandlerStruct.unparsedEntityDecl = rsvg_unparsed_entity_decl;
        rsvgSAXHandlerStruct.getParameterEntity = rsvg_get_parameter_entity;
        rsvgSAXHandlerStruct.characters = rsvg_characters;
        rsvgSAXHandlerStruct.error = rsvg_error_cb;
        rsvgSAXHandlerStruct.cdataBlock = rsvg_characters;
        rsvgSAXHandlerStruct.startElement = rsvg_start_element;
        rsvgSAXHandlerStruct.endElement = rsvg_end_element;
        rsvgSAXHandlerStruct.processingInstruction = rsvg_processing_instruction;
    }
}

/* http://www.ietf.org/rfc/rfc2396.txt */

static gboolean
rsvg_path_is_uri (char const *path)
{
    char const *p;

    if (path == NULL)
        return FALSE;

    if (strlen (path) < 4)
        return FALSE;

    if ((path[0] < 'a' || path[0] > 'z') &&
        (path[0] < 'A' || path[0] > 'Z')) {
        return FALSE;
    }

    for (p = &path[1];
	    (*p >= 'a' && *p <= 'z') ||
        (*p >= 'A' && *p <= 'Z') ||
        (*p >= '0' && *p <= '9') ||
         *p == '+' ||
         *p == '-' ||
         *p == '.';
        p++);

    if (strlen (p) < 3)
        return FALSE;

    return (p[0] == ':' && p[1] == '/' && p[2] == '/');
}

gchar *
rsvg_get_base_uri_from_filename (const gchar * filename)
{
    gchar *current_dir;
    gchar *absolute_filename;
    gchar *base_uri;


    if (g_path_is_absolute (filename))
        return g_filename_to_uri (filename, NULL, NULL);

    current_dir = g_get_current_dir ();
    absolute_filename = g_build_filename (current_dir, filename, NULL);
    base_uri = g_filename_to_uri (absolute_filename, NULL, NULL);
    g_free (absolute_filename);
    g_free (current_dir);

    return base_uri;
}

/**
 * rsvg_handle_set_base_uri:
 * @handle: A #RsvgHandle
 * @base_uri: The base uri
 *
 * Set the base URI for this SVG. This can only be called before rsvg_handle_write()
 * has been called.
 *
 * Since: 2.9
 */
void
rsvg_handle_set_base_uri (RsvgHandle * handle, const char *base_uri)
{
    gchar *uri;
    GFile *file;

    g_return_if_fail (handle != NULL);

    if (base_uri == NULL)
	return;

    if (rsvg_path_is_uri (base_uri))
        uri = g_strdup (base_uri);
    else
        uri = rsvg_get_base_uri_from_filename (base_uri);

    file = g_file_new_for_uri (uri ? uri : "data:");
    rsvg_handle_set_base_gfile (handle, file);
    g_object_unref (file);
    g_free (uri);
}

/**
 * rsvg_handle_set_base_gfile:
 * @handle: a #RsvgHandle
 * @base_file: a #GFile
 *
 * Set the base URI for @handle from @file.
 * Note: This function may only be called before rsvg_handle_write()
 * or rsvg_handle_read_stream_sync() has been called.
 *
 * Since: 2.32
 */
void
rsvg_handle_set_base_gfile (RsvgHandle *handle,
                            GFile      *base_file)
{
    RsvgHandlePrivate *priv;

    g_return_if_fail (RSVG_IS_HANDLE (handle));
    g_return_if_fail (G_IS_FILE (base_file));

    priv = handle->priv;

    g_object_ref (base_file);
    if (priv->base_gfile)
        g_object_unref (priv->base_gfile);
    priv->base_gfile = base_file;

    g_free (priv->base_uri);
    priv->base_uri = g_file_get_uri (base_file);
}

/**
 * rsvg_handle_get_base_uri:
 * @handle: A #RsvgHandle
 *
 * Gets the base uri for this #RsvgHandle.
 *
 * Returns: the base uri, possibly null
 * Since: 2.8
 */
const char *
rsvg_handle_get_base_uri (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);
    return handle->priv->base_uri;
}

/**
 * rsvg_error_quark:
 *
 * The error domain for RSVG
 *
 * Returns: The error domain
 */
GQuark
rsvg_error_quark (void)
{
    /* don't use from_static_string(), since librsvg might be used in a module
       that's ultimately unloaded */
    return g_quark_from_string ("rsvg-error-quark");
}

static void
rsvg_set_error (GError **error, xmlParserCtxtPtr ctxt)
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

static void
create_xml_push_parser_ctxt (RsvgHandle *handle)
{
    if (handle->priv->ctxt == NULL) {
        handle->priv->ctxt = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0,
                                                      rsvg_handle_get_base_uri (handle));
        _rsvg_set_xml_parse_options(handle->priv->ctxt, handle);

        /* if false, external entities work, but internal ones don't. if true, internal entities
           work, but external ones don't. favor internal entities, in order to not cause a
           regression */
        handle->priv->ctxt->replaceEntities = TRUE;
    }
}

static gboolean
rsvg_handle_write_impl (RsvgHandle * handle, const guchar * buf, gsize count, GError ** error)
{
    GError *real_error = NULL;
    int result;

    rsvg_return_val_if_fail (handle != NULL, FALSE, error);

    handle->priv->error = &real_error;
    create_xml_push_parser_ctxt (handle);

    result = xmlParseChunk (handle->priv->ctxt, (char *) buf, count, 0);
    if (result != 0) {
        rsvg_set_error (error, handle->priv->ctxt);
        return FALSE;
    }

    handle->priv->error = NULL;

    if (real_error != NULL) {
        g_propagate_error (error, real_error);
        return FALSE;
    }

    return TRUE;
}

static gboolean
rsvg_handle_close_impl (RsvgHandle * handle, GError ** error)
{
    GError *real_error = NULL;

	handle->priv->is_closed = TRUE;

    handle->priv->error = &real_error;

    if (handle->priv->ctxt != NULL) {
        xmlDocPtr xml_doc;
        int result;

        xml_doc = handle->priv->ctxt->myDoc;

        result = xmlParseChunk (handle->priv->ctxt, "", 0, TRUE);
        if (result != 0) {
            rsvg_set_error (error, handle->priv->ctxt);
            xmlFreeParserCtxt (handle->priv->ctxt);
            xmlFreeDoc (xml_doc);
            return FALSE;
        }

        xmlFreeParserCtxt (handle->priv->ctxt);
        xmlFreeDoc (xml_doc);
    }

    free_element_name_stack (handle);

    handle->priv->finished = TRUE;
    handle->priv->error = NULL;

    if (real_error != NULL) {
        g_propagate_error (error, real_error);
        return FALSE;
    }

    return TRUE;
}

void
rsvg_drawing_ctx_free (RsvgDrawingCtx * handle)
{
    rsvg_render_free (handle->render);

    rsvg_state_free_all (handle->state);

	g_slist_free_full (handle->drawsub_stack, (GDestroyNotify) rsvg_node_unref);

    g_warn_if_fail (handle->acquired_nodes == NULL);
    g_slist_free (handle->acquired_nodes);

    if (handle->pango_context != NULL)
        g_object_unref (handle->pango_context);

    g_free (handle);
}

/**
 * rsvg_handle_get_metadata:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's metadata in UTF-8 or %NULL. You must make a copy
 * of this metadata if you wish to use it after @handle has been freed.
 *
 * Returns: (nullable): The SVG's title
 *
 * Since: 2.9
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_metadata (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->metadata)
        return handle->priv->metadata->str;
    else
        return NULL;
}

/**
 * rsvg_handle_get_title:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's title in UTF-8 or %NULL. You must make a copy
 * of this title if you wish to use it after @handle has been freed.
 *
 * Returns: (nullable): The SVG's title
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_title (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->title)
        return handle->priv->title->str;
    else
        return NULL;
}

/**
 * rsvg_handle_get_desc:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's description in UTF-8 or %NULL. You must make a copy
 * of this description if you wish to use it after @handle has been freed.
 *
 * Returns: (nullable): The SVG's description
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_desc (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->desc)
        return handle->priv->desc->str;
    else
        return NULL;
}

/**
 * rsvg_handle_get_dimensions:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 *
 * Get the SVG's size. Do not call from within the size_func callback, because an infinite loop will occur.
 *
 * Since: 2.14
 */
void
rsvg_handle_get_dimensions (RsvgHandle * handle, RsvgDimensionData * dimension_data)
{
    /* This function is probably called from the cairo_render functions.
     * To prevent an infinite loop we are saving the state.
     */
    if (!handle->priv->in_loop) {
        handle->priv->in_loop = TRUE;
        rsvg_handle_get_dimensions_sub (handle, dimension_data, NULL);
        handle->priv->in_loop = FALSE;
    } else {
        /* Called within the size function, so return a standard size */
        dimension_data->em = dimension_data->width = 1;
        dimension_data->ex = dimension_data->height = 1;
    }
}

/**
 * rsvg_handle_get_dimensions_sub:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 * @id: (nullable): An element's id within the SVG, or %NULL to get
 *   the dimension of the whole SVG.  For example, if you have a layer
 *   called "layer1" for that you want to get the dimension, pass
 *   "#layer1" as the id.
 *
 * Get the size of a subelement of the SVG file. Do not call from within the size_func callback, because an infinite loop will occur.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_dimensions_sub (RsvgHandle * handle, RsvgDimensionData * dimension_data, const char *id)
{
    cairo_t *cr;
    cairo_surface_t *target;
    RsvgDrawingCtx *draw;
    RsvgNodeSvg *root = NULL;
    RsvgNode *sself = NULL;
    RsvgBbox bbox;

    gboolean handle_subelement = TRUE;

    g_return_val_if_fail (handle, FALSE);
    g_return_val_if_fail (dimension_data, FALSE);

    memset (dimension_data, 0, sizeof (RsvgDimensionData));

    if (id && *id) {
        sself = rsvg_defs_lookup (handle->priv->defs, id);

        if (rsvg_node_is_same (sself, handle->priv->treebase))
            id = NULL;
    } else {
        sself = handle->priv->treebase;
    }

    if (!sself && id)
        return FALSE;

    if (!handle->priv->treebase)
        return FALSE;

    g_assert (rsvg_node_get_type (handle->priv->treebase) == RSVG_NODE_TYPE_SVG);
    root = rsvg_rust_cnode_get_impl (handle->priv->treebase);

    bbox.rect.x = bbox.rect.y = 0;
    bbox.rect.width = bbox.rect.height = 1;

    if (!id) {
        if ((root->w.unit == LENGTH_UNIT_PERCENT || root->h.unit == LENGTH_UNIT_PERCENT) && !root->vbox.active)
            handle_subelement = TRUE;
        else
            handle_subelement = FALSE;
    }

    if (handle_subelement == TRUE) {
        target = cairo_image_surface_create (CAIRO_FORMAT_RGB24,
                                             1, 1);
        cr = cairo_create  (target);

        draw = rsvg_cairo_new_drawing_ctx (cr, handle);

        if (!draw) {
            cairo_destroy (cr);
            cairo_surface_destroy (target);

            return FALSE;
        }

        g_assert (sself != NULL);
        sself = rsvg_node_ref (sself);

        while (sself != NULL) {
            draw->drawsub_stack = g_slist_prepend (draw->drawsub_stack, sself);
            sself = rsvg_node_get_parent (sself);
        }

        rsvg_node_draw_from_stack (handle->priv->treebase, draw, 0);
        bbox = RSVG_CAIRO_RENDER (draw->render)->bbox;

        rsvg_drawing_ctx_free (draw);
        cairo_destroy (cr);
        cairo_surface_destroy (target);

        dimension_data->width = bbox.rect.width;
        dimension_data->height = bbox.rect.height;
    } else {
        bbox.rect.width = root->vbox.rect.width;
        bbox.rect.height = root->vbox.rect.height;

        dimension_data->width = (int) (rsvg_length_hand_normalize (&root->w, handle->priv->dpi_x,
                                                                   bbox.rect.width, 12) + 0.5);
        dimension_data->height = (int) (rsvg_length_hand_normalize (&root->h, handle->priv->dpi_y,
                                                                    bbox.rect.height, 12) + 0.5);
    }

    dimension_data->em = dimension_data->width;
    dimension_data->ex = dimension_data->height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data->width, &dimension_data->height,
                                    handle->priv->user_data);

    return TRUE;
}

/**
 * rsvg_handle_get_position_sub:
 * @handle: A #RsvgHandle
 * @position_data: (out): A place to store the SVG fragment's position.
 * @id: An element's id within the SVG.
 * For example, if you have a layer called "layer1" for that you want to get
 * the position, pass "##layer1" as the id.
 *
 * Get the position of a subelement of the SVG file. Do not call from within
 * the size_func callback, because an infinite loop will occur.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_position_sub (RsvgHandle * handle, RsvgPositionData * position_data, const char *id)
{
    RsvgDrawingCtx		*draw;
    RsvgNode			*node;
    RsvgBbox			 bbox;
    RsvgDimensionData    dimension_data;
    cairo_surface_t		*target = NULL;
    cairo_t				*cr = NULL;
    gboolean			 ret = FALSE;

    g_return_val_if_fail (handle, FALSE);
    g_return_val_if_fail (position_data, FALSE);

    /* Short-cut when no id is given. */
    if (NULL == id || '\0' == *id) {
        position_data->x = 0;
        position_data->y = 0;
        return TRUE;
    }

    memset (position_data, 0, sizeof (*position_data));
    memset (&dimension_data, 0, sizeof (dimension_data));

    node = rsvg_defs_lookup (handle->priv->defs, id);
    if (!node) {
        return FALSE;
    } else if (rsvg_node_is_same (node, handle->priv->treebase)) {
        /* Root node. */
        position_data->x = 0;
        position_data->y = 0;
        return TRUE;
    }

    if (!handle->priv->treebase)
        return FALSE;

    target = cairo_image_surface_create (CAIRO_FORMAT_RGB24, 1, 1);
    cr = cairo_create  (target);
    draw = rsvg_cairo_new_drawing_ctx (cr, handle);
    if (!draw)
        goto bail;

    g_assert (node != NULL);

    node = rsvg_node_ref (node);

    while (node != NULL) {
        draw->drawsub_stack = g_slist_prepend (draw->drawsub_stack, node);
        node = rsvg_node_get_parent (node);
    }

    rsvg_node_draw_from_stack (handle->priv->treebase, draw, 0);
    bbox = RSVG_CAIRO_RENDER (draw->render)->bbox;

    rsvg_drawing_ctx_free (draw);

    position_data->x = bbox.rect.x;
    position_data->y = bbox.rect.y;
    dimension_data.width = bbox.rect.width;
    dimension_data.height = bbox.rect.height;

    dimension_data.em = dimension_data.width;
    dimension_data.ex = dimension_data.height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data.width, &dimension_data.height,
                                    handle->priv->user_data);

    ret = TRUE;

bail:
    if (cr)
        cairo_destroy (cr);
    if (target)
        cairo_surface_destroy (target);

    return ret;
}

/**
 * rsvg_handle_has_sub:
 * @handle: a #RsvgHandle
 * @id: an element's id within the SVG
 *
 * Checks whether the element @id exists in the SVG document.
 *
 * Returns: %TRUE if @id exists in the SVG document
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_has_sub (RsvgHandle * handle,
                     const char *id)
{
    g_return_val_if_fail (handle, FALSE);

    if (G_UNLIKELY (!id || !id[0]))
      return FALSE;

    return rsvg_defs_lookup (handle->priv->defs, id) != NULL;
}

/**
 * rsvg_set_default_dpi:
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the all future outgoing pixbufs. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to @dpi will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_set_default_dpi (double dpi)
{
    rsvg_set_default_dpi_x_y (dpi, dpi);
}

/**
 * rsvg_set_default_dpi_x_y:
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the all future outgoing pixbufs. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to @dpi will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_set_default_dpi_x_y (double dpi_x, double dpi_y)
{
    if (dpi_x <= 0.)
        rsvg_internal_dpi_x = RSVG_DEFAULT_DPI_X;
    else
        rsvg_internal_dpi_x = dpi_x;

    if (dpi_y <= 0.)
        rsvg_internal_dpi_y = RSVG_DEFAULT_DPI_Y;
    else
        rsvg_internal_dpi_y = dpi_y;
}

/**
 * rsvg_handle_set_dpi:
 * @handle: An #RsvgHandle
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to @dpi will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi (RsvgHandle * handle, double dpi)
{
    rsvg_handle_set_dpi_x_y (handle, dpi, dpi);
}

/**
 * rsvg_handle_set_dpi_x_y:
 * @handle: An #RsvgHandle
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi_x or @dpi_y will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi_x_y (RsvgHandle * handle, double dpi_x, double dpi_y)
{
    g_return_if_fail (handle != NULL);

    if (dpi_x <= 0.)
        handle->priv->dpi_x = rsvg_internal_dpi_x;
    else
        handle->priv->dpi_x = dpi_x;

    if (dpi_y <= 0.)
        handle->priv->dpi_y = rsvg_internal_dpi_y;
    else
        handle->priv->dpi_y = dpi_y;
}

/**
 * rsvg_handle_set_size_callback:
 * @handle: An #RsvgHandle
 * @size_func: (nullable): A sizing function, or %NULL
 * @user_data: User data to pass to @size_func, or %NULL
 * @user_data_destroy: Destroy function for @user_data, or %NULL
 *
 * Sets the sizing function for the @handle.  This function is called right
 * after the size of the image has been loaded.  The size of the image is passed
 * in to the function, which may then modify these values to set the real size
 * of the generated pixbuf.  If the image has no associated size, then the size
 * arguments are set to -1.
 *
 * Deprecated: Set up a cairo matrix and use rsvg_handle_render_cairo() instead.
 * You can call rsvg_handle_get_dimensions() to figure out the size of your SVG,
 * and then scale it to the desired size via Cairo.  For example, the following
 * code renders an SVG at a specified size, scaled proportionally from whatever
 * original size it may have had:
 *
 * |[<!-- language="C" -->
 * void
 * render_scaled_proportionally (RsvgHandle *handle, cairo_t cr, int width, int height)
 * {
 *     RsvgDimensionData dimensions;
 *     double x_factor, y_factor;
 *     double scale_factor;
 *
 *     rsvg_handle_get_dimensions (handle, &dimensions);
 *
 *     x_factor = (double) width / dimensions.width;
 *     y_factor = (double) height / dimensions.height;
 *
 *     scale_factor = MIN (x_factor, y_factor);
 *
 *     cairo_scale (cr, scale_factor, scale_factor);
 *
 *     rsvg_handle_render_cairo (handle, cr);
 * }
 * ]|
 **/
void
rsvg_handle_set_size_callback (RsvgHandle * handle,
                               RsvgSizeFunc size_func,
                               gpointer user_data, GDestroyNotify user_data_destroy)
{
    g_return_if_fail (handle != NULL);

    if (handle->priv->user_data_destroy)
        (*handle->priv->user_data_destroy) (handle->priv->user_data);

    handle->priv->size_func = size_func;
    handle->priv->user_data = user_data;
    handle->priv->user_data_destroy = user_data_destroy;
}

/**
 * rsvg_handle_write:
 * @handle: an #RsvgHandle
 * @buf: (array length=count) (element-type guchar): pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Loads the next @count bytes of the image.  This will return %TRUE if the data
 * was loaded successful, and %FALSE if an error occurred.  In the latter case,
 * the loader will be closed, and will not accept further writes. If %FALSE is
 * returned, @error will be set to an error from the #RsvgError domain. Errors
 * from #GIOErrorEnum are also possible.
 *
 * Returns: %TRUE on success, or %FALSE on error
 **/
gboolean
rsvg_handle_write (RsvgHandle * handle, const guchar * buf, gsize count, GError ** error)
{
    RsvgHandlePrivate *priv;

    rsvg_return_val_if_fail (handle, FALSE, error);
    priv = handle->priv;

    rsvg_return_val_if_fail (!priv->is_closed, FALSE, error);

    if (priv->first_write) {
        priv->first_write = FALSE;

        /* test for GZ marker. todo: store the first 2 bytes in the odd circumstance that someone calls
         * write() in 1 byte increments */
        if ((count >= 2) && (buf[0] == (guchar) 0x1f) && (buf[1] == (guchar) 0x8b)) {
            priv->data_input_stream = g_memory_input_stream_new ();
        }
    }

    if (priv->data_input_stream) {
        g_memory_input_stream_add_data ((GMemoryInputStream *) priv->data_input_stream,
                                        g_memdup (buf, count), count, (GDestroyNotify) g_free);
        return TRUE;
    }

    return rsvg_handle_write_impl (handle, buf, count, error);
}

/**
 * rsvg_handle_close:
 * @handle: a #RsvgHandle
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return %TRUE if the loader closed successfully.  Note that @handle isn't
 * freed until @g_object_unref is called.
 *
 * Returns: %TRUE on success, or %FALSE on error
 **/
gboolean
rsvg_handle_close (RsvgHandle * handle, GError ** error)
{
    RsvgHandlePrivate *priv;

    rsvg_return_val_if_fail (handle, FALSE, error);
    priv = handle->priv;

    if (priv->is_closed)
          return TRUE;

    if (priv->data_input_stream) {
        gboolean ret;

        ret = rsvg_handle_read_stream_sync (handle, priv->data_input_stream, NULL, error);
        g_object_unref (priv->data_input_stream);
        priv->data_input_stream = NULL;

        return ret;
    }

    return rsvg_handle_close_impl (handle, error);
}

/**
 * rsvg_handle_read_stream_sync:
 * @handle: a #RsvgHandle
 * @stream: a #GInputStream
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Reads @stream and writes the data from it to @handle.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: %TRUE if reading @stream succeeded, or %FALSE otherwise
 *   with @error filled in
 *
 * Since: 2.32
 */
gboolean
rsvg_handle_read_stream_sync (RsvgHandle   *handle,
                              GInputStream *stream,
                              GCancellable *cancellable,
                              GError      **error)
{
    RsvgHandlePrivate *priv;
    xmlParserInputBufferPtr buffer;
    xmlParserInputPtr input;
    int result;
    xmlDocPtr doc;
    GError *err = NULL;
    gboolean res = FALSE;
    const guchar *buf;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), FALSE);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), FALSE);
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);

    priv = handle->priv;

    /* detect zipped streams */
    stream = g_buffered_input_stream_new (stream);
    if (g_buffered_input_stream_fill (G_BUFFERED_INPUT_STREAM (stream), 2, cancellable, error) != 2) {
        g_object_unref (stream);
        return FALSE;
    }
    buf = g_buffered_input_stream_peek_buffer (G_BUFFERED_INPUT_STREAM (stream), NULL);
    if ((buf[0] == 0x1f) && (buf[1] == 0x8b)) {
        GConverter *converter;
        GInputStream *conv_stream;

        converter = G_CONVERTER (g_zlib_decompressor_new (G_ZLIB_COMPRESSOR_FORMAT_GZIP));
        conv_stream = g_converter_input_stream_new (stream, converter);
        g_object_unref (converter);
        g_object_unref (stream);

        stream = conv_stream;
    }

    priv->error = &err;
    priv->cancellable = cancellable ? g_object_ref (cancellable) : NULL;
    create_xml_push_parser_ctxt (handle);

    buffer = _rsvg_xml_input_buffer_new_from_stream (stream, cancellable, XML_CHAR_ENCODING_NONE, &err);
    input = xmlNewIOInputStream (priv->ctxt, buffer, XML_CHAR_ENCODING_NONE);

    if (xmlPushInput (priv->ctxt, input) < 0) {
        rsvg_set_error (error, priv->ctxt);
        xmlFreeInputStream (input);
        goto out;
    }

    result = xmlParseDocument (priv->ctxt);
    if (result != 0) {
        if (err)
            g_propagate_error (error, err);
        else
            rsvg_set_error (error, handle->priv->ctxt);

        goto out;
    }

    if (err != NULL) {
        g_propagate_error (error, err);
        goto out;
    }

    doc = priv->ctxt->myDoc;
    xmlFreeParserCtxt (priv->ctxt);
    priv->ctxt = NULL;

    xmlFreeDoc (doc);

    priv->finished = TRUE;

    res = TRUE;

  out:

    g_object_unref (stream);

    priv->error = NULL;
    g_clear_object (&priv->cancellable);

    return res;
}

/**
 * rsvg_handle_new_from_gfile_sync:
 * @file: a #GFile
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @file.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: a new #RsvgHandle on success, or %NULL with @error filled in
 *
 * Since: 2.32
 */
RsvgHandle *
rsvg_handle_new_from_gfile_sync (GFile          *file,
                                 RsvgHandleFlags flags,
                                 GCancellable   *cancellable,
                                 GError        **error)
{
    RsvgHandle *handle;
    GFileInputStream *stream;

    g_return_val_if_fail (G_IS_FILE (file), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    stream = g_file_read (file, cancellable, error);
    if (stream == NULL)
        return NULL;

    handle = rsvg_handle_new_from_stream_sync (G_INPUT_STREAM (stream), file,
                                               flags, cancellable, error);
    g_object_unref (stream);

    return handle;
}

/**
 * rsvg_handle_new_from_stream_sync:
 * @input_stream: a #GInputStream
 * @base_file: (allow-none): a #GFile, or %NULL
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @stream.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: a new #RsvgHandle on success, or %NULL with @error filled in
 *
 * Since: 2.32
 */
RsvgHandle *
rsvg_handle_new_from_stream_sync (GInputStream   *input_stream,
                                  GFile          *base_file,
                                  RsvgHandleFlags flags,
                                  GCancellable    *cancellable,
                                  GError         **error)
{
    RsvgHandle *handle;

    g_return_val_if_fail (G_IS_INPUT_STREAM (input_stream), NULL);
    g_return_val_if_fail (base_file == NULL || G_IS_FILE (base_file), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    handle = rsvg_handle_new_with_flags (flags);

    if (base_file)
        rsvg_handle_set_base_gfile (handle, base_file);

    if (!rsvg_handle_read_stream_sync (handle, input_stream, cancellable, error)) {
        g_object_unref (handle);
        return NULL;
    }

    return handle;
}

/**
 * _rsvg_handle_internal_set_testing:
 * @handle: a #RsvgHandle
 * @testing: Whether to enable testing mode
 *
 * Do not call this function.  This is intended for librsvg's internal
 * test suite only.
 **/
void
rsvg_handle_internal_set_testing (RsvgHandle *handle, gboolean testing)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));

    handle->priv->is_testing = testing ? TRUE : FALSE;
}

/**
 * rsvg_init:
 *
 * Initializes librsvg
 * Since: 2.9
 * Deprecated: 2.36: Use g_type_init()
 **/
void
rsvg_init (void)
{
    RSVG_G_TYPE_INIT;
}

/**
 * rsvg_term:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36
 **/
void
rsvg_term (void)
{
}

/**
 * rsvg_cleanup:
 *
 * This function should not be called from normal programs.
 * See xmlCleanupParser() for more information.
 *
 * Since: 2.36
 **/
void
rsvg_cleanup (void)
{
    xmlCleanupParser ();
}

void
rsvg_pop_discrete_layer (RsvgDrawingCtx * ctx)
{
    ctx->render->pop_discrete_layer (ctx);
}

void
rsvg_push_discrete_layer (RsvgDrawingCtx * ctx)
{
    ctx->render->push_discrete_layer (ctx);
}

/*
 * rsvg_drawing_ctx_acquire_node:
 * @ctx: The drawing context in use
 * @url: The IRI to lookup, or %NULL
 *
 * Use this function when looking up urls to other nodes. This
 * function does proper recursion checking and thereby avoids
 * infinite loops.
 *
 * Nodes acquired by this function must be released using
 * rsvg_drawing_ctx_release_node() in reverse acquiring order.
 *
 * Note that if you acquire a node, you have to release it before trying to
 * acquire it again.  If you acquire a node "#foo" and don't release it before
 * trying to acquire "foo" again, you will obtain a %NULL the second time.
 *
 * Returns: The node referenced by @url; or %NULL if the @url
 *          is %NULL or it does not reference a node.
 */
RsvgNode *
rsvg_drawing_ctx_acquire_node (RsvgDrawingCtx * ctx, const char *url)
{
  RsvgNode *node;

  if (url == NULL)
      return NULL;

  node = rsvg_defs_lookup (ctx->defs, url);
  if (node == NULL)
    return NULL;

  if (g_slist_find (ctx->acquired_nodes, node))
    return NULL;

  ctx->acquired_nodes = g_slist_prepend (ctx->acquired_nodes, node);

  return node;
}

/**
 * rsvg_drawing_ctx_acquire_node_of_type:
 * @ctx: The drawing context in use
 * @url: The IRI to lookup
 * @type: Type which the node must have
 *
 * Use this function when looking up urls to other nodes, and when you expect
 * the node to be of a particular type. This function does proper recursion
 * checking and thereby avoids infinite loops.
 *
 * Malformed SVGs, for example, may reference a marker by its IRI, but
 * the object referenced by the IRI is not a marker.
 *
 * Nodes acquired by this function must be released using
 * rsvg_drawing_ctx_release_node() in reverse acquiring order.
 *
 * Note that if you acquire a node, you have to release it before trying to
 * acquire it again.  If you acquire a node "#foo" and don't release it before
 * trying to acquire "foo" again, you will obtain a %NULL the second time.
 *
 * Returns: The node referenced by @url or %NULL if the @url
 *          does not reference a node.  Also returns %NULL if
 *          the node referenced by @url is not of the specified @type.
 */
RsvgNode *
rsvg_drawing_ctx_acquire_node_of_type (RsvgDrawingCtx * ctx, const char *url, RsvgNodeType type)
{
    RsvgNode *node;

    node = rsvg_drawing_ctx_acquire_node (ctx, url);
    if (node == NULL || rsvg_node_get_type (node) != type) {
        rsvg_drawing_ctx_release_node (ctx, node);
        return NULL;
    }

    return node;
}

/*
 * rsvg_drawing_ctx_release_node:
 * @ctx: The drawing context the node was acquired from
 * @node: Node to release
 *
 * Releases a node previously acquired via rsvg_drawing_ctx_acquire_node() or
 * rsvg_drawing_ctx_acquire_node_of_type().
 *
 * if @node is %NULL, this function does nothing.
 */
void
rsvg_drawing_ctx_release_node (RsvgDrawingCtx * ctx, RsvgNode *node)
{
  if (node == NULL)
    return;

  g_return_if_fail (ctx->acquired_nodes != NULL);
  g_return_if_fail (ctx->acquired_nodes->data == node);

  ctx->acquired_nodes = g_slist_remove (ctx->acquired_nodes, node);
}

cairo_matrix_t
rsvg_drawing_ctx_get_current_state_affine (RsvgDrawingCtx *ctx)
{
    return rsvg_current_state (ctx)->affine;
}

void
rsvg_drawing_ctx_set_current_state_affine (RsvgDrawingCtx *ctx, cairo_matrix_t *affine)
{
    rsvg_current_state (ctx)->personal_affine =
        rsvg_current_state (ctx)->affine = *affine;
}

void
rsvg_render_path_builder (RsvgDrawingCtx * ctx, RsvgPathBuilder *builder)
{
    ctx->render->render_path_builder (ctx, builder);
}

void
rsvg_render_surface (RsvgDrawingCtx * ctx, cairo_surface_t *surface, double x, double y, double w, double h)
{
    /* surface must be a cairo image surface */
    g_return_if_fail (cairo_surface_get_type (surface) == CAIRO_SURFACE_TYPE_IMAGE);

    ctx->render->render_surface (ctx, surface, x, y, w, h);
}

double
rsvg_get_normalized_stroke_width (RsvgDrawingCtx *ctx)
{
    RsvgState *state = rsvg_current_state (ctx);

    return rsvg_length_normalize (&state->stroke_width, ctx);
}


const char *
rsvg_get_start_marker (RsvgDrawingCtx *ctx)
{
    RsvgState *state = rsvg_current_state (ctx);

    return state->startMarker;
}

const char *
rsvg_get_middle_marker (RsvgDrawingCtx *ctx)
{
    RsvgState *state = rsvg_current_state (ctx);

    return state->middleMarker;
}

const char *
rsvg_get_end_marker (RsvgDrawingCtx *ctx)
{
    RsvgState *state = rsvg_current_state (ctx);

    return state->endMarker;
}

void
rsvg_add_clipping_rect (RsvgDrawingCtx * ctx, double x, double y, double w, double h)
{
    ctx->render->add_clipping_rect (ctx, x, y, w, h);
}

cairo_surface_t *
rsvg_get_surface_of_node (RsvgDrawingCtx * ctx, RsvgNode * drawable, double w, double h)
{
    return ctx->render->get_surface_of_node (ctx, drawable, w, h);
}

void
rsvg_render_free (RsvgRender * render)
{
    render->free (render);
}

void
rsvg_drawing_ctx_push_view_box (RsvgDrawingCtx * ctx, double w, double h)
{
    RsvgViewBox *vb = g_new (RsvgViewBox, 1);
    *vb = ctx->vb;
    ctx->vb_stack = g_slist_prepend (ctx->vb_stack, vb);
    ctx->vb.rect.width = w;
    ctx->vb.rect.height = h;
}

void
rsvg_drawing_ctx_pop_view_box (RsvgDrawingCtx * ctx)
{
    ctx->vb = *((RsvgViewBox *) ctx->vb_stack->data);
    g_free (ctx->vb_stack->data);
    ctx->vb_stack = g_slist_delete_link (ctx->vb_stack, ctx->vb_stack);
}

void
rsvg_drawing_ctx_get_view_box_size (RsvgDrawingCtx *ctx, double *out_width, double *out_height)
{
    if (out_width)
        *out_width = ctx->vb.rect.width;

    if (out_height)
        *out_height = ctx->vb.rect.height;
}

void
rsvg_drawing_ctx_get_dpi (RsvgDrawingCtx *ctx, double *out_dpi_x, double *out_dpi_y)
{
    if (out_dpi_x)
        *out_dpi_x = ctx->dpi_x;

    if (out_dpi_y)
        *out_dpi_y = ctx->dpi_y;
}

char *
rsvg_get_url_string (const char *str, const char **out_rest)
{
    if (!strncmp (str, "url(", 4)) {
        const char *p = str + 4;
        int ix;

        while (g_ascii_isspace (*p))
            p++;

        for (ix = 0; p[ix]; ix++) {
            if (p[ix] == ')') {
                if (out_rest)
                    *out_rest = p + ix + 1;

                return g_strndup (p, ix);
            }
        }
    }

    if (out_rest)
        *out_rest = NULL;

    return NULL;
}

void
rsvg_return_if_fail_warning (const char *pretty_function, const char *expression, GError ** error)
{
    g_set_error (error, RSVG_ERROR, 0, _("%s: assertion `%s' failed"), pretty_function, expression);
}

static gboolean
_rsvg_handle_allow_load (RsvgHandle *handle,
                         const char *uri,
                         GError **error)
{
    RsvgHandlePrivate *priv = handle->priv;
    GFile *base;
    char *path, *dir;
    char *scheme = NULL, *cpath = NULL, *cdir = NULL;

    scheme = g_uri_parse_scheme (uri);

    /* Not a valid URI */
    if (scheme == NULL)
        goto deny;

    /* Allow loads of data: from any location */
    if (g_str_equal (scheme, "data"))
        goto allow;

    /* No base to compare to? */
    if (priv->base_gfile == NULL)
        goto deny;

    /* Deny loads from differing URI schemes */
    if (!g_file_has_uri_scheme (priv->base_gfile, scheme))
        goto deny;

    /* resource: is allowed to load anything from other resources */
    if (g_str_equal (scheme, "resource"))
        goto allow;

    /* Non-file: isn't allowed to load anything */
    if (!g_str_equal (scheme, "file"))
        goto deny;

    base = g_file_get_parent (priv->base_gfile);
    if (base == NULL)
        goto deny;

    dir = g_file_get_path (base);
    g_object_unref (base);

    cdir = realpath (dir, NULL);
    g_free (dir);
    if (cdir == NULL)
        goto deny;

    path = g_filename_from_uri (uri, NULL, NULL);
    if (path == NULL)
        goto deny;

    cpath = realpath (path, NULL);
    g_free (path);

    if (cpath == NULL)
        goto deny;

    /* Now check that @cpath is below @cdir */
    if (!g_str_has_prefix (cpath, cdir) ||
        cpath[strlen (cdir)] != G_DIR_SEPARATOR)
        goto deny;

    /* Allow load! */

 allow:
    g_free (scheme);
    free (cpath);
    free (cdir);
    return TRUE;

 deny:
    g_free (scheme);
    free (cpath);
    free (cdir);

    g_set_error (error, G_IO_ERROR, G_IO_ERROR_PERMISSION_DENIED,
                 "File may not link to URI \"%s\"", uri);
    return FALSE;
}

static char *
_rsvg_handle_resolve_uri (RsvgHandle *handle,
                          const char *uri)
{
    RsvgHandlePrivate *priv = handle->priv;
    char *scheme, *resolved_uri;
    GFile *base, *resolved;

    if (uri == NULL)
        return NULL;

    scheme = g_uri_parse_scheme (uri);
    if (scheme != NULL ||
        priv->base_gfile == NULL ||
        (base = g_file_get_parent (priv->base_gfile)) == NULL) {
        g_free (scheme);
        return g_strdup (uri);
    }

    resolved = g_file_resolve_relative_path (base, uri);
    resolved_uri = g_file_get_uri (resolved);

    g_free (scheme);
    g_object_unref (base);
    g_object_unref (resolved);

    return resolved_uri;
}

char *
_rsvg_handle_acquire_data (RsvgHandle *handle,
                           const char *url,
                           char **content_type,
                           gsize *len,
                           GError **error)
{
    char *uri;
    char *data;

    uri = _rsvg_handle_resolve_uri (handle, url);

    if (_rsvg_handle_allow_load (handle, uri, error)) {
        data = _rsvg_io_acquire_data (uri,
                                      rsvg_handle_get_base_uri (handle),
                                      content_type,
                                      len,
                                      handle->priv->cancellable,
                                      error);
    } else {
        data = NULL;
    }

    g_free (uri);
    return data;
}

GInputStream *
_rsvg_handle_acquire_stream (RsvgHandle *handle,
                             const char *url,
                             char **content_type,
                             GError **error)
{
    char *uri;
    GInputStream *stream;

    uri = _rsvg_handle_resolve_uri (handle, url);

    if (_rsvg_handle_allow_load (handle, uri, error)) {
        stream = _rsvg_io_acquire_stream (uri,
                                          rsvg_handle_get_base_uri (handle),
                                          content_type,
                                          handle->priv->cancellable,
                                          error);
    } else {
        stream = NULL;
    }

    g_free (uri);
    return stream;
}
