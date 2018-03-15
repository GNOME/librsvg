/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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

#include "rsvg-private.h"
#include "rsvg-attributes.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-structure.h"
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
    RsvgHandle *handle;
} RsvgSaxHandlerDefs;

typedef struct _RsvgSaxHandlerStyle {
    RsvgSaxHandler super;
    RsvgSaxHandlerDefs *parent;
    RsvgHandle *handle;
    GString *style;
    gboolean is_text_css;
} RsvgSaxHandlerStyle;

typedef struct {
    RsvgSaxHandler super;
    RsvgHandle *handle;
    const char *name;
    GString *string;
    GString **stringptr;
} RsvgSaxHandlerExtra;

/* hide this fact from the general public */
typedef RsvgSaxHandlerExtra RsvgSaxHandlerTitle;
typedef RsvgSaxHandlerExtra RsvgSaxHandlerDesc;

static void
rsvg_style_handler_free (RsvgSaxHandler * self)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    RsvgHandle *handle = z->handle;

    if (z->is_text_css)
        rsvg_parse_cssbuffer (handle, z->style->str, z->style->len);

    g_string_free (z->style, TRUE);
    g_free (z);
}

static void
rsvg_style_handler_characters (RsvgSaxHandler * self, const char *ch, gssize len)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    g_string_append_len (z->style, ch, len);
}

static void
rsvg_style_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag atts)
{
}

static void
rsvg_style_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    RsvgHandle *handle = z->handle;
    RsvgSaxHandler *prev = &z->parent->super;

    if (!strcmp (name, "style")) {
        if (handle->priv->handler != NULL) {
            handle->priv->handler->free (handle->priv->handler);
            handle->priv->handler = prev;
        }
    }
}

static void
rsvg_start_style (RsvgHandle *handle, RsvgPropertyBag *atts)
{
    RsvgSaxHandlerStyle *handler = g_new0 (RsvgSaxHandlerStyle, 1);
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    handler->super.free = rsvg_style_handler_free;
    handler->super.characters = rsvg_style_handler_characters;
    handler->super.start_element = rsvg_style_handler_start;
    handler->super.end_element = rsvg_style_handler_end;
    handler->handle = handle;

    handler->style = g_string_new (NULL);

    handler->parent = (RsvgSaxHandlerDefs *) handle->priv->handler;
    handle->priv->handler = &handler->super;

    /* FIXME: See these:
     *
     * https://www.w3.org/TR/SVG/styling.html#StyleElementTypeAttribute
     * https://www.w3.org/TR/SVG/styling.html#ContentStyleTypeAttribute
     *
     * If the "type" attribute is not present, we should fallback to the
     * "contentStyleType" attribute of the svg element, which in turn
     * defaults to "text/css".
     *
     * See where is_text_css is used to see where we parse the contents
     * of the style element.
     */
    handler->is_text_css = TRUE;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        if (attr == RSVG_ATTRIBUTE_TYPE) {
            handler->is_text_css = (g_ascii_strcasecmp (value, "text/css") == 0);
        }
    }

    rsvg_property_bag_iter_end (iter);
}

static void
add_node_to_handle (RsvgHandle *handle, RsvgNode *node)
{
    g_assert (handle != NULL);
    g_assert (node != NULL);

    g_ptr_array_add (handle->priv->all_nodes, rsvg_node_ref (node));
}

static void
push_element_name (RsvgHandle *handle, const char *name)
{
    /* libxml holds on to the name while parsing; we won't dup the name here */
    handle->priv->element_name_stack = g_slist_prepend (handle->priv->element_name_stack, (void *) name);
}

static gboolean
topmost_element_name_is (RsvgHandle *handle, const char *name)
{
    if (handle->priv->element_name_stack) {
        const char *name_in_stack = handle->priv->element_name_stack->data;

        return strcmp (name, name_in_stack) == 0;
    } else
        return FALSE;
}

static void
pop_element_name (RsvgHandle *handle)
{
    handle->priv->element_name_stack = g_slist_delete_link (handle->priv->element_name_stack,
                                                            handle->priv->element_name_stack);
}

static void
free_element_name_stack (RsvgHandle *handle)
{
    g_slist_free (handle->priv->element_name_stack);
    handle->priv->element_name_stack = NULL;
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
    { "a",                   TRUE,  rsvg_node_link_new },
    /* "altGlyph",           TRUE,  */
    /* "altGlyphDef",        FALSE, */
    /* "altGlyphItem",       FALSE, */
    /* "animate",            FALSE, */
    /* "animateColor",       FALSE, */
    /* "animateMotion",      FALSE, */
    /* "animateTransform",   FALSE, */
    { "circle",              TRUE,  rsvg_node_circle_new },
    { "clipPath",            TRUE,  rsvg_node_clip_path_new },
    /* "color-profile",      FALSE, */
    { "conicalGradient",     TRUE,  rsvg_node_radial_gradient_new },
    /* "cursor",             FALSE, */
    { "defs",                TRUE,  rsvg_node_defs_new },
    /* "desc",               TRUE,  */
    { "ellipse",             TRUE,  rsvg_node_ellipse_new },
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
    { "g",                   TRUE,  rsvg_node_group_new },
    /* "glyph",              TRUE,  */
    /* "glyphRef",           TRUE,  */
    /* "hkern",              FALSE, */
    { "image",               TRUE,  rsvg_node_image_new },
    { "line",                TRUE,  rsvg_node_line_new },
    { "linearGradient",      TRUE,  rsvg_node_linear_gradient_new },
    { "marker",              TRUE,  rsvg_node_marker_new },
    { "mask",                TRUE,  rsvg_node_mask_new },
    /* "metadata",           FALSE, */
    /* "missing-glyph",      TRUE,  */
    /* "mpath"               FALSE, */
    { "multiImage",          FALSE, rsvg_node_switch_new }, /* hack to make multiImage sort-of work */
    { "path",                TRUE,  rsvg_node_path_new },
    { "pattern",             TRUE,  rsvg_node_pattern_new },
    { "polygon",             TRUE,  rsvg_node_polygon_new },
    { "polyline",            TRUE,  rsvg_node_polyline_new },
    { "radialGradient",      TRUE,  rsvg_node_radial_gradient_new },
    { "rect",                TRUE,  rsvg_node_rect_new },
    /* "script",             FALSE, */
    /* "set",                FALSE, */
    { "stop",                TRUE,  rsvg_node_stop_new },
    /* "style",              FALSE, */
    { "subImage",            FALSE, rsvg_node_group_new },
    { "subImageRef",         FALSE, rsvg_node_image_new },
    { "svg",                 TRUE,  rsvg_node_svg_new },
    { "switch",              TRUE,  rsvg_node_switch_new },
    { "symbol",              TRUE,  rsvg_node_symbol_new },
    { "text",                TRUE,  rsvg_new_text },
    /* "textPath",           TRUE,  */
    /* "title",              TRUE,  */
    { "tref",                TRUE,  rsvg_new_tref },
    { "tspan",               TRUE,  rsvg_new_tspan },
    { "use",                 TRUE,  rsvg_node_use_new },
    /* "view",               FALSE, */
    /* "vkern",              FALSE, */
};

/* Whenever we encounter a node we don't understand, represent it as a defs.
 * This is like a group, but it doesn't do any rendering of children.  The
 * effect is that we will ignore all children of unknown elements.
 */
static const NodeCreator default_node_creator = { NULL, TRUE, rsvg_node_defs_new };

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
node_set_atts (RsvgNode * node, RsvgHandle *handle, const NodeCreator *creator, RsvgPropertyBag atts)
{
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    const char *id = NULL;
    const char *klazz = NULL;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_ID:
            id = value;
            rsvg_defs_register_node_by_id (handle->priv->defs, id, node);
            break;

        case RSVG_ATTRIBUTE_CLASS:
            if (creator->supports_class_attribute) {
                klazz = value;
            }
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);

    rsvg_node_set_atts (node, handle, atts);

    /* The "svg" node is special; it will load its id/class
     * attributes until the end, when rsvg_end_element() calls
     * rsvg_node_svg_apply_atts()
     */
    if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_SVG) {
        rsvg_parse_style_attrs (handle, node, creator->element_name, klazz, id, atts);
    }
}

static void
rsvg_standard_element_start (RsvgHandle *handle, const char *name, RsvgPropertyBag * atts)
{
    const NodeCreator *creator;
    RsvgNode *newnode = NULL;

    creator = get_node_creator_for_element_name (name);
    g_assert (creator != NULL && creator->create_fn != NULL);

    newnode = creator->create_fn (name, handle->priv->currentnode);
    g_assert (newnode != NULL);

    g_assert (rsvg_node_get_type (newnode) != RSVG_NODE_TYPE_INVALID);

    push_element_name (handle, name);

    add_node_to_handle (handle, newnode);

    if (handle->priv->currentnode) {
        rsvg_node_add_child (handle->priv->currentnode, newnode);
        handle->priv->currentnode = rsvg_node_unref (handle->priv->currentnode);
    } else if (rsvg_node_get_type (newnode) == RSVG_NODE_TYPE_SVG) {
        handle->priv->treebase = rsvg_node_ref (newnode);
    }

    handle->priv->currentnode = rsvg_node_ref (newnode);

    node_set_atts (newnode, handle, creator, atts);

    newnode = rsvg_node_unref (newnode);
}

/* extra (title, desc) */

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
rsvg_extra_handler_characters (RsvgSaxHandler * self, const char *ch, gssize len)
{
    RsvgSaxHandlerExtra *z = (RsvgSaxHandlerExtra *) self;

    /* This isn't quite the correct behavior - in theory, any graphics
       element may contain a title, desc, or metadata element */

    if (!z->string)
        return;

    if (!ch || !len)
        return;

    g_string_append_len (z->string, ch, len);
}

static void
rsvg_extra_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag atts)
{
}

static void
rsvg_extra_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerExtra *z = (RsvgSaxHandlerExtra *) self;
    RsvgHandle *handle = z->handle;

    if (!strcmp (name, z->name)) {
        if (handle->priv->handler != NULL) {
            handle->priv->handler->free (handle->priv->handler);
            handle->priv->handler = NULL;
        }
    }
}

static RsvgSaxHandlerExtra *
rsvg_start_extra (RsvgHandle *handle,
                  const char *name,
                  GString **stringptr)
{
    RsvgSaxHandlerExtra *handler = g_new0 (RsvgSaxHandlerExtra, 1);
    RsvgNode *treebase = handle->priv->treebase;
    RsvgNode *currentnode = handle->priv->currentnode;
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
    handler->handle = handle;
    handler->name = name; /* interned */
    handler->string = do_care ? g_string_new (NULL) : NULL;
    handler->stringptr = do_care ? stringptr : NULL;

    handle->priv->handler = &handler->super;

    return handler;
}

/* start desc */

static void
rsvg_start_desc (RsvgHandle *handle)
{
    rsvg_start_extra (handle, "desc", &handle->priv->desc);
}

/* end desc */

/* start title */

static void
rsvg_start_title (RsvgHandle *handle)
{
    rsvg_start_extra (handle, "title", &handle->priv->title);
}

/* end title */

/* start xinclude */

typedef struct _RsvgSaxHandlerXinclude {
    RsvgSaxHandler super;

    RsvgSaxHandler *prev_handler;
    RsvgHandle *handle;
    gboolean success;
    gboolean in_fallback;
} RsvgSaxHandlerXinclude;

static void rsvg_start_xinclude (RsvgHandle *handle, RsvgPropertyBag *atts);
static void rsvg_characters_impl (RsvgHandle *handle, const char *ch, gssize len);

static void
rsvg_xinclude_handler_free (RsvgSaxHandler * self)
{
    g_free (self);
}

static void
rsvg_xinclude_handler_characters (RsvgSaxHandler * self, const char *ch, gssize len)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;

    if (z->in_fallback) {
        rsvg_characters_impl (z->handle, ch, len);
    }
}

static void
rsvg_xinclude_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag atts)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;

    if (!z->success) {
        if (z->in_fallback) {
            if (!strcmp (name, "xi:include"))
                rsvg_start_xinclude (z->handle, atts);
            else
                rsvg_standard_element_start (z->handle, (const char *) name, atts);
        } else if (!strcmp (name, "xi:fallback")) {
            z->in_fallback = TRUE;
        }
    }
}

static void
rsvg_xinclude_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerXinclude *z = (RsvgSaxHandlerXinclude *) self;
    RsvgHandle *handle = z->handle;

    if (!strcmp (name, "include") || !strcmp (name, "xi:include")) {
        if (handle->priv->handler != NULL) {
            RsvgSaxHandler *previous_handler;

            previous_handler = z->prev_handler;
            handle->priv->handler->free (handle->priv->handler);
            handle->priv->handler = previous_handler;
        }
    } else if (z->in_fallback) {
        if (!strcmp (name, "xi:fallback"))
            z->in_fallback = FALSE;
    }
}

static void
rsvg_set_xml_parse_options(xmlParserCtxtPtr xml_parser,
                           RsvgHandle *handle)
{
    int options;

    options = (XML_PARSE_NONET |
               XML_PARSE_BIG_LINES);

    if (handle->priv->flags & RSVG_HANDLE_FLAG_UNLIMITED) {
        options |= XML_PARSE_HUGE;
    }

    xmlCtxtUseOptions (xml_parser, options);

    /* if false, external entities work, but internal ones don't. if true, internal entities
       work, but external ones don't. favor internal entities, in order to not cause a
       regression */
    xml_parser->replaceEntities = TRUE;
}

static xmlParserCtxtPtr
create_xml_push_parser (RsvgHandle *handle,
                        const char *base_uri)
{
    xmlParserCtxtPtr parser;

    parser = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0, base_uri);
    rsvg_set_xml_parse_options (parser, handle);

    return parser;
}

static xmlParserCtxtPtr
create_xml_stream_parser (RsvgHandle    *handle,
                          GInputStream  *stream,
                          GCancellable  *cancellable,
                          GError       **error)
{
    xmlParserCtxtPtr parser;

    parser = rsvg_create_xml_parser_from_stream (&rsvgSAXHandlerStruct,
                                                 handle,
                                                 stream,
                                                 cancellable,
                                                 error);
    if (parser) {
        rsvg_set_xml_parse_options (parser, handle);
    }

    return parser;
}

/* http://www.w3.org/TR/xinclude/ */
static void
rsvg_start_xinclude (RsvgHandle *handle, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerXinclude *handler;
    const char *href = NULL;
    const char *parse = NULL;
    const char *encoding = NULL;
    gboolean success = FALSE;

    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_HREF:
            href = value;
            break;

        case RSVG_ATTRIBUTE_PARSE:
            parse = value;
            break;

        case RSVG_ATTRIBUTE_ENCODING:
            encoding = value;
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);

    if (href) {
        if (parse && !strcmp (parse, "text")) {
            char *data;
            gsize data_len;

            data = _rsvg_handle_acquire_data (handle, href, NULL, &data_len, NULL);
            if (data) {
                if (encoding && g_ascii_strcasecmp (encoding, "UTF-8") != 0) {
                    char *text_data;
                    gsize text_data_len;

                    text_data = g_convert (data, data_len, "utf-8", encoding, NULL,
                                           &text_data_len, NULL);
                    g_free (data);

                    data = text_data;
                    data_len = text_data_len;
                }

                rsvg_characters_impl (handle, data, data_len);

                g_free (data);

                success = TRUE;
            }
        } else {
            /* xml */
            GInputStream *stream;
            GError *err = NULL;
            xmlParserCtxtPtr xml_parser;

            stream = _rsvg_handle_acquire_stream (handle, href, NULL, NULL);
            if (stream) {
                xml_parser = create_xml_stream_parser (handle,
                                                       stream,
                                                       NULL, /* cancellable */
                                                       &err);

                g_object_unref (stream);

                if (xml_parser) {
                    (void) xmlParseDocument (xml_parser);

                    xml_parser = rsvg_free_xml_parser_and_doc (xml_parser);
                }

                g_clear_error (&err);

                success = TRUE;
            }
        }
    }

    /* needed to handle xi:fallback */
    handler = g_new0 (RsvgSaxHandlerXinclude, 1);

    handler->super.free = rsvg_xinclude_handler_free;
    handler->super.characters = rsvg_xinclude_handler_characters;
    handler->super.start_element = rsvg_xinclude_handler_start;
    handler->super.end_element = rsvg_xinclude_handler_end;
    handler->prev_handler = handle->priv->handler;
    handler->handle = handle;
    handler->success = success;

    handle->priv->handler = &handler->super;
}

/* end xinclude */

static void
rsvg_start_element (void *data, const xmlChar * name, const xmlChar ** atts)
{
    RsvgPropertyBag bag;
    RsvgHandle *handle = (RsvgHandle *) data;

    bag = rsvg_property_bag_new ((const char **) atts);

    if (handle->priv->handler) {
        handle->priv->handler_nest++;
        if (handle->priv->handler->start_element != NULL)
            handle->priv->handler->start_element (handle->priv->handler, (const char *) name, bag);
    } else {
        const char *tempname;
        for (tempname = (const char *) name; *tempname != '\0'; tempname++)
            if (*tempname == ':')
                name = (const xmlChar *) (tempname + 1);

        if (!strcmp ((const char *) name, "style"))
            rsvg_start_style (handle, bag);
        else if (!strcmp ((const char *) name, "title"))
            rsvg_start_title (handle);
        else if (!strcmp ((const char *) name, "desc"))
            rsvg_start_desc (handle);
        else if (!strcmp ((const char *) name, "include"))      /* xi:include */
            rsvg_start_xinclude (handle, bag);
        else
            rsvg_standard_element_start (handle, (const char *) name, bag);
    }

    rsvg_property_bag_free (bag);
}

static void
rsvg_end_element (void *data, const xmlChar * xmlname)
{
    RsvgHandle *handle = (RsvgHandle *) data;
    const char *name = (const char *) xmlname;

    if (handle->priv->handler_nest > 0 && handle->priv->handler != NULL) {
        if (handle->priv->handler->end_element != NULL)
            handle->priv->handler->end_element (handle->priv->handler, name);
        handle->priv->handler_nest--;
    } else {
        const char *tempname;
        for (tempname = name; *tempname != '\0'; tempname++)
            if (*tempname == ':')
                name = tempname + 1;

        if (handle->priv->handler != NULL) {
            handle->priv->handler->free (handle->priv->handler);
            handle->priv->handler = NULL;
        }

        if (handle->priv->currentnode && rsvg_node_get_type (handle->priv->currentnode) == RSVG_NODE_TYPE_SVG) {
            rsvg_node_svg_apply_atts (handle->priv->currentnode, handle);
        }

        if (handle->priv->currentnode && topmost_element_name_is (handle, name)) {
            RsvgNode *parent;

            parent = rsvg_node_get_parent (handle->priv->currentnode);
            handle->priv->currentnode = rsvg_node_unref (handle->priv->currentnode);
            handle->priv->currentnode = parent;
            pop_element_name (handle);
        }
    }
}

/* Implemented in rust/src/chars.rs */
extern RsvgNode *rsvg_node_chars_new(RsvgNode *parent);

/* Implemented in rust/src/chars.rs */
extern void rsvg_node_chars_append (RsvgNode *node, const char *text, gssize len);

static gboolean
node_is_text_or_tspan (RsvgNode *node)
{
    RsvgNodeType type;

    if (!node) {
        return FALSE;
    }

    type = rsvg_node_get_type (node);
    return type == RSVG_NODE_TYPE_TEXT || type == RSVG_NODE_TYPE_TSPAN;
}

/* Finds the last chars child inside a given @node to which new characters can
 * be appended.  @node can be null; in this case we'll return NULL as we didn't
 * find any children.
 */
static RsvgNode *
find_last_chars_child (RsvgNode *node)
{
    RsvgNode *child = NULL;

    RsvgNode *temp;
    RsvgNodeChildrenIter *iter;

    if (node_is_text_or_tspan (node)) {
        /* find the last CHARS node in the text or tspan node, so that we can
         * coalesce the text, and thus avoid screwing up the Pango layouts.
         */
        iter = rsvg_node_children_iter_begin (node);

        while (rsvg_node_children_iter_next_back (iter, &temp)) {
            /* If a tspan node is encountered before any chars node
             * (which means there's a tspan node after any chars nodes,
             * because this is backwards iteration), return NULL.
             */
            if (rsvg_node_get_type (temp) == RSVG_NODE_TYPE_TSPAN) {
                temp = rsvg_node_unref (temp);
                break;
            } else if (rsvg_node_get_type (temp) == RSVG_NODE_TYPE_CHARS) {
                child = temp;
                break;
            } else {
                temp = rsvg_node_unref (temp);
            }
        }

        rsvg_node_children_iter_end (iter);
    }

    return child;
}

static RsvgNode *
add_new_chars_child_to_current_node (RsvgHandle *handle)
{
    RsvgNode *node;

    node = rsvg_node_chars_new (handle->priv->currentnode);
    add_node_to_handle (handle, node);

    if (handle->priv->currentnode) {
        rsvg_node_add_child (handle->priv->currentnode, node);
    }

    return node;
}

static void
rsvg_characters_impl (RsvgHandle *handle, const char *ch, gssize len)
{
    RsvgNode *node = NULL;

    if (!ch || !len) {
        return;
    }

    if (!node_is_text_or_tspan (handle->priv->currentnode)) {
        return;
    }

    node = find_last_chars_child (handle->priv->currentnode);

    if (!node) {
        node = add_new_chars_child_to_current_node (handle);
    }

    g_assert (rsvg_node_get_type (node) == RSVG_NODE_TYPE_CHARS);
    rsvg_node_chars_append (node, ch, len);

    node = rsvg_node_unref (node);
}

static void
rsvg_characters (void *data, const xmlChar * ch, int len)
{
    RsvgHandle *handle = (RsvgHandle *) data;

    if (handle->priv->handler && handle->priv->handler->characters != NULL) {
        handle->priv->handler->characters (handle->priv->handler, (const char *) ch, len);
        return;
    }

    rsvg_characters_impl (handle, (const char *) ch, len);
}

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar * name)
{
    RsvgHandle *handle = (RsvgHandle *) data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (handle->priv->entities, name);

    return entity;
}

static void
rsvg_entity_decl (void *data, const xmlChar * name, int type,
                  const xmlChar * publicId, const xmlChar * systemId, xmlChar * content)
{
    RsvgHandle *handle = (RsvgHandle *) data;
    GHashTable *entities = handle->priv->entities;
    xmlEntityPtr entity;
    xmlChar *resolvedSystemId = NULL, *resolvedPublicId = NULL;

    if (systemId)
        resolvedSystemId = xmlBuildRelativeURI (systemId, (xmlChar*) rsvg_handle_get_base_uri (handle));
    else if (publicId)
        resolvedPublicId = xmlBuildRelativeURI (publicId, (xmlChar*) rsvg_handle_get_base_uri (handle));

    if (type == XML_EXTERNAL_PARAMETER_ENTITY && !content) {
        char *entity_data;
        gsize entity_data_len;

        if (systemId)
            entity_data = _rsvg_handle_acquire_data (handle,
                                                     (const char *) systemId,
                                                     NULL,
                                                     &entity_data_len,
                                                     NULL);
        else if (publicId)
            entity_data = _rsvg_handle_acquire_data (handle,
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
rsvg_unparsed_entity_decl (void *handle,
                           const xmlChar * name,
                           const xmlChar * publicId,
                           const xmlChar * systemId, const xmlChar * notationName)
{
    rsvg_entity_decl (handle, name, XML_INTERNAL_GENERAL_ENTITY, publicId, systemId, NULL);
}

static xmlEntityPtr
rsvg_get_parameter_entity (void *data, const xmlChar * name)
{
    RsvgHandle *handle = (RsvgHandle *) data;
    xmlEntityPtr entity;

    entity = g_hash_table_lookup (handle->priv->entities, name);

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
rsvg_processing_instruction (void *user_data, const xmlChar * target, const xmlChar * data)
{
    /* http://www.w3.org/TR/xml-stylesheet/ */
    RsvgHandle *handle = user_data;

    if (!strcmp ((const char *) target, "xml-stylesheet")) {
        RsvgPropertyBag *atts;
        char **xml_atts;

        xml_atts = rsvg_css_parse_xml_attribute_string ((const char *) data);

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

            if ((!alternate || strcmp (alternate, "no") != 0)
                && type && strcmp (type, "text/css") == 0
                && href) {
                char *style_data;
                gsize style_data_len;
                char *mime_type = NULL;

                style_data = _rsvg_handle_acquire_data (handle,
                                                        href,
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

static gboolean
write_impl (RsvgHandle * handle, const guchar * buf, gsize count, GError ** error)
{
    GError *real_error = NULL;
    int result;

    rsvg_return_val_if_fail (handle != NULL, FALSE, error);

    handle->priv->error = &real_error;

    if (handle->priv->ctxt == NULL) {
        handle->priv->ctxt = create_xml_push_parser (handle, rsvg_handle_get_base_uri (handle));
    }

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
close_impl (RsvgHandle * handle, GError ** error)
{
    GError *real_error = NULL;

    handle->priv->error = &real_error;

    if (handle->priv->ctxt != NULL) {
        int result;

        result = xmlParseChunk (handle->priv->ctxt, "", 0, TRUE);
        if (result != 0) {
            rsvg_set_error (error, handle->priv->ctxt);
            handle->priv->ctxt = rsvg_free_xml_parser_and_doc (handle->priv->ctxt);
            return FALSE;
        }

        handle->priv->ctxt = rsvg_free_xml_parser_and_doc (handle->priv->ctxt);
    }

    free_element_name_stack (handle);

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
 * rsvg_set_default_dpi:
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
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
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi_x_y() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
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

#define GZ_MAGIC_0 ((guchar) 0x1f)
#define GZ_MAGIC_1 ((guchar) 0x8b)

/* Creates handle->priv->compressed_input_stream and adds the gzip header data
 * to it.  We implicitly consume the header data from the caller in
 * rsvg_handle_write(); that's why we add it back here.
 */
static void
create_compressed_input_stream (RsvgHandle *handle)
{
    RsvgHandlePrivate *priv = handle->priv;

    static const guchar gz_magic[2] = { GZ_MAGIC_0, GZ_MAGIC_1 };

    g_assert (priv->compressed_input_stream == NULL);

    priv->compressed_input_stream = g_memory_input_stream_new ();
    g_memory_input_stream_add_data (G_MEMORY_INPUT_STREAM (priv->compressed_input_stream),
                                    gz_magic, 2, NULL);
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

    rsvg_return_val_if_fail (priv->state == RSVG_HANDLE_STATE_START
                             || priv->state == RSVG_HANDLE_STATE_EXPECTING_GZ_1
                             || priv->state == RSVG_HANDLE_STATE_READING_COMPRESSED
                             || priv->state == RSVG_HANDLE_STATE_READING,
                             FALSE,
                             error);

    while (count > 0) {
        switch (priv->state) {
        case RSVG_HANDLE_STATE_START:
            if (buf[0] == GZ_MAGIC_0) {
                priv->state = RSVG_HANDLE_STATE_EXPECTING_GZ_1;
                buf++;
                count--;
            } else {
                priv->state = RSVG_HANDLE_STATE_READING;
                return write_impl (handle, buf, count, error);
            }

            break;

        case RSVG_HANDLE_STATE_EXPECTING_GZ_1:
            if (buf[0] == GZ_MAGIC_1) {
                priv->state = RSVG_HANDLE_STATE_READING_COMPRESSED;
                create_compressed_input_stream (handle);
                buf++;
                count--;
            } else {
                priv->state = RSVG_HANDLE_STATE_READING;
                return write_impl (handle, buf, count, error);
            }

            break;

        case RSVG_HANDLE_STATE_READING_COMPRESSED:
            g_memory_input_stream_add_data (G_MEMORY_INPUT_STREAM (priv->compressed_input_stream),
                                            g_memdup (buf, count), count, (GDestroyNotify) g_free);
            return TRUE;

        case RSVG_HANDLE_STATE_READING:
            return write_impl (handle, buf, count, error);

        default:
            g_assert_not_reached ();
        }
    }

    return TRUE;
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
    gboolean result;

    rsvg_return_val_if_fail (handle, FALSE, error);
    priv = handle->priv;

    if (priv->state == RSVG_HANDLE_STATE_CLOSED_OK
        || priv->state == RSVG_HANDLE_STATE_CLOSED_ERROR) {
        /* closing is idempotent */
        return TRUE;
    }

    if (priv->state == RSVG_HANDLE_STATE_READING_COMPRESSED) {
        gboolean ret;

        /* FIXME: when using rsvg_handle_write()/rsvg_handle_close(), as opposed to using the
         * stream functions, for compressed SVGs we buffer the whole compressed file in memory
         * and *then* uncompress/parse it here.
         *
         * We should make it so that the incoming data is decompressed and parsed on the fly.
         */
        priv->state = RSVG_HANDLE_STATE_START;
        ret = rsvg_handle_read_stream_sync (handle, priv->compressed_input_stream, NULL, error);
        g_object_unref (priv->compressed_input_stream);
        priv->compressed_input_stream = NULL;

        return ret;
    }

    result = close_impl (handle, error);

    if (result) {
        priv->state = RSVG_HANDLE_STATE_CLOSED_OK;
    } else {
        priv->state = RSVG_HANDLE_STATE_CLOSED_ERROR;
    }

    return result;
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
    int result;
    GError *err = NULL;
    gboolean res = FALSE;
    const guchar *buf;
    gssize num_read;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), FALSE);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), FALSE);
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);

    priv = handle->priv;

    g_return_val_if_fail (priv->state == RSVG_HANDLE_STATE_START, FALSE);

    /* detect zipped streams */
    stream = g_buffered_input_stream_new (stream);
    num_read = g_buffered_input_stream_fill (G_BUFFERED_INPUT_STREAM (stream), 2, cancellable, error);
    if (num_read < 2) {
        g_object_unref (stream);
        priv->state = RSVG_HANDLE_STATE_CLOSED_ERROR;
        if (num_read < 0) {
            g_assert (error == NULL || *error != NULL);
        } else {
            g_set_error (error, rsvg_error_quark (), RSVG_ERROR_FAILED,
                         _("Input file is too short"));
        }

        return FALSE;
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

    priv->error = &err;
    priv->cancellable = cancellable ? g_object_ref (cancellable) : NULL;

    g_assert (handle->priv->ctxt == NULL);
    handle->priv->ctxt = create_xml_stream_parser (handle,
                                                   stream,
                                                   cancellable,
                                                   &err);

    if (!handle->priv->ctxt) {
        if (err) {
            g_propagate_error (error, err);
        }

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

    res = TRUE;

  out:

    priv->ctxt = rsvg_free_xml_parser_and_doc (priv->ctxt);

    g_object_unref (stream);

    priv->error = NULL;
    g_clear_object (&priv->cancellable);

    if (res) {
        priv->state = RSVG_HANDLE_STATE_CLOSED_OK;
    } else {
        priv->state = RSVG_HANDLE_STATE_CLOSED_ERROR;
    }

    return res;
}

/**
 * rsvg_init:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to initialize librsvg.
 **/
void
rsvg_init (void)
{
}

/**
 * rsvg_term:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to de-initialize librsvg.
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

void
rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx, RsvgNode *node)
{
    if (node) {
        node = rsvg_node_ref (node);

        while (node != NULL) {
            draw_ctx->drawsub_stack = g_slist_prepend (draw_ctx->drawsub_stack, node);
            node = rsvg_node_get_parent (node);
        }
    }
}

void
rsvg_drawing_ctx_draw_node_from_stack (RsvgDrawingCtx *ctx, RsvgNode *node, int dominate)
{
    RsvgState *state;
    GSList *stacksave;

    stacksave = ctx->drawsub_stack;
    if (stacksave) {
        RsvgNode *stack_node = stacksave->data;

        if (!rsvg_node_is_same (stack_node, node))
            return;

        ctx->drawsub_stack = stacksave->next;
    }

    state = rsvg_node_get_state (node);

    if (state->visible) {
        rsvg_state_push (ctx);

        rsvg_node_draw (node, ctx, dominate);

        rsvg_state_pop (ctx);
    }

    ctx->drawsub_stack = stacksave;
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

PangoContext *
rsvg_drawing_ctx_get_pango_context (RsvgDrawingCtx *draw_ctx)
{
    return draw_ctx->render->get_pango_context (draw_ctx);
}

void
rsvg_drawing_ctx_render_pango_layout (RsvgDrawingCtx *draw_ctx,
                                      PangoLayout *layout,
                                      double x,
                                      double y)
{
    draw_ctx->render->render_pango_layout (draw_ctx, layout, x, y);
}

void
rsvg_drawing_ctx_render_path_builder (RsvgDrawingCtx * ctx, RsvgPathBuilder *builder)
{
    ctx->render->render_path_builder (ctx, builder);
}

void
rsvg_drawing_ctx_render_surface (RsvgDrawingCtx * ctx, cairo_surface_t *surface,
                                 double x, double y, double w, double h)
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
rsvg_drawing_ctx_add_clipping_rect (RsvgDrawingCtx * ctx, double x, double y, double w, double h)
{
    ctx->render->add_clipping_rect (ctx, x, y, w, h);
}

cairo_surface_t *
rsvg_get_surface_of_node (RsvgDrawingCtx * ctx, RsvgNode * drawable, double w, double h)
{
    return ctx->render->get_surface_of_node (ctx, drawable, w, h);
}

cairo_surface_t *
rsvg_cairo_surface_new_from_href (RsvgHandle *handle,
                                  const char *href,
                                  GError **error)
{
    char *data;
    gsize data_len;
    char *mime_type = NULL;
    GdkPixbufLoader *loader = NULL;
    GdkPixbuf *pixbuf = NULL;
    cairo_surface_t *surface = NULL;

    data = _rsvg_handle_acquire_data (handle, href, &mime_type, &data_len, error);
    if (data == NULL)
        return NULL;

    if (mime_type) {
        loader = gdk_pixbuf_loader_new_with_mime_type (mime_type, error);
    } else {
        loader = gdk_pixbuf_loader_new ();
    }

    if (loader == NULL)
        goto out;

    if (!gdk_pixbuf_loader_write (loader, (guchar *) data, data_len, error)) {
        gdk_pixbuf_loader_close (loader, NULL);
        goto out;
    }

    if (!gdk_pixbuf_loader_close (loader, error))
        goto out;

    pixbuf = gdk_pixbuf_loader_get_pixbuf (loader);

    if (!pixbuf) {
        g_set_error (error,
                     GDK_PIXBUF_ERROR,
                     GDK_PIXBUF_ERROR_FAILED,
                      _("Failed to load image '%s': reason not known, probably a corrupt image file"),
                      href);
        goto out;
    }

    surface = rsvg_cairo_surface_from_pixbuf (pixbuf);

    if (mime_type == NULL) {
        /* Try to get the information from the loader */
        GdkPixbufFormat *format;
        char **mime_types;

        if ((format = gdk_pixbuf_loader_get_format (loader)) != NULL) {
            mime_types = gdk_pixbuf_format_get_mime_types (format);

            if (mime_types != NULL)
                mime_type = g_strdup (mime_types[0]);
            g_strfreev (mime_types);
        }
    }

    if ((handle->priv->flags & RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA) != 0 &&
        mime_type != NULL &&
        cairo_surface_set_mime_data (surface, mime_type, (guchar *) data,
                                     data_len, g_free, data) == CAIRO_STATUS_SUCCESS) {
        data = NULL; /* transferred to the surface */
    }

  out:
    if (loader)
        g_object_unref (loader);
    g_free (mime_type);
    g_free (data);

    return surface;
}

void
rsvg_render_free (RsvgRender * render)
{
    render->free (render);
}

void
rsvg_drawing_ctx_push_view_box (RsvgDrawingCtx * ctx, double w, double h)
{
    RsvgViewBox *vb = g_new0 (RsvgViewBox, 1);
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

gboolean
rsvg_allow_load (GFile       *base_gfile,
                 const char  *uri,
                 GError     **error)
{
    GFile *base;
    char *path, *dir;
    char *scheme = NULL, *cpath = NULL, *cdir = NULL;

    g_assert (error == NULL || *error == NULL);

    scheme = g_uri_parse_scheme (uri);

    /* Not a valid URI */
    if (scheme == NULL)
        goto deny;

    /* Allow loads of data: from any location */
    if (g_str_equal (scheme, "data"))
        goto allow;

    /* No base to compare to? */
    if (base_gfile == NULL)
        goto deny;

    /* Deny loads from differing URI schemes */
    if (!g_file_has_uri_scheme (base_gfile, scheme))
        goto deny;

    /* resource: is allowed to load anything from other resources */
    if (g_str_equal (scheme, "resource"))
        goto allow;

    /* Non-file: isn't allowed to load anything */
    if (!g_str_equal (scheme, "file"))
        goto deny;

    base = g_file_get_parent (base_gfile);
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

char *
rsvg_handle_resolve_uri (RsvgHandle *handle,
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
    RsvgHandlePrivate *priv = handle->priv;
    char *uri;
    char *data;

    uri = rsvg_handle_resolve_uri (handle, url);

    if (rsvg_allow_load (priv->base_gfile, uri, error)) {
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
    RsvgHandlePrivate *priv = handle->priv;
    char *uri;
    GInputStream *stream;

    uri = rsvg_handle_resolve_uri (handle, url);

    if (rsvg_allow_load (priv->base_gfile, uri, error)) {
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

/* Frees the ctxt and its ctxt->myDoc - libxml2 doesn't free them together
 * http://xmlsoft.org/html/libxml-parser.html#xmlFreeParserCtxt
 *
 * Returns NULL.
 */
xmlParserCtxtPtr
rsvg_free_xml_parser_and_doc (xmlParserCtxtPtr ctxt)
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
