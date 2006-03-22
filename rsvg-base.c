/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
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
rsvg_style_handler_free (RsvgSaxHandler *self)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	RsvgHandle *ctx = z->ctx;
	
	rsvg_parse_cssbuffer (ctx, z->style->str, z->style->len);
	
	g_string_free (z->style, TRUE);
	g_free (z);
}

static void
rsvg_style_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	g_string_append_len (z->style, (const char *)ch, len);
}

static void
rsvg_style_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						  RsvgPropertyBag *atts)
{
}

static void
rsvg_style_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerStyle *z = (RsvgSaxHandlerStyle *)self;
	RsvgHandle *ctx = z->ctx;
	RsvgSaxHandler *prev = &z->parent->super;
	
	if (!strcmp ((char *)name, "style"))
		{
			if (ctx->priv->handler != NULL)
				{
					ctx->priv->handler->free (ctx->priv->handler);
					ctx->priv->handler = prev;
				}
		}
}

static void
rsvg_start_style (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerStyle *handler = g_new0 (RsvgSaxHandlerStyle, 1);
	
	handler->super.free = rsvg_style_handler_free;
	handler->super.characters = rsvg_style_handler_characters;
	handler->super.start_element = rsvg_style_handler_start;
	handler->super.end_element   = rsvg_style_handler_end;
	handler->ctx = ctx;
	
	handler->style = g_string_new (NULL);
	
	handler->parent = (RsvgSaxHandlerDefs*)ctx->priv->handler;
	ctx->priv->handler = &handler->super;
}


static void
rsvg_standard_element_start (RsvgHandle *ctx, const xmlChar *name,
							 RsvgPropertyBag *atts)
{

	/*replace this stuff with a hash for fast reading!*/
	RsvgNode * newnode = NULL;
	if (!strcmp ((char *)name, "g"))
		newnode = rsvg_new_group ();
	else if (!strcmp ((char *)name, "a")) /*treat anchors as groups for now*/
		newnode = rsvg_new_group ();
	else if (!strcmp ((char *)name, "switch"))
		newnode = rsvg_new_switch ();
	else if (!strcmp ((char *)name, "defs"))
		newnode = rsvg_new_defs ();	
	else if (!strcmp ((char *)name, "use"))
		newnode = rsvg_new_use ();
	else if (!strcmp ((char *)name, "path"))
		newnode = rsvg_new_path ();
	else if (!strcmp ((char *)name, "line"))
		newnode = rsvg_new_line ();
	else if (!strcmp ((char *)name, "rect"))
		newnode = rsvg_new_rect ();
	else if (!strcmp ((char *)name, "ellipse"))
		newnode = rsvg_new_ellipse ();
	else if (!strcmp ((char *)name, "circle"))
		newnode = rsvg_new_circle ();
	else if (!strcmp ((char *)name, "polygon"))
		newnode = rsvg_new_polygon ();
	else if (!strcmp ((char *)name, "polyline"))
		newnode = rsvg_new_polyline ();
	else if (!strcmp ((char *)name, "symbol"))
		newnode = rsvg_new_symbol ();
	else if (!strcmp ((char *)name, "svg"))
		newnode = rsvg_new_svg ();
	else if (!strcmp ((char *)name, "mask"))
		newnode = rsvg_new_mask();
	else if (!strcmp ((char *)name, "clipPath"))
		newnode = rsvg_new_clip_path();
	else if (!strcmp ((char *)name, "image"))
		newnode = rsvg_new_image ();
	else if (!strcmp ((char *)name, "marker"))
		newnode = rsvg_new_marker ();
	else if (!strcmp ((char *)name, "stop"))
		newnode = rsvg_new_stop ();
	else if (!strcmp ((char *)name, "pattern"))
		newnode = rsvg_new_pattern ();
	else if (!strcmp ((char *)name, "linearGradient"))
		newnode = rsvg_new_linear_gradient ();
	else if (!strcmp ((char *)name, "radialGradient"))
		newnode = rsvg_new_radial_gradient ();
	else if (!strcmp ((char *)name, "conicalGradient"))
		newnode = rsvg_new_radial_gradient ();
	else if (!strcmp ((char *)name, "filter"))
		newnode = rsvg_new_filter();
	else if (!strcmp ((char *)name, "feBlend"))
		newnode = rsvg_new_filter_primitive_blend ();
	else if (!strcmp ((char *)name, "feColorMatrix"))
		newnode = rsvg_new_filter_primitive_colour_matrix();
	else if (!strcmp ((char *)name, "feComponentTransfer"))
		newnode = rsvg_new_filter_primitive_component_transfer();
	else if (!strcmp ((char *)name, "feComposite"))
		newnode = rsvg_new_filter_primitive_composite();
	else if (!strcmp ((char *)name, "feConvolveMatrix"))
		newnode = rsvg_new_filter_primitive_convolve_matrix ();
	else if (!strcmp ((char *)name, "feDiffuseLighting"))
		newnode = rsvg_new_filter_primitive_diffuse_lighting();
	else if (!strcmp ((char *)name, "feDisplacementMap"))
		newnode = rsvg_new_filter_primitive_displacement_map();
	else if (!strcmp ((char *)name, "feFlood"))
		newnode = rsvg_new_filter_primitive_flood();
	else if (!strcmp ((char *)name, "feGaussianBlur"))
		newnode = rsvg_new_filter_primitive_gaussian_blur ();
	else if (!strcmp ((char *)name, "feImage"))
		newnode = rsvg_new_filter_primitive_image ();
	else if (!strcmp ((char *)name, "feMerge"))
		newnode = rsvg_new_filter_primitive_merge();
	else if (!strcmp ((char *)name, "feMorphology"))
		newnode = rsvg_new_filter_primitive_erode();
	else if (!strcmp ((char *)name, "feOffset"))
		newnode = rsvg_new_filter_primitive_offset();
	else if (!strcmp ((char *)name, "feSpecularLighting"))
		newnode = rsvg_new_filter_primitive_specular_lighting();
	else if (!strcmp ((char *)name, "feTile"))
		newnode = rsvg_new_filter_primitive_tile();
	else if (!strcmp ((char *)name, "feTurbulence"))
		newnode = rsvg_new_filter_primitive_turbulence();
	else if (!strcmp ((char *)name, "feMergeNode"))
		newnode = rsvg_new_filter_primitive_merge_node();
	else if (!strcmp ((char *)name, "feFuncR"))
		newnode = rsvg_new_node_component_transfer_function('r');
	else if (!strcmp ((char *)name, "feFuncG"))
		newnode = rsvg_new_node_component_transfer_function('g');
	else if (!strcmp ((char *)name, "feFuncB"))
		newnode = rsvg_new_node_component_transfer_function('b');
	else if (!strcmp ((char *)name, "feFuncA"))
		newnode = rsvg_new_node_component_transfer_function('a');
	else if (!strcmp ((char *)name, "feDistantLight"))
		newnode = rsvg_new_filter_primitive_light_source('d');
	else if (!strcmp ((char *)name, "feSpotLight"))
		newnode = rsvg_new_filter_primitive_light_source('s');
	else if (!strcmp ((char *)name, "fePointLight"))
		newnode = rsvg_new_filter_primitive_light_source('p');

	/* hack to make multiImage sort-of work */
	else if (!strcmp ((char *)name, "multiImage"))
		newnode = rsvg_new_switch();
	else if (!strcmp ((char *)name, "subImageRef"))
		newnode = rsvg_new_image();
	else if (!strcmp ((char *)name, "subImage"))
		newnode = rsvg_new_group();
	else if (!strcmp ((char *)name, "text"))
		newnode = rsvg_new_text();
	else if (!strcmp ((char *)name, "tspan"))
		newnode = rsvg_new_tspan();
	else if (!strcmp ((char *)name, "tref"))
		newnode = rsvg_new_tref();
	if (newnode)
		{
			newnode->type = g_string_new((char *)name);
			rsvg_node_set_atts(newnode, ctx, atts);
			rsvg_defs_register_memory(ctx->priv->defs, newnode);
			if (ctx->priv->currentnode) {
				rsvg_node_group_pack(ctx->priv->currentnode, newnode);
				ctx->priv->currentnode = newnode;
			}
			else if (!strcmp ((char *)name, "svg")) {
				newnode->parent = NULL;
				ctx->priv->treebase = newnode;
				ctx->priv->currentnode = newnode;
			}
		}
}

/* start desc */

static void
rsvg_desc_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_desc_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;

	char * string = NULL;
	char * utf8 = NULL;

	/* This isn't quite the correct behavior - in theory, any graphics
	   element may contain a title or desc element */

	if (!ch || !len)
		return;

	string = g_strndup ((char*)ch, len);
	if (!g_utf8_validate (string, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = utf8;
		}

	g_string_append (ctx->priv->desc, string);
	g_free (string);
}

static void
rsvg_desc_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
}

static void
rsvg_desc_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "desc"))
		{
			if (ctx->priv->handler != NULL)
				{
					ctx->priv->handler->free (ctx->priv->handler);
					ctx->priv->handler = NULL;
				}
		}
}

static void
rsvg_start_desc (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerDesc *handler = g_new0 (RsvgSaxHandlerDesc, 1);
	
	handler->super.free = rsvg_desc_handler_free;
	handler->super.characters = rsvg_desc_handler_characters;
	handler->super.start_element = rsvg_desc_handler_start;
	handler->super.end_element   = rsvg_desc_handler_end;
	handler->ctx = ctx;

	ctx->priv->desc = g_string_new (NULL);
	ctx->priv->handler = &handler->super;
}

/* end desc */

/* start title */

static void
rsvg_title_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_title_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;

	char * string = NULL;
	char * utf8 = NULL;

	/* This isn't quite the correct behavior - in theory, any graphics
	   element may contain a title or desc element */

	if (!ch || !len)
		return;

	string = g_strndup ((char*)ch, len);
	if (!g_utf8_validate (string, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = utf8;
		}

	g_string_append (ctx->priv->title, string);
	g_free (string);
}

static void
rsvg_title_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
}

static void
rsvg_title_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerTitle *z = (RsvgSaxHandlerTitle *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "title"))
		{
			if (ctx->priv->handler != NULL)
				{
					ctx->priv->handler->free (ctx->priv->handler);
					ctx->priv->handler = NULL;
				}
		}
}

static void
rsvg_start_title (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerTitle *handler = g_new0 (RsvgSaxHandlerTitle, 1);
	
	handler->super.free = rsvg_title_handler_free;
	handler->super.characters = rsvg_title_handler_characters;
	handler->super.start_element = rsvg_title_handler_start;
	handler->super.end_element   = rsvg_title_handler_end;
	handler->ctx = ctx;

	ctx->priv->title = g_string_new (NULL);
	ctx->priv->handler = &handler->super;
}

/* end title */

/* start metadata */

static void
rsvg_metadata_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_metadata_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerDesc *z = (RsvgSaxHandlerDesc *)self;
	RsvgHandle *ctx = z->ctx;

	char * string = NULL;
	char * utf8 = NULL;

	/* This isn't quite the correct behavior - in theory, any graphics
	   element may contain a metadata or desc element */

	if (!ch || !len)
		return;

	string = g_strndup ((char*)ch, len);
	if (!g_utf8_validate (string, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = utf8;
		}

	g_string_append (ctx->priv->metadata, string);
	g_free (string);
}

static void
rsvg_metadata_props_enumerate (const char * key,
							   const char * value,
							   gpointer user_data)
{
	GString * metadata = (GString *)user_data;
	g_string_append_printf (metadata, "%s=\"%s\" ", key, value);
}

static void
rsvg_metadata_handler_start (RsvgSaxHandler *self, const xmlChar *name,
							 RsvgPropertyBag *atts)
{
	RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *)self;
	RsvgHandle *ctx = z->ctx;

	g_string_append_printf (ctx->priv->metadata, "<%s ", name);
	rsvg_property_bag_enumerate (atts, rsvg_metadata_props_enumerate, ctx->priv->metadata);
	g_string_append (ctx->priv->metadata, ">\n");
}

static void
rsvg_metadata_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "metadata"))
		{
			if (ctx->priv->handler != NULL)
				{
					ctx->priv->handler->free (ctx->priv->handler);
					ctx->priv->handler = NULL;
				}
		}
	else
		g_string_append_printf (ctx->priv->metadata, "</%s>\n", name);
}

static void
rsvg_start_metadata (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgSaxHandlerMetadata *handler = g_new0 (RsvgSaxHandlerMetadata, 1);
	
	handler->super.free = rsvg_metadata_handler_free;
	handler->super.characters = rsvg_metadata_handler_characters;
	handler->super.start_element = rsvg_metadata_handler_start;
	handler->super.end_element   = rsvg_metadata_handler_end;
	handler->ctx = ctx;

	ctx->priv->metadata = g_string_new (NULL);
	ctx->priv->handler = &handler->super;
}

/* end metadata */

static void
rsvg_start_element (void *data, const xmlChar *name,
					const xmlChar ** atts)
{
	RsvgHandle *ctx = (RsvgHandle *)data;

	RsvgPropertyBag * bag;

	bag = rsvg_property_bag_new(atts);

	if (ctx->priv->handler)
		{
			ctx->priv->handler_nest++;
			if (ctx->priv->handler->start_element != NULL)
				ctx->priv->handler->start_element (ctx->priv->handler, name, bag);
		}
	else
		{
			const xmlChar * tempname;
			for (tempname = name; *tempname != '\0'; tempname++)
				if (*tempname == ':')
					name = tempname + 1;
			
			if (!strcmp ((char *)name, "style"))
				rsvg_start_style (ctx, bag);
			else if (!strcmp ((char *)name, "title"))
				rsvg_start_title (ctx, bag);
			else if (!strcmp ((char *)name, "desc"))
				rsvg_start_desc (ctx, bag);
			else if (!strcmp ((char *)name, "metadata"))
				rsvg_start_metadata (ctx, bag);
			rsvg_standard_element_start (ctx, name, bag);
    }

	rsvg_property_bag_free(bag);
}

static void
rsvg_end_element (void *data, const xmlChar *name)
{
	RsvgHandle *ctx = (RsvgHandle *)data;

	if (ctx->priv->handler_nest > 0 && ctx->priv->handler != NULL)
		{
			if (ctx->priv->handler->end_element != NULL)
				ctx->priv->handler->end_element (ctx->priv->handler, name);
			ctx->priv->handler_nest--;
		}
	else
		{
			const xmlChar * tempname;
			for (tempname = name; *tempname != '\0'; tempname++)
				if (*tempname == ':')
					name = tempname + 1;
			if (ctx->priv->handler != NULL)
				{
					ctx->priv->handler->free (ctx->priv->handler);
					ctx->priv->handler = NULL;
				}

			if (ctx->priv->currentnode && !strcmp ((char *)name, ctx->priv->currentnode->type->str))
				rsvg_pop_def_group(ctx);
			
		}
}

static void _rsvg_node_chars_free(RsvgNode * node)
{
	RsvgNodeChars * self = (RsvgNodeChars *)node;
	g_string_free(self->contents, TRUE);
	_rsvg_node_free(node);
}

static void
rsvg_characters (void *data, const xmlChar *ch, int len)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	char * utf8 = NULL;
	RsvgNodeChars * self;
	GString * string;
	
	if (ctx->priv->handler && ctx->priv->handler->characters != NULL)
		{
			ctx->priv->handler->characters (ctx->priv->handler, ch, len);
			return;
		}

	if (!ch || !len)
		return;

	string = g_string_new_len ((char*)ch, len);
	if (!g_utf8_validate (string->str, -1, NULL))
		{
			utf8 = rsvg_make_valid_utf8 (string->str);
			g_string_free (string, TRUE);
			string = g_string_new ((char*)ch);
		}

	self = g_new(RsvgNodeChars, 1);
	_rsvg_node_init(&self->super);
	self->contents = string;

	self->super.type = g_string_new("RSVG_NODE_CHARS");
	self->super.free = _rsvg_node_chars_free;
	self->super.state->cond_true = FALSE;

	rsvg_defs_register_memory(ctx->priv->defs, (RsvgNode *)self);
	if (ctx->priv->currentnode)
		rsvg_node_group_pack(ctx->priv->currentnode, (RsvgNode *)self);
}

#if LIBXML_VERSION >= 20621
#define RSVG_ENABLE_ENTITIES
#elif defined(__GNUC__)
#warning "libxml version less than 2.6.22. XML entities won't work"
#endif

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar *name)
{
#ifdef RSVG_ENABLE_ENTITIES
	RsvgHandle *ctx = (RsvgHandle *)data;
	xmlEntityPtr entity;

	entity = g_hash_table_lookup (ctx->priv->entities, name);

	return entity;
#else
	return NULL;
#endif
}

static void
rsvg_entity_decl (void *data, const xmlChar *name, int type,
				  const xmlChar *publicId, const xmlChar *systemId, xmlChar *content)
{
#ifdef RSVG_ENABLE_ENTITIES
	RsvgHandle *ctx = (RsvgHandle *)data;
	GHashTable *entities = ctx->priv->entities;
	xmlEntityPtr entity;
	xmlChar *dupname;

	entity = g_new0 (xmlEntity, 1);
	entity->type = XML_ENTITY_DECL;
	dupname = (xmlChar *) g_strdup ((char *)name);
	entity->name = dupname;
	entity->ExternalID = (xmlChar *) g_strdup ((char *)publicId);
	entity->SystemID = (xmlChar *) g_strdup ((char *)systemId);
	entity->etype = type;
	if (content)
		{
			entity->content = (xmlChar *) xmlMemStrdup ((char *)content);
			entity->length = strlen ((char *)content);
		}
	g_hash_table_insert (entities, dupname, entity);
#endif
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

/* TODO: this is indempotent, but not exactly threadsafe */
static xmlSAXHandler rsvgSAXHandlerStruct;
static gboolean rsvgSAXHandlerStructInited = FALSE;

void rsvg_SAX_handler_struct_init (void)
{
	if(!rsvgSAXHandlerStructInited) 
		{
			rsvgSAXHandlerStructInited = TRUE;

			memset(&rsvgSAXHandlerStruct, 0, sizeof(rsvgSAXHandlerStruct));

			rsvgSAXHandlerStruct.getEntity = rsvg_get_entity;
			rsvgSAXHandlerStruct.entityDecl = rsvg_entity_decl;
			rsvgSAXHandlerStruct.characters = rsvg_characters;
			rsvgSAXHandlerStruct.error = rsvg_error_cb;
			rsvgSAXHandlerStruct.cdataBlock = rsvg_characters;
			rsvgSAXHandlerStruct.startElement = rsvg_start_element;
			rsvgSAXHandlerStruct.endElement = rsvg_end_element;
		}
}

gchar *
rsvg_get_base_uri_from_filename(const gchar * file_name)
{
	gchar *curdir;
	gchar *reldir;
	gchar *base_uri;

	reldir = g_path_get_dirname (file_name);

	if (g_path_is_absolute (file_name))
		return reldir;
	
	curdir = g_get_current_dir();
	base_uri = g_build_filename (curdir, reldir, NULL);
	g_free (curdir);
	g_free (reldir);

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
void rsvg_handle_set_base_uri (RsvgHandle *handle,
							   const char *base_uri)
{
	g_return_if_fail(handle);

	if (base_uri) {
		if (handle->priv->base_uri)
			g_free (handle->priv->base_uri);
		handle->priv->base_uri = g_strdup (base_uri);
		rsvg_defs_set_base_uri(handle->priv->defs, handle->priv->base_uri);
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
G_CONST_RETURN char *rsvg_handle_get_base_uri (RsvgHandle *handle)
{
	g_return_val_if_fail(handle, NULL);
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
	static GQuark q = 0;
	if (q == 0)
		q = g_quark_from_static_string ("rsvg-error-quark");
	
	return q;
}

static gboolean
rsvg_handle_write_impl (RsvgHandle    *handle,
						const guchar  *buf,
						gsize          count,
						GError       **error)
{
	GError *real_error = NULL;
	int result;

	g_return_val_if_fail (handle != NULL, FALSE);
	
	handle->priv->error = &real_error;
	if (handle->priv->ctxt == NULL)
		{
			handle->priv->ctxt = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0, NULL);
			handle->priv->ctxt->replaceEntities = TRUE;
		}
	
	result = xmlParseChunk (handle->priv->ctxt, (char*)buf, count, 0);
	if (result != 0) {
		g_set_error (error, rsvg_error_quark (), 0,
					 _("Error parsing XML data"));
		return FALSE;
	}

	handle->priv->error = NULL;

	if (real_error != NULL)
		{
			g_propagate_error (error, real_error);
			return FALSE;
		}

	return TRUE;
}

static gboolean
rsvg_handle_close_impl (RsvgHandle  *handle,
						GError     **error)
{
	GError *real_error = NULL;
	
	handle->priv->error = &real_error;
	
	if (handle->priv->ctxt != NULL)
		{
			xmlDocPtr xmlDoc;
			int result;

			xmlDoc = handle->priv->ctxt->myDoc;

			result = xmlParseChunk (handle->priv->ctxt, "", 0, TRUE);
			xmlFreeParserCtxt (handle->priv->ctxt);
			xmlFreeDoc(xmlDoc);

			if (result != 0) {
				g_set_error (error, rsvg_error_quark (), 0,
							 _("Error parsing XML data"));
				return FALSE;
			}
		}
  
	rsvg_defs_resolve_all(handle->priv->defs);
	handle->priv->finished = TRUE;
	handle->priv->error = NULL;

	if (real_error != NULL)
		{
			g_propagate_error (error, real_error);
			return FALSE;
		}

	return TRUE;
}

static void
rsvg_state_free_func(gpointer data, gpointer user_data)
{
	RsvgDrawingCtx * ctx = (RsvgDrawingCtx *)user_data;
	rsvg_state_finalize((RsvgState *)data);
	g_mem_chunk_free(ctx->state_allocator, data);
}

void
rsvg_drawing_ctx_free (RsvgDrawingCtx *handle)
{
	rsvg_render_free (handle->render);
	
	g_slist_foreach(handle->state, rsvg_state_free_func, (gpointer)handle);
	g_slist_free (handle->state);

	if (handle->base_uri)
		g_free (handle->base_uri);

	g_mem_chunk_destroy(handle->state_allocator);

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
G_CONST_RETURN char *rsvg_handle_get_metadata (RsvgHandle *handle)
{
	g_return_val_if_fail(handle, NULL);

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
G_CONST_RETURN char *rsvg_handle_get_title (RsvgHandle *handle)
{
	g_return_val_if_fail(handle, NULL);

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
G_CONST_RETURN char *rsvg_handle_get_desc (RsvgHandle *handle)
{
	g_return_val_if_fail(handle, NULL);

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
rsvg_bbox_render_path (RsvgDrawingCtx *ctx, const RsvgBpathDef *bpath_def)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgBpath *bpath;
	RsvgBboxRender *render = (RsvgBboxRender *)ctx->render;
	RsvgBbox bbox;
	int i;

	rsvg_bbox_init(&bbox, state->affine);
	bbox.w = bbox.h = bbox.virgin = 0;

	for (i=0; i < bpath_def->n_bpath; i++) {
		bpath = &bpath_def->bpath[i];

		switch (bpath->code) {
		case RSVG_MOVETO:
		case RSVG_MOVETO_OPEN:
		case RSVG_CURVETO:
		case RSVG_LINETO:
			bbox.x = bpath->x3;
			bbox.y = bpath->y3;
			rsvg_bbox_insert(&render->bbox, &bbox);
			break;
		default:
			break;
		}
	}
}

static void rsvg_bbox_render_image (RsvgDrawingCtx *ctx, 
										  const GdkPixbuf * pixbuf, 
										  double pixbuf_x, double pixbuf_y, 
										  double w, double h)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgBboxRender *render = (RsvgBboxRender *)ctx->render;
	RsvgBbox bbox;

	rsvg_bbox_init(&bbox, state->affine);
	bbox.x = pixbuf_x;
	bbox.y = pixbuf_y;
	bbox.w = w;
	bbox.h = h;
	bbox.virgin = 0;

	rsvg_bbox_insert(&render->bbox, &bbox);	
}


static void
rsvg_bbox_render_free (RsvgRender * self)
{
	g_free (self);
}

static void
rsvg_bbox_push_discrete_layer (RsvgDrawingCtx *ctx) {}

static void
rsvg_bbox_pop_discrete_layer (RsvgDrawingCtx *ctx) {}

static void 
rsvg_bbox_add_clipping_rect (RsvgDrawingCtx *ctx,
								   double x, double y,
								   double w, double h){}

static RsvgBboxRender * 
rsvg_bbox_render_new()
{
	RsvgBboxRender * render = g_new0(RsvgBboxRender, 1);
	double affine[6];

	render->super.free                 = rsvg_bbox_render_free;
	render->super.render_image         = rsvg_bbox_render_image;
	render->super.render_path          = rsvg_bbox_render_path;
	render->super.pop_discrete_layer   = 
		rsvg_bbox_pop_discrete_layer;
	render->super.push_discrete_layer  = 
		rsvg_bbox_push_discrete_layer;
	render->super.add_clipping_rect    = 
		rsvg_bbox_add_clipping_rect;
	render->super.get_image_of_node    = NULL;
	_rsvg_affine_identity(affine);
	rsvg_bbox_init(&render->bbox, affine);

	return render;
}

static RsvgBbox
_rsvg_find_bbox (RsvgHandle *handle)
{
	RsvgDrawingCtx * ctx = g_new(RsvgDrawingCtx, 1);
	RsvgBbox output;
	RsvgBboxRender * render = rsvg_bbox_render_new();
	ctx->drawsub_stack = NULL;
	ctx->render = (RsvgRender *)render;
	
	ctx->state = NULL;

	ctx->state_allocator = g_mem_chunk_create (RsvgState, 256, G_ALLOC_AND_FREE);

	ctx->defs = handle->priv->defs;
	ctx->base_uri = g_strdup(handle->priv->base_uri);
	ctx->dpi_x = handle->priv->dpi_x;
	ctx->dpi_y = handle->priv->dpi_y;
	ctx->vb.w = 512;
	ctx->vb.h = 512;
	ctx->pango_context = NULL;

	rsvg_state_push(ctx);
	_rsvg_affine_identity(rsvg_state_current(ctx)->affine);
	_rsvg_node_draw_children ((RsvgNode *)handle->priv->treebase, ctx, 0);
	rsvg_state_pop(ctx);

	output = render->bbox;
	rsvg_render_free(ctx->render);
	g_free(ctx);
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
rsvg_handle_get_dimensions(RsvgHandle * handle, RsvgDimensionData * dimension_data)
{
	RsvgNodeSvg * sself;
	RsvgBbox bbox;

	g_return_if_fail(dimension_data);
	memset(dimension_data, 0, sizeof(RsvgDimensionData));
	g_return_if_fail(handle);

	sself = (RsvgNodeSvg *)handle->priv->treebase;	
	if(!sself)
		return;
	
	bbox.x = bbox.y = 0;
	bbox.w = bbox.h = 1;
	
	if (sself->w.factor == 'p' || sself->h.factor == 'p')
		{
			if (sself->vbox.active && sself->vbox.w > 0. && sself->vbox.h > 0.)
				{
					bbox.w = sself->vbox.w;
					bbox.h = sself->vbox.h;
				}
			else
				bbox = _rsvg_find_bbox(handle);
		}
	dimension_data->width  = _rsvg_css_hand_normalize_length(&sself->w, handle->priv->dpi_x, 
																	bbox.w + bbox.x * 2, 12);
	dimension_data->height = _rsvg_css_hand_normalize_length(&sself->h, handle->priv->dpi_y, 
													 bbox.h + bbox.y * 2, 12);
	
	dimension_data->em = dimension_data->width;
	dimension_data->ex = dimension_data->height;

	if (handle->priv->size_func)
		(* handle->priv->size_func) (&dimension_data->width, &dimension_data->height, handle->priv->user_data);
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
rsvg_handle_set_size_callback (RsvgHandle     *handle,
							   RsvgSizeFunc    size_func,
							   gpointer        user_data,
							   GDestroyNotify  user_data_destroy)
{
	g_return_if_fail (handle != NULL);
	
	if (handle->priv->user_data_destroy)
		(* handle->priv->user_data_destroy) (handle->priv->user_data);
	
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
rsvg_handle_write (RsvgHandle    *handle,
				   const guchar  *buf,
				   gsize          count,
				   GError       **error)
{
	g_return_val_if_fail(handle, FALSE);

	if (handle->priv->first_write) {
		handle->priv->first_write = FALSE;

		/* test for GZ marker. todo: store the first 2 bytes in the odd circumstance that someone calls
		 * write() in 1 byte increments */
		if ((count >= 2) && (buf[0] == (guchar)0x1f) && (buf[1] == (guchar)0x8b)) {
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
 * freed until @rsvg_handle_free is called.
 *
 * Returns: #TRUE if the loader closed successfully, or #FALSE if there was
 * an error.
 **/
gboolean
rsvg_handle_close (RsvgHandle  *handle,
				   GError     **error)
{
	g_return_val_if_fail(handle, FALSE);

#if HAVE_SVGZ
	if (handle->priv->is_gzipped) {
		GsfInput * gzip;
		const guchar * bytes;
		gsize size;
		gsize remaining;
		
		bytes = gsf_output_memory_get_bytes (GSF_OUTPUT_MEMORY (handle->priv->gzipped_data));
		size = gsf_output_size (handle->priv->gzipped_data);

		gzip = GSF_INPUT (gsf_input_gzip_new (GSF_INPUT (gsf_input_memory_new (bytes, size, FALSE)), error));
		remaining = gsf_input_remaining (gzip);
		while ((size = MIN (remaining, 1024)) > 0) {
			guint8 const *buf;
			
			/* write to parent */
			buf = gsf_input_read (gzip, size, NULL);
			if (!buf)
				{
					/* an error occured, so bail */
					g_warning (_("rsvg_gz_handle_close_impl: gsf_input_read returned NULL"));
					break;
				}
			
			rsvg_handle_write_impl (handle,
									buf,
									size, error);
			/* if we didn't manage to lower remaining number of bytes,
			 * something is wrong, and we should avoid an endless loop */
			if (remaining == ((gsize) gsf_input_remaining (gzip)))
				{
					g_warning (_("rsvg_gz_handle_close_impl: write_impl didn't lower the input_remaining count"));
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

#ifdef HAVE_GNOME_VFS
	gnome_vfs_init();
#endif
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
rsvg_node_set_atts(RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
	node->set_atts(node, ctx, atts);
}

void 
rsvg_pop_discrete_layer(RsvgDrawingCtx *ctx)
{
	ctx->render->pop_discrete_layer(ctx);
}

void 
rsvg_push_discrete_layer (RsvgDrawingCtx *ctx)
{
	ctx->render->push_discrete_layer(ctx);
}

void 
rsvg_render_path (RsvgDrawingCtx *ctx, const char *d)
{
	/* todo: store and use the bpath higher up */
	RsvgBpathDef * bpath_def;

	bpath_def = rsvg_parse_path (d);
	rsvg_bpath_def_art_finish (bpath_def);

	ctx->render->render_path(ctx, bpath_def);
	rsvg_render_markers(bpath_def, ctx);

	rsvg_bpath_def_free (bpath_def);
}

void 
rsvg_render_image (RsvgDrawingCtx *ctx, GdkPixbuf * pb, 
						double x, double y, double w, double h)
{
	ctx->render->render_image(ctx, pb, x, y, w, h);
}

void 
rsvg_add_clipping_rect (RsvgDrawingCtx *ctx, double x, double y, double w, double h)
{
	ctx->render->add_clipping_rect(ctx, x, y, w, h);
}

GdkPixbuf * rsvg_get_image_of_node(RsvgDrawingCtx *ctx, RsvgNode * drawable,
							  double w, double h)
{
	return ctx->render->get_image_of_node(ctx, drawable, w, h);
}

void 
rsvg_render_free (RsvgRender * render)
{
	render->free (render);
}

void rsvg_bbox_init(RsvgBbox * self, double * affine)
{
	int i;
	self->virgin = 1;
	for (i = 0; i < 6; i++)
		self->affine[i] = affine[i];
}

void rsvg_bbox_insert(RsvgBbox * dst, RsvgBbox * src)
{
	double affine[6];
	double xmin = dst->x, ymin = dst->y;
	double xmax = dst->x + dst->w, ymax = dst->y + dst->h;
	int i;

	if (src->virgin)
		return;
	_rsvg_affine_invert(affine, dst->affine);
	_rsvg_affine_multiply(affine, src->affine, affine);

	for (i = 0; i < 4; i++)
		{
			double rx, ry, x, y;
			rx = src->x + src->w * (double)(i % 2);
			ry = src->y + src->h * (double)(i / 2);
			x = affine[0] * rx + affine[2] * ry + affine[4];
			y = affine[1] * rx + affine[3] * ry + affine[5];
			if (dst->virgin)
				{
					xmin = xmax = x;
					ymin = ymax = y;
					dst->virgin = 0;
				}
			else
				{
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

void rsvg_bbox_clip(RsvgBbox * dst, RsvgBbox * src)
{
	double affine[6];
	double xmin = dst->x + dst->w, ymin = dst->y + dst->h;
	double xmax = dst->x, ymax = dst->y;
	int i;

	if (src->virgin)
		return;
	_rsvg_affine_invert(affine, dst->affine);
	_rsvg_affine_multiply(affine, src->affine, affine);

	for (i = 0; i < 4; i++)
		{
			double rx, ry, x, y;
			rx = src->x + src->w * (double)(i % 2);
			ry = src->y + src->h * (double)(i / 2);
			x = affine[0] * rx + affine[2] * ry + affine[4];
			y = affine[1] * rx + affine[3] * ry + affine[5];
			if (dst->virgin)
				{
					xmin = xmax = x;
					ymin = ymax = y;
					dst->virgin = 0;
				}
			else
				{
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

void _rsvg_push_view_box(RsvgDrawingCtx *ctx, double w, double h)
{
	RsvgViewBox * vb = g_new(RsvgViewBox, 1);
	*vb = ctx->vb;
	ctx->vb_stack = g_slist_prepend(ctx->vb_stack, vb);
	ctx->vb.w = w;
	ctx->vb.h = h;
}

void _rsvg_pop_view_box(RsvgDrawingCtx *ctx)
{
	ctx->vb = *((RsvgViewBox *)ctx->vb_stack->data);
	g_free(ctx->vb_stack->data);
	ctx->vb_stack = g_slist_delete_link(ctx->vb_stack, ctx->vb_stack);
}
