/* vim: set sw=4 sts=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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

#ifdef HAVE_SVGZ
#include <gsf/gsf-input-gzip.h>
#include <gsf/gsf-input-memory.h>
#include <gsf/gsf-output-memory.h>
#include <gsf/gsf-utils.h>
#endif

#include "rsvg.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-shapes.h"
#include "rsvg-image.h"
#include "rsvg-text.h"
#include "rsvg-filter.h"
#include "rsvg-mask.h"
#include "rsvg-marker.h"

#include <math.h>
#include <string.h>
#include <stdarg.h>

#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-paint-server.h"

/*
 * This is configurable at runtime
 */
#define RSVG_DEFAULT_DPI_X 90.0
#define RSVG_DEFAULT_DPI_Y 90.0
double rsvg_internal_dpi_x = RSVG_DEFAULT_DPI_X;
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
} RsvgSaxHandlerStyle;

/* hide this fact from the general public */
typedef RsvgSaxHandlerDefs RsvgSaxHandlerTitle;
typedef RsvgSaxHandlerDefs RsvgSaxHandlerDesc;
typedef RsvgSaxHandlerDefs RsvgSaxHandlerMetadata;

static void
rsvg_style_handler_free (RsvgSaxHandler * self)
{
    RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *) self;
    RsvgHandle *ctx = z->ctx;

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
rsvg_start_style (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerStyle *handler = g_new0 (RsvgSaxHandlerStyle, 1);

    handler->super.free = rsvg_style_handler_free;
    handler->super.characters = rsvg_style_handler_characters;
    handler->super.start_element = rsvg_style_handler_start;
    handler->super.end_element = rsvg_style_handler_end;
    handler->ctx = ctx;

    handler->style = g_string_new (NULL);

    handler->parent = (RsvgSaxHandlerDefs *) ctx->priv->handler;
    ctx->priv->handler = &handler->super;
}


static void
rsvg_standard_element_start (RsvgHandle * ctx, const char *name, RsvgPropertyBag * atts)
{

    /*replace this stuff with a hash for fast reading! */
    RsvgNode *newnode = NULL;
    if (!strcmp (name, "g"))
        newnode = rsvg_new_group ();
    else if (!strcmp (name, "a"))       /*treat anchors as groups for now */
        newnode = rsvg_new_group ();
    else if (!strcmp (name, "switch"))
        newnode = rsvg_new_switch ();
    else if (!strcmp (name, "defs"))
        newnode = rsvg_new_defs ();
    else if (!strcmp (name, "use"))
        newnode = rsvg_new_use ();
    else if (!strcmp (name, "path"))
        newnode = rsvg_new_path ();
    else if (!strcmp (name, "line"))
        newnode = rsvg_new_line ();
    else if (!strcmp (name, "rect"))
        newnode = rsvg_new_rect ();
    else if (!strcmp (name, "ellipse"))
        newnode = rsvg_new_ellipse ();
    else if (!strcmp (name, "circle"))
        newnode = rsvg_new_circle ();
    else if (!strcmp (name, "polygon"))
        newnode = rsvg_new_polygon ();
    else if (!strcmp (name, "polyline"))
        newnode = rsvg_new_polyline ();
    else if (!strcmp (name, "symbol"))
        newnode = rsvg_new_symbol ();
    else if (!strcmp (name, "svg"))
        newnode = rsvg_new_svg ();
    else if (!strcmp (name, "mask"))
        newnode = rsvg_new_mask ();
    else if (!strcmp (name, "clipPath"))
        newnode = rsvg_new_clip_path ();
    else if (!strcmp (name, "image"))
        newnode = rsvg_new_image ();
    else if (!strcmp (name, "marker"))
        newnode = rsvg_new_marker ();
    else if (!strcmp (name, "stop"))
        newnode = rsvg_new_stop ();
    else if (!strcmp (name, "pattern"))
        newnode = rsvg_new_pattern ();
    else if (!strcmp (name, "linearGradient"))
        newnode = rsvg_new_linear_gradient ();
    else if (!strcmp (name, "radialGradient"))
        newnode = rsvg_new_radial_gradient ();
    else if (!strcmp (name, "conicalGradient"))
        newnode = rsvg_new_radial_gradient ();
    else if (!strcmp (name, "filter"))
        newnode = rsvg_new_filter ();
    else if (!strcmp (name, "feBlend"))
        newnode = rsvg_new_filter_primitive_blend ();
    else if (!strcmp (name, "feColorMatrix"))
        newnode = rsvg_new_filter_primitive_colour_matrix ();
    else if (!strcmp (name, "feComponentTransfer"))
        newnode = rsvg_new_filter_primitive_component_transfer ();
    else if (!strcmp (name, "feComposite"))
        newnode = rsvg_new_filter_primitive_composite ();
    else if (!strcmp (name, "feConvolveMatrix"))
        newnode = rsvg_new_filter_primitive_convolve_matrix ();
    else if (!strcmp (name, "feDiffuseLighting"))
        newnode = rsvg_new_filter_primitive_diffuse_lighting ();
    else if (!strcmp (name, "feDisplacementMap"))
        newnode = rsvg_new_filter_primitive_displacement_map ();
    else if (!strcmp (name, "feFlood"))
        newnode = rsvg_new_filter_primitive_flood ();
    else if (!strcmp (name, "feGaussianBlur"))
        newnode = rsvg_new_filter_primitive_gaussian_blur ();
    else if (!strcmp (name, "feImage"))
        newnode = rsvg_new_filter_primitive_image ();
    else if (!strcmp (name, "feMerge"))
        newnode = rsvg_new_filter_primitive_merge ();
    else if (!strcmp (name, "feMorphology"))
        newnode = rsvg_new_filter_primitive_erode ();
    else if (!strcmp (name, "feOffset"))
        newnode = rsvg_new_filter_primitive_offset ();
    else if (!strcmp (name, "feSpecularLighting"))
        newnode = rsvg_new_filter_primitive_specular_lighting ();
    else if (!strcmp (name, "feTile"))
        newnode = rsvg_new_filter_primitive_tile ();
    else if (!strcmp (name, "feTurbulence"))
        newnode = rsvg_new_filter_primitive_turbulence ();
    else if (!strcmp (name, "feMergeNode"))
        newnode = rsvg_new_filter_primitive_merge_node ();
    else if (!strcmp (name, "feFuncR"))
        newnode = rsvg_new_node_component_transfer_function ('r');
    else if (!strcmp (name, "feFuncG"))
        newnode = rsvg_new_node_component_transfer_function ('g');
    else if (!strcmp (name, "feFuncB"))
        newnode = rsvg_new_node_component_transfer_function ('b');
    else if (!strcmp (name, "feFuncA"))
        newnode = rsvg_new_node_component_transfer_function ('a');
    else if (!strcmp (name, "feDistantLight"))
        newnode = rsvg_new_filter_primitive_light_source ('d');
    else if (!strcmp (name, "feSpotLight"))
        newnode = rsvg_new_filter_primitive_light_source ('s');
    else if (!strcmp (name, "fePointLight"))
        newnode = rsvg_new_filter_primitive_light_source ('p');
    /* hack to make multiImage sort-of work */
    else if (!strcmp (name, "multiImage"))
        newnode = rsvg_new_switch ();
    else if (!strcmp (name, "subImageRef"))
        newnode = rsvg_new_image ();
    else if (!strcmp (name, "subImage"))
        newnode = rsvg_new_group ();
    else if (!strcmp (name, "text"))
        newnode = rsvg_new_text ();
    else if (!strcmp (name, "tspan"))
        newnode = rsvg_new_tspan ();
    else if (!strcmp (name, "tref"))
        newnode = rsvg_new_tref ();
	else {
		/* hack for bug 401115. whenever we encounter a node we don't understand, push it into a group. 
		   this will allow us to handle things like conditionals properly. */
		newnode = rsvg_new_group ();
	}

    if (newnode) {
        newnode->type = g_string_new (name);
	newnode->parent = ctx->priv->currentnode;
        rsvg_node_set_atts (newnode, ctx, atts);
        rsvg_defs_register_memory (ctx->priv->defs, newnode);
        if (ctx->priv->currentnode) {
            rsvg_node_group_pack (ctx->priv->currentnode, newnode);
            ctx->priv->currentnode = newnode;
        } else if (!strcmp (name, "svg")) {
            ctx->priv->treebase = newnode;
            ctx->priv->currentnode = newnode;
        }
    }
}

/* start desc */

static void
rsvg_desc_handler_free (RsvgSaxHandler * self)
{
    g_free (self);
}

static void
rsvg_desc_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *) self;
    RsvgHandle *ctx = z->ctx;

    /* This isn't quite the correct behavior - in theory, any graphics
       element may contain a title or desc element */

    if (!ch || !len)
        return;

    if (!g_utf8_validate ((char *) ch, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
        g_string_append (ctx->priv->desc, utf8);
        g_free (utf8);
    } else {
        g_string_append_len (ctx->priv->desc, (char *) ch, len);
    }
}

static void
rsvg_desc_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
}

static void
rsvg_desc_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *) self;
    RsvgHandle *ctx = z->ctx;

    if (!strcmp (name, "desc")) {
        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }
    }
}

static void
rsvg_start_desc (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerDesc *handler = g_new0 (RsvgSaxHandlerDesc, 1);

    handler->super.free = rsvg_desc_handler_free;
    handler->super.characters = rsvg_desc_handler_characters;
    handler->super.start_element = rsvg_desc_handler_start;
    handler->super.end_element = rsvg_desc_handler_end;
    handler->ctx = ctx;

    ctx->priv->desc = g_string_new (NULL);
    ctx->priv->handler = &handler->super;
}

/* end desc */

/* start title */

static void
rsvg_title_handler_free (RsvgSaxHandler * self)
{
    g_free (self);
}

static void
rsvg_title_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerTitle *z = (RsvgSaxHandlerTitle *) self;
    RsvgHandle *ctx = z->ctx;

    /* This isn't quite the correct behavior - in theory, any graphics
       element may contain a title or desc element */

    if (!ch || !len)
        return;

    if (!g_utf8_validate ((char *) ch, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
        g_string_append (ctx->priv->title, utf8);
        g_free (utf8);
    } else {
        g_string_append_len (ctx->priv->title, (char *) ch, len);
    }
}

static void
rsvg_title_handler_start (RsvgSaxHandler * self, const char *name, RsvgPropertyBag * atts)
{
}

static void
rsvg_title_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerTitle *z = (RsvgSaxHandlerTitle *) self;
    RsvgHandle *ctx = z->ctx;

    if (!strcmp (name, "title")) {
        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }
    }
}

static void
rsvg_start_title (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerTitle *handler = g_new0 (RsvgSaxHandlerTitle, 1);

    handler->super.free = rsvg_title_handler_free;
    handler->super.characters = rsvg_title_handler_characters;
    handler->super.start_element = rsvg_title_handler_start;
    handler->super.end_element = rsvg_title_handler_end;
    handler->ctx = ctx;

    ctx->priv->title = g_string_new (NULL);
    ctx->priv->handler = &handler->super;
}

/* end title */

/* start metadata */

static void
rsvg_metadata_handler_free (RsvgSaxHandler * self)
{
    g_free (self);
}

static void
rsvg_metadata_handler_characters (RsvgSaxHandler * self, const char *ch, int len)
{
    RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *) self;
    RsvgHandle *ctx = z->ctx;

    /* This isn't quite the correct behavior - in theory, any graphics
       element may contain a metadata or desc element */

    if (!ch || !len)
        return;

    if (!g_utf8_validate ((char *) ch, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
        g_string_append (ctx->priv->metadata, utf8);
        g_free (utf8);
    } else {
        g_string_append_len (ctx->priv->metadata, (char *) ch, len);
    }
}

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
    RsvgHandle *ctx = z->ctx;

    g_string_append_printf (ctx->priv->metadata, "<%s ", name);
    rsvg_property_bag_enumerate (atts, rsvg_metadata_props_enumerate, ctx->priv->metadata);
    g_string_append (ctx->priv->metadata, ">\n");
}

static void
rsvg_metadata_handler_end (RsvgSaxHandler * self, const char *name)
{
    RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *) self;
    RsvgHandle *ctx = z->ctx;

    if (!strcmp (name, "metadata")) {
        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }
    } else
        g_string_append_printf (ctx->priv->metadata, "</%s>\n", name);
}

static void
rsvg_start_metadata (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerMetadata *handler = g_new0 (RsvgSaxHandlerMetadata, 1);

    handler->super.free = rsvg_metadata_handler_free;
    handler->super.characters = rsvg_metadata_handler_characters;
    handler->super.start_element = rsvg_metadata_handler_start;
    handler->super.end_element = rsvg_metadata_handler_end;
    handler->ctx = ctx;

    ctx->priv->metadata = g_string_new (NULL);
    ctx->priv->handler = &handler->super;
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

/* http://www.w3.org/TR/xinclude/ */
static void
rsvg_start_xinclude (RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    RsvgSaxHandlerXinclude *handler;
    GByteArray *data;
    const char *href;
    gboolean success = FALSE;

    href = rsvg_property_bag_lookup (atts, "href");
    if (href) {
        data = _rsvg_acquire_xlink_href_resource (href, rsvg_handle_get_base_uri (ctx), NULL);
        if (data) {
            const char *parse;

            parse = rsvg_property_bag_lookup (atts, "parse");
            if (parse && !strcmp (parse, "text")) {
                const char *encoding;
                char *text_data;
                gsize text_data_len;
                gboolean text_data_needs_free = FALSE;

                encoding = rsvg_property_bag_lookup (atts, "encoding");
                if (encoding) {
                    text_data =
                        g_convert ((const char *) data->data, data->len, "utf-8", encoding, NULL,
                                   &text_data_len, NULL);
                    text_data_needs_free = TRUE;
                } else {
                    text_data = (char *) data->data;
                    text_data_len = data->len;
                }

                rsvg_characters_impl (ctx, (const xmlChar *) text_data, text_data_len);

                if (text_data_needs_free)
                    g_free (text_data);
            } else {
                /* xml */
                xmlDocPtr xml_doc;
                xmlParserCtxtPtr xml_parser;
                int result;

                xml_parser = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, ctx, NULL, 0, NULL);
                result = xmlParseChunk (xml_parser, (char *) data->data, data->len, 0);
                result = xmlParseChunk (xml_parser, "", 0, TRUE);

                xml_doc = xml_parser->myDoc;
                xmlFreeParserCtxt (xml_parser);
                xmlFreeDoc (xml_doc);
            }

            g_byte_array_free (data, TRUE);
            success = TRUE;
        }
    }

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
            rsvg_start_title (ctx, bag);
        else if (!strcmp ((const char *) name, "desc"))
            rsvg_start_desc (ctx, bag);
        else if (!strcmp ((const char *) name, "metadata"))
            rsvg_start_metadata (ctx, bag);
        else if (!strcmp ((const char *) name, "include"))      /* xi:include */
            rsvg_start_xinclude (ctx, bag);
        else
            rsvg_standard_element_start (ctx, (const char *) name, bag);
    }

    rsvg_property_bag_free (bag);
}

static void
rsvg_end_element (void *data, const xmlChar * name)
{
    RsvgHandle *ctx = (RsvgHandle *) data;

    if (ctx->priv->handler_nest > 0 && ctx->priv->handler != NULL) {
        if (ctx->priv->handler->end_element != NULL)
            ctx->priv->handler->end_element (ctx->priv->handler, (const char *) name);
        ctx->priv->handler_nest--;
    } else {
        const char *tempname;
        for (tempname = (const char *) name; *tempname != '\0'; tempname++)
            if (*tempname == ':')
                name = (const xmlChar *) (tempname + 1);

        if (ctx->priv->handler != NULL) {
            ctx->priv->handler->free (ctx->priv->handler);
            ctx->priv->handler = NULL;
        }

        if (ctx->priv->currentnode
            && !strcmp ((const char *) name, ctx->priv->currentnode->type->str))
            rsvg_pop_def_group (ctx);

    }
}

static void
_rsvg_node_chars_free (RsvgNode * node)
{
    RsvgNodeChars *self = (RsvgNodeChars *) node;
    g_string_free (self->contents, TRUE);
    _rsvg_node_free (node);
}

static void
rsvg_characters_impl (RsvgHandle * ctx, const xmlChar * ch, int len)
{
    RsvgNodeChars *self;

    if (!ch || !len)
        return;

	if (ctx->priv->currentnode)
		{
			if (!strcmp ("tspan", ctx->priv->currentnode->type->str) ||
				!strcmp ("text", ctx->priv->currentnode->type->str))
				{
					guint i;

					/* find the last CHARS node in the text or tspan node, so that we
					   can coalesce the text, and thus avoid screwing up the Pango layouts */
					self = NULL;
					for (i = 0; i < ctx->priv->currentnode->children->len; i++) {
						RsvgNode *node = g_ptr_array_index (ctx->priv->currentnode->children, i);
						if (!strcmp (node->type->str, "RSVG_NODE_CHARS")) {
							self = (RsvgNodeChars*)node;
						}
					}

					if (self != NULL) {
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

    self = g_new (RsvgNodeChars, 1);
    _rsvg_node_init (&self->super);

    if (!g_utf8_validate ((char *) ch, len, NULL)) {
        char *utf8;
        utf8 = rsvg_make_valid_utf8 ((char *) ch, len);
        self->contents = g_string_new (utf8);
        g_free (utf8);
    } else {
        self->contents = g_string_new_len ((char *) ch, len);
    }

    self->super.type = g_string_new ("RSVG_NODE_CHARS");
    self->super.free = _rsvg_node_chars_free;
    self->super.state->cond_true = FALSE;

    rsvg_defs_register_memory (ctx->priv->defs, (RsvgNode *) self);
    if (ctx->priv->currentnode)
        rsvg_node_group_pack (ctx->priv->currentnode, (RsvgNode *) self);
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

#if LIBXML_VERSION >= 20621
#define RSVG_ENABLE_ENTITIES
#elif defined(__GNUC__)
#warning "libxml version less than 2.6.22. XML entities won't work"
#endif

#ifdef RSVG_ENABLE_ENTITIES

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
    xmlChar *dupname;

    entity = xmlMalloc (sizeof (xmlEntity));
    memset (entity, 0, sizeof (xmlEntity));
    entity->type = XML_ENTITY_DECL;
    dupname = (xmlChar *) xmlMemStrdup ((const char *) name);
    entity->name = dupname;

    entity->etype = type;
    if (content) {
        entity->content = (xmlChar *) xmlMemStrdup ((const char *) content);
        entity->length = strlen ((const char *) content);
    } else if (systemId || publicId) {
        GByteArray *data = NULL;

        if (systemId)
            data =
                _rsvg_acquire_xlink_href_resource ((const char *) systemId,
                                                   rsvg_handle_get_base_uri (ctx), NULL);
        else if (publicId)
            data =
                _rsvg_acquire_xlink_href_resource ((const char *) publicId,
                                                   rsvg_handle_get_base_uri (ctx), NULL);

        if (data) {
            entity->SystemID = (xmlChar *) xmlMemStrdup ((const char *) systemId);
            entity->ExternalID = (xmlChar *) xmlMemStrdup ((const char *) publicId);
            entity->content = (xmlChar *) xmlMemStrdup ((const char *) data->data);
            entity->length = data->len;

            /* fool libxml2 into supporting SYSTEM and PUBLIC entities */
            entity->etype = XML_INTERNAL_GENERAL_ENTITY;

            g_byte_array_free (data, TRUE);
        }
    }

    g_hash_table_insert (entities, dupname, entity);
}

static void
rsvg_unparsed_entity_decl (void *ctx,
                           const xmlChar * name,
                           const xmlChar * publicId,
                           const xmlChar * systemId, const xmlChar * notationName)
{
    rsvg_entity_decl (ctx, name, XML_INTERNAL_GENERAL_ENTITY, publicId, systemId, NULL);
}

#endif

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
        atts = rsvg_property_bag_new ((const char **) xml_atts);

        if (atts) {
            const char *value;

            value = rsvg_property_bag_lookup (atts, "alternate");
            if (!value || (strcmp (value, "no") != 0)) {
                value = rsvg_property_bag_lookup (atts, "type");
                if (value && strcmp (value, "text/css") == 0) {
                    value = rsvg_property_bag_lookup (atts, "href");
                    if (value) {
                        GByteArray *style;

                        style =
                            _rsvg_acquire_xlink_href_resource (value,
                                                               rsvg_handle_get_base_uri (handle),
                                                               NULL);
                        if (style) {
                            rsvg_parse_cssbuffer (handle, (char *) style->data, style->len);
                            g_byte_array_free (style, TRUE);
                        }
                    }
                }
            }

            g_strfreev (xml_atts);
            rsvg_property_bag_free (atts);
        }
    }
}

void
rsvg_SAX_handler_struct_init (void)
{
    if (!rsvgSAXHandlerStructInited) {
        rsvgSAXHandlerStructInited = TRUE;

        memset (&rsvgSAXHandlerStruct, 0, sizeof (rsvgSAXHandlerStruct));

#ifdef RSVG_ENABLE_ENTITIES
        rsvgSAXHandlerStruct.getEntity = rsvg_get_entity;
        rsvgSAXHandlerStruct.entityDecl = rsvg_entity_decl;
        rsvgSAXHandlerStruct.unparsedEntityDecl = rsvg_unparsed_entity_decl;
#endif
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

    if (   (path[0] < 'a' || path[0] > 'z')
	&& (path[0] < 'A' || path[0] > 'Z'))
	return FALSE;

    for (p = &path[1];
	    (*p >= 'a' && *p <= 'z') 
	 || (*p >= 'A' && *p <= 'Z') 
	 || (*p >= '0' && *p <= '9') 
	 || *p == '+' 
	 || *p == '-' 
	 || *p == '.';
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
 * rsvg_handle_set_base_uri
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

    g_return_if_fail (handle != NULL);

    if (base_uri == NULL)
	return;

    if (rsvg_path_is_uri (base_uri)) 
	uri = g_strdup (base_uri);
    else
	uri = rsvg_get_base_uri_from_filename (base_uri);

    if (uri) {
        if (handle->priv->base_uri)
            g_free (handle->priv->base_uri);
        handle->priv->base_uri = uri;
        rsvg_defs_set_base_uri (handle->priv->defs, handle->priv->base_uri);
    }
}

/**
 * rsvg_handle_get_base_uri:
 * @handle: A #RsvgHandle
 *
 * Gets the base uri for this #RsvgHandle.
 *
 * Returns: the base uri, possibly null
 * Since: 2.9 (really present in 2.8 as well)
 */
G_CONST_RETURN char *
rsvg_handle_get_base_uri (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);
    return handle->priv->base_uri;
}

/**
 * rsvg_error_quark
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

static gboolean
rsvg_handle_write_impl (RsvgHandle * handle, const guchar * buf, gsize count, GError ** error)
{
    GError *real_error = NULL;
    int result;

    rsvg_return_val_if_fail (handle != NULL, FALSE, error);

    handle->priv->error = &real_error;
    if (handle->priv->ctxt == NULL) {
        handle->priv->ctxt = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0,
                                                      rsvg_handle_get_base_uri (handle));

        /* if false, external entities work, but internal ones don't. if true, internal entities
           work, but external ones don't. favor internal entities, in order to not cause a
           regression */
        handle->priv->ctxt->replaceEntities = TRUE;
    }

    result = xmlParseChunk (handle->priv->ctxt, (char *) buf, count, 0);
    if (result != 0) {
        g_set_error (error, rsvg_error_quark (), 0, _("Error parsing XML data"));
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
        xmlDocPtr xmlDoc;
        int result;

        xmlDoc = handle->priv->ctxt->myDoc;

        result = xmlParseChunk (handle->priv->ctxt, "", 0, TRUE);
        xmlFreeParserCtxt (handle->priv->ctxt);
        xmlFreeDoc (xmlDoc);

        if (result != 0) {
            g_set_error (error, rsvg_error_quark (), 0, _("Error parsing XML data"));
            return FALSE;
        }
    }

    rsvg_defs_resolve_all (handle->priv->defs);
    handle->priv->finished = TRUE;
    handle->priv->error = NULL;

    if (real_error != NULL) {
        g_propagate_error (error, real_error);
        return FALSE;
    }

    return TRUE;
}

static void
rsvg_state_free_func (gpointer data, gpointer user_data)
{
    rsvg_state_finalize ((RsvgState *) data);
    g_slice_free (RsvgState, data);
}

void
rsvg_drawing_ctx_free (RsvgDrawingCtx * handle)
{
    rsvg_render_free (handle->render);

    g_slist_foreach (handle->state, rsvg_state_free_func, (gpointer) handle);
    g_slist_free (handle->state);

	/* the drawsub stack's nodes are owned by the ->defs */
	g_slist_free (handle->drawsub_stack);

    if (handle->base_uri)
        g_free (handle->base_uri);

    if (handle->pango_context != NULL)
        g_object_unref (handle->pango_context);

    g_free (handle);
}

/**
 * rsvg_handle_get_metadata:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's metadata in UTF-8 or %NULL. You must make a copy
 * of this metadata if you wish to use it after #handle has been freed.
 *
 * Returns: The SVG's title
 *
 * Since: 2.9
 */
G_CONST_RETURN char *
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
 * of this title if you wish to use it after #handle has been freed.
 *
 * Returns: The SVG's title
 *
 * Since: 2.4
 */
G_CONST_RETURN char *
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
 * of this description if you wish to use it after #handle has been freed.
 *
 * Returns: The SVG's description
 *
 * Since: 2.4
 */
G_CONST_RETURN char *
rsvg_handle_get_desc (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->desc)
        return handle->priv->desc->str;
    else
        return NULL;
}

typedef struct {
    RsvgRender super;
    RsvgBbox bbox;
} RsvgBboxRender;

static void
rsvg_bbox_render_path (RsvgDrawingCtx * ctx, const RsvgBpathDef * bpath_def)
{
    RsvgState *state = rsvg_state_current (ctx);
    RsvgBpath *bpath;
    RsvgBboxRender *render = (RsvgBboxRender *) ctx->render;
    RsvgBbox bbox;
    int i;

    rsvg_bbox_init (&bbox, state->affine);
    bbox.w = bbox.h = bbox.virgin = 0;

    for (i = 0; i < bpath_def->n_bpath; i++) {
        bpath = &bpath_def->bpath[i];

        switch (bpath->code) {
        case RSVG_MOVETO:
        case RSVG_MOVETO_OPEN:
        case RSVG_CURVETO:
        case RSVG_LINETO:
            bbox.x = bpath->x3;
            bbox.y = bpath->y3;
            rsvg_bbox_insert (&render->bbox, &bbox);
            break;
        default:
            break;
        }
    }
}

static void
rsvg_bbox_render_image (RsvgDrawingCtx * ctx,
                        const GdkPixbuf * pixbuf,
                        double pixbuf_x, double pixbuf_y, double w, double h)
{
    RsvgState *state = rsvg_state_current (ctx);
    RsvgBboxRender *render = (RsvgBboxRender *) ctx->render;
    RsvgBbox bbox;

    rsvg_bbox_init (&bbox, state->affine);
    bbox.x = pixbuf_x;
    bbox.y = pixbuf_y;
    bbox.w = w;
    bbox.h = h;
    bbox.virgin = 0;

    rsvg_bbox_insert (&render->bbox, &bbox);
}


static void
rsvg_bbox_render_free (RsvgRender * self)
{
    g_free (self);
}

static void
rsvg_bbox_push_discrete_layer (RsvgDrawingCtx * ctx)
{
}

static void
rsvg_bbox_pop_discrete_layer (RsvgDrawingCtx * ctx)
{
}

static void
rsvg_bbox_add_clipping_rect (RsvgDrawingCtx * ctx, double x, double y, double w, double h)
{
}

static RsvgBboxRender *
rsvg_bbox_render_new ()
{
    RsvgBboxRender *render = g_new0 (RsvgBboxRender, 1);
    double affine[6];

    render->super.free = rsvg_bbox_render_free;
    render->super.render_image = rsvg_bbox_render_image;
    render->super.render_path = rsvg_bbox_render_path;
    render->super.pop_discrete_layer = rsvg_bbox_pop_discrete_layer;
    render->super.push_discrete_layer = rsvg_bbox_push_discrete_layer;
    render->super.add_clipping_rect = rsvg_bbox_add_clipping_rect;
    render->super.get_image_of_node = NULL;
    _rsvg_affine_identity (affine);
    rsvg_bbox_init (&render->bbox, affine);

    return render;
}

static RsvgBbox
_rsvg_find_bbox (RsvgHandle * handle)
{
    RsvgDrawingCtx *ctx = g_new (RsvgDrawingCtx, 1);
    RsvgBbox output;
    RsvgBboxRender *render = rsvg_bbox_render_new ();
    ctx->drawsub_stack = NULL;
    ctx->render = (RsvgRender *) render;

    ctx->state = NULL;

    ctx->defs = handle->priv->defs;
    ctx->base_uri = g_strdup (handle->priv->base_uri);
    ctx->dpi_x = handle->priv->dpi_x;
    ctx->dpi_y = handle->priv->dpi_y;
    ctx->vb.w = 512;
    ctx->vb.h = 512;
    ctx->pango_context = NULL;

    rsvg_state_push (ctx);
    _rsvg_affine_identity (rsvg_state_current (ctx)->affine);
    _rsvg_node_draw_children ((RsvgNode *) handle->priv->treebase, ctx, 0);
    rsvg_state_pop (ctx);

    output = render->bbox;
    rsvg_render_free (ctx->render);
    g_free (ctx);
    return output;
}

/**
 * rsvg_handle_get_dimensions
 * @handle: A #RsvgHandle
 * @dimension_data: A place to store the SVG's size
 *
 * Get the SVG's size. Do not call from within the size_func callback, because an infinite loop will occur.
 *
 * Since: 2.14
 */
void
rsvg_handle_get_dimensions (RsvgHandle * handle, RsvgDimensionData * dimension_data)
{
    RsvgNodeSvg *sself;
    RsvgBbox bbox;

    g_return_if_fail (handle);
    g_return_if_fail (dimension_data);

    memset (dimension_data, 0, sizeof (RsvgDimensionData));

    sself = (RsvgNodeSvg *) handle->priv->treebase;
    if (!sself)
        return;

    bbox.x = bbox.y = 0;
    bbox.w = bbox.h = 1;

    if (sself->w.factor == 'p' || sself->h.factor == 'p') {
        if (sself->vbox.active && sself->vbox.w > 0. && sself->vbox.h > 0.) {
            bbox.w = sself->vbox.w;
            bbox.h = sself->vbox.h;
        } else
            bbox = _rsvg_find_bbox (handle);
    }

    dimension_data->width = (int) (_rsvg_css_hand_normalize_length (&sself->w, handle->priv->dpi_x,
                                                                    bbox.w + bbox.x * 2, 12) + 0.5);
    dimension_data->height = (int) (_rsvg_css_hand_normalize_length (&sself->h, handle->priv->dpi_y,
                                                                     bbox.h + bbox.y * 2,
                                                                     12) + 0.5);

    dimension_data->em = dimension_data->width;
    dimension_data->ex = dimension_data->height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data->width, &dimension_data->height,
                                    handle->priv->user_data);
}

/** 
 * rsvg_set_default_dpi
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the all future outgoing pixbufs. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
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
 * rsvg_set_default_dpi_x_y
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the all future outgoing pixbufs. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
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
 * rsvg_handle_set_dpi
 * @handle: An #RsvgHandle
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
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
 * rsvg_handle_set_dpi_x_y
 * @handle: An #RsvgHandle
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi_x or #dpi_y will 
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
 * @size_func: A sizing function, or %NULL
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
 * @handle: An #RsvgHandle
 * @buf: Pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: return location for errors
 *
 * Loads the next @count bytes of the image.  This will return #TRUE if the data
 * was loaded successful, and #FALSE if an error occurred.  In the latter case,
 * the loader will be closed, and will not accept further writes. If FALSE is
 * returned, @error will be set to an error from the #RSVG_ERROR domain.
 *
 * Returns: #TRUE if the write was successful, or #FALSE if there was an
 * error.
 **/
gboolean
rsvg_handle_write (RsvgHandle * handle, const guchar * buf, gsize count, GError ** error)
{
    rsvg_return_val_if_fail (handle, FALSE, error);
    rsvg_return_val_if_fail (!handle->priv->is_closed, FALSE, error);

    if (handle->priv->first_write) {
        handle->priv->first_write = FALSE;

        /* test for GZ marker. todo: store the first 2 bytes in the odd circumstance that someone calls
         * write() in 1 byte increments */
        if ((count >= 2) && (buf[0] == (guchar) 0x1f) && (buf[1] == (guchar) 0x8b)) {
            handle->priv->is_gzipped = TRUE;

#ifdef HAVE_SVGZ
            handle->priv->gzipped_data = GSF_OUTPUT (gsf_output_memory_new ());
#endif
        }
    }

    if (handle->priv->is_gzipped) {
#ifdef HAVE_SVGZ
        return gsf_output_write (handle->priv->gzipped_data, count, buf);
#else
        return FALSE;
#endif
    }

    return rsvg_handle_write_impl (handle, buf, count, error);
}

/**
 * rsvg_handle_close:
 * @handle: A #RsvgHandle
 * @error: A #GError
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return #TRUE if the loader closed successfully.  Note that @handle isn't
 * freed until @g_object_unref is called.
 *
 * Returns: #TRUE if the loader closed successfully, or #FALSE if there was
 * an error.
 **/
gboolean
rsvg_handle_close (RsvgHandle * handle, GError ** error)
{
    rsvg_return_val_if_fail (handle, FALSE, error);

	if (handle->priv->is_closed)
		return TRUE;

#if HAVE_SVGZ
    if (handle->priv->is_gzipped) {
        GsfInput *gzip;
        const guchar *bytes;
        gsize size;
        gsize remaining;

        bytes = gsf_output_memory_get_bytes (GSF_OUTPUT_MEMORY (handle->priv->gzipped_data));
        size = gsf_output_size (handle->priv->gzipped_data);

        gzip =
            GSF_INPUT (gsf_input_gzip_new
                       (GSF_INPUT (gsf_input_memory_new (bytes, size, FALSE)), error));
        remaining = gsf_input_remaining (gzip);
        while ((size = MIN (remaining, 1024)) > 0) {
            guint8 const *buf;

            /* write to parent */
            buf = gsf_input_read (gzip, size, NULL);
            if (!buf) {
                /* an error occured, so bail */
                g_warning (_("rsvg_gz_handle_close_impl: gsf_input_read returned NULL"));
                break;
            }

            rsvg_handle_write_impl (handle, buf, size, error);
            /* if we didn't manage to lower remaining number of bytes,
             * something is wrong, and we should avoid an endless loop */
            if (remaining == ((gsize) gsf_input_remaining (gzip))) {
                g_warning (_
                           ("rsvg_gz_handle_close_impl: write_impl didn't lower the input_remaining count"));
                break;
            }
            remaining = gsf_input_remaining (gzip);
        }
        g_object_unref (G_OBJECT (gzip));

        /* close parent */
        gsf_output_close (handle->priv->gzipped_data);
    }
#endif

    return rsvg_handle_close_impl (handle, error);
}

#ifdef HAVE_GNOME_VFS
#include <libgnomevfs/gnome-vfs.h>
#endif

/**
 * rsvg_init:
 *
 * Initializes librsvg
 * Since: 2.9
 **/
void
rsvg_init (void)
{
    g_type_init ();

#ifdef HAVE_SVGZ
    gsf_init ();
#endif

    xmlInitParser ();
}

/**
 * rsvg_term:
 *
 * De-initializes librsvg
 * Since: 2.9
 **/
void
rsvg_term (void)
{
#ifdef HAVE_SVGZ
    gsf_shutdown ();
#endif

    xmlCleanupParser ();
}

void
rsvg_node_set_atts (RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
    node->set_atts (node, ctx, atts);
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

void
rsvg_render_path (RsvgDrawingCtx * ctx, const char *d)
{
    /* todo: store and use the bpath higher up */
    RsvgBpathDef *bpath_def;

    bpath_def = rsvg_parse_path (d);
    rsvg_bpath_def_art_finish (bpath_def);

    ctx->render->render_path (ctx, bpath_def);
    rsvg_render_markers (bpath_def, ctx);

    rsvg_bpath_def_free (bpath_def);
}

void
rsvg_render_image (RsvgDrawingCtx * ctx, GdkPixbuf * pb, double x, double y, double w, double h)
{
    ctx->render->render_image (ctx, pb, x, y, w, h);
}

void
rsvg_add_clipping_rect (RsvgDrawingCtx * ctx, double x, double y, double w, double h)
{
    ctx->render->add_clipping_rect (ctx, x, y, w, h);
}

GdkPixbuf *
rsvg_get_image_of_node (RsvgDrawingCtx * ctx, RsvgNode * drawable, double w, double h)
{
    return ctx->render->get_image_of_node (ctx, drawable, w, h);
}

void
rsvg_render_free (RsvgRender * render)
{
    render->free (render);
}

void
rsvg_bbox_init (RsvgBbox * self, double *affine)
{
    int i;
    self->virgin = 1;
    for (i = 0; i < 6; i++)
        self->affine[i] = affine[i];
}

void
rsvg_bbox_insert (RsvgBbox * dst, RsvgBbox * src)
{
    double affine[6];
    double xmin, ymin;
    double xmax, ymax;
    int i;

    if (src->virgin)
        return;

    if (!dst->virgin)
        {
            xmin = dst->x, ymin = dst->y;
            xmax = dst->x + dst->w, ymax = dst->y + dst->h;
        }
    else
        {
            xmin = ymin = xmax = ymax = 0;
        }

    _rsvg_affine_invert (affine, dst->affine);
    _rsvg_affine_multiply (affine, src->affine, affine);

    for (i = 0; i < 4; i++) {
        double rx, ry, x, y;
        rx = src->x + src->w * (double) (i % 2);
        ry = src->y + src->h * (double) (i / 2);
        x = affine[0] * rx + affine[2] * ry + affine[4];
        y = affine[1] * rx + affine[3] * ry + affine[5];
        if (dst->virgin) {
            xmin = xmax = x;
            ymin = ymax = y;
            dst->virgin = 0;
        } else {
            if (x < xmin)
                xmin = x;
            if (x > xmax)
                xmax = x;
            if (y < ymin)
                ymin = y;
            if (y > ymax)
                ymax = y;
        }
    }
    dst->x = xmin;
    dst->y = ymin;
    dst->w = xmax - xmin;
    dst->h = ymax - ymin;
}

void
rsvg_bbox_clip (RsvgBbox * dst, RsvgBbox * src)
{
    double affine[6];
	double xmin, ymin;
	double xmax, ymax;
    int i;

    if (src->virgin)
        return;

	if (!dst->virgin)
		{
			xmin = dst->x + dst->w, ymin = dst->y + dst->h;
			xmax = dst->x, ymax = dst->y;
		}
	else
		{
			xmin = ymin = xmax = ymax = 0;
		}

    _rsvg_affine_invert (affine, dst->affine);
    _rsvg_affine_multiply (affine, src->affine, affine);

    for (i = 0; i < 4; i++) {
        double rx, ry, x, y;
        rx = src->x + src->w * (double) (i % 2);
        ry = src->y + src->h * (double) (i / 2);
        x = affine[0] * rx + affine[2] * ry + affine[4];
        y = affine[1] * rx + affine[3] * ry + affine[5];
        if (dst->virgin) {
            xmin = xmax = x;
            ymin = ymax = y;
            dst->virgin = 0;
        } else {
            if (x < xmin)
                xmin = x;
            if (x > xmax)
                xmax = x;
            if (y < ymin)
                ymin = y;
            if (y > ymax)
                ymax = y;
        }
    }

    if (xmin < dst->x)
        xmin = dst->x;
    if (ymin < dst->y)
        ymin = dst->y;
    if (xmax > dst->x + dst->w)
        xmax = dst->x + dst->w;
    if (ymax > dst->y + dst->h)
        ymax = dst->y + dst->h;

    dst->x = xmin;
    dst->w = xmax - xmin;
    dst->y = ymin;
    dst->h = ymax - ymin;
}

void
_rsvg_push_view_box (RsvgDrawingCtx * ctx, double w, double h)
{
    RsvgViewBox *vb = g_new (RsvgViewBox, 1);
    *vb = ctx->vb;
    ctx->vb_stack = g_slist_prepend (ctx->vb_stack, vb);
    ctx->vb.w = w;
    ctx->vb.h = h;
}

void
_rsvg_pop_view_box (RsvgDrawingCtx * ctx)
{
    ctx->vb = *((RsvgViewBox *) ctx->vb_stack->data);
    g_free (ctx->vb_stack->data);
    ctx->vb_stack = g_slist_delete_link (ctx->vb_stack, ctx->vb_stack);
}

void
rsvg_return_if_fail_warning (const char *pretty_function, const char *expression, GError ** error)
{
    g_set_error (error, RSVG_ERROR, 0, _("%s: assertion `%s' failed"), pretty_function, expression);
}
