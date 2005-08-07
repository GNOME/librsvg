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
static double internal_dpi_x = RSVG_DEFAULT_DPI_X;
static double internal_dpi_y = RSVG_DEFAULT_DPI_Y;

void
rsvg_drawing_ctx_init(RsvgDrawingCtx * handle);

static void
rsvg_ctx_free_helper (gpointer key, gpointer value, gpointer user_data)
{
	xmlEntityPtr entval = (xmlEntityPtr)value;
	
	/* key == entval->name, so it's implicitly freed below */
	
	g_free ((char *) entval->name);
	g_free ((char *) entval->ExternalID);
	g_free ((char *) entval->SystemID);
	xmlFree (entval->content);
	xmlFree (entval->orig);
	g_free (entval);
}

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
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = prev;
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
	
	handler->parent = (RsvgSaxHandlerDefs*)ctx->handler;
	ctx->handler = &handler->super;
}


static void
rsvg_filter_handler_start (RsvgHandle *ctx, const xmlChar *name,
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
	if (newnode)
		{
			rsvg_node_set_atts(newnode, ctx, atts);
			rsvg_defs_register_memory(ctx->defs, newnode);
			if (ctx->currentnode) {
				rsvg_node_group_pack(ctx->currentnode, newnode);
				ctx->currentnode = newnode;
			}
			else if (!strcmp ((char *)name, "svg")) {
				newnode->parent = NULL;
				ctx->treebase = newnode;
				ctx->currentnode = newnode;
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

	g_string_append (ctx->desc, string);
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
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
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

	ctx->desc = g_string_new (NULL);
	ctx->handler = &handler->super;
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

	g_string_append (ctx->title, string);
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
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
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

	ctx->title = g_string_new (NULL);
	ctx->handler = &handler->super;
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

	g_string_append (ctx->metadata, string);
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

	g_string_append_printf (ctx->metadata, "<%s ", name);
	rsvg_property_bag_enumerate (atts, rsvg_metadata_props_enumerate, ctx->metadata);
	g_string_append (ctx->metadata, ">\n");
}

static void
rsvg_metadata_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerMetadata *z = (RsvgSaxHandlerMetadata *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp((char *)name, "metadata"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}
		}
	else
		g_string_append_printf (ctx->metadata, "</%s>\n", name);
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

	ctx->metadata = g_string_new (NULL);
	ctx->handler = &handler->super;
}

/* end metadata */

static void
rsvg_start_element (void *data, const xmlChar *name,
					const xmlChar ** atts)
{
	RsvgHandle *ctx = (RsvgHandle *)data;

	RsvgPropertyBag * bag;
  
	RsvgDimensionData * newdimension;
	newdimension = g_new(RsvgDimensionData, 1);
	newdimension->width = ctx->width;
	newdimension->height = ctx->height;
	newdimension->em = rsvg_state_current_font_size(ctx);
	ctx->dimensions = g_slist_prepend(ctx->dimensions, newdimension);

	bag = rsvg_property_bag_new(atts);

	if (ctx->handler)
		{
			ctx->handler_nest++;
			if (ctx->handler->start_element != NULL)
				ctx->handler->start_element (ctx->handler, name, bag);
		}
	else
		{
			const xmlChar * tempname;
			for (tempname = name; *tempname != '\0'; tempname++)
				if (*tempname == ':')
					name = tempname + 1;
			
			if (!strcmp ((char *)name, "text"))
				rsvg_start_text (ctx, bag);
			else if (!strcmp ((char *)name, "style"))
				rsvg_start_style (ctx, bag);
			else if (!strcmp ((char *)name, "title"))
				rsvg_start_title (ctx, bag);
			else if (!strcmp ((char *)name, "desc"))
				rsvg_start_desc (ctx, bag);
			else if (!strcmp ((char *)name, "metadata"))
				rsvg_start_metadata (ctx, bag);
			rsvg_filter_handler_start (ctx, name, bag);
    }

	rsvg_property_bag_free(bag);
}

static void
rsvg_end_element (void *data, const xmlChar *name)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	GSList * link = g_slist_nth(ctx->dimensions, 0);
	RsvgDimensionData * dead_dimension = (RsvgDimensionData *)link->data;
	ctx->width = dead_dimension->width;
	ctx->height = dead_dimension->height;
	g_free (dead_dimension);
	ctx->dimensions = g_slist_delete_link(ctx->dimensions, link);

	if (ctx->handler_nest > 0 && ctx->handler != NULL)
		{
			if (ctx->handler->end_element != NULL)
				ctx->handler->end_element (ctx->handler, name);
			ctx->handler_nest--;
		}
	else
		{
			const xmlChar * tempname;
			for (tempname = name; *tempname != '\0'; tempname++)
				if (*tempname == ':')
					name = tempname + 1;
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = NULL;
				}

			if (!strcmp ((char *)name, "image") ||
				!strcmp ((char *)name, "use") ||
				!strcmp ((char *)name, "switch") ||
				!strcmp ((char *)name, "marker") ||
				!strcmp ((char *)name, "clipPath") ||
				!strcmp ((char *)name, "mask") ||
				!strcmp ((char *)name, "defs") ||
				!strcmp ((char *)name, "filter") ||
				!strcmp ((char *)name, "symbol") ||
				!strcmp ((char *)name, "svg") ||
				!strcmp ((char *)name, "a") ||
				!strcmp ((char *)name, "line") ||
				!strcmp ((char *)name, "rect") ||
				!strcmp ((char *)name, "circle") ||
				!strcmp ((char *)name, "ellipse") ||
				!strcmp ((char *)name, "polyline") ||
				!strcmp ((char *)name, "polygon") ||
				!strcmp ((char *)name, "path") ||
				!strcmp ((char *)name, "g") ||
				!strcmp ((char *)name, "pattern") ||
				!strcmp ((char *)name, "linearGradient") ||
				!strcmp ((char *)name, "radialGradient") ||
				!strcmp ((char *)name, "conicalGradient") ||
				!strcmp ((char *)name, "stop") ||
				!strncmp ((char *)name, "fe", 2))
				{
					/*when type enums are working right we should test if the end is the same as the current node*/
					rsvg_pop_def_group(ctx);
				}
			
		}
}

#if 0
static void _rsvg_node_chars_free(RsvgNode * node)
{
	RsvgNodeChars * self = (RsvgNodeChars *)node;
	g_string_free(self->contents, TRUE);
	_rsvg_node_free(node);
}
#endif

static void
rsvg_characters (void *data, const xmlChar *ch, int len)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	if (ctx->handler && ctx->handler->characters != NULL)
		{
			ctx->handler->characters (ctx->handler, ch, len);
			return;
		}

#if 0
	char * utf8 = NULL;
	RsvgNodeChars * self;
	GString * string;

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

	self->super.type = RSVG_NODE_CHARS;
	self->super.free = _rsvg_node_chars_free;

	rsvg_defs_register_memory(ctx->defs, (RsvgNode *)self);
	if (ctx->currentnode)
		rsvg_node_group_pack(ctx->currentnode, (RsvgNode *)self);
#endif
}

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar *name)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	
	return (xmlEntityPtr)g_hash_table_lookup (ctx->entities, name);
}

static void
rsvg_entity_decl (void *data, const xmlChar *name, int type,
				  const xmlChar *publicId, const xmlChar *systemId, xmlChar *content)
{
	RsvgHandle *ctx = (RsvgHandle *)data;
	GHashTable *entities = ctx->entities;
	xmlEntityPtr entity;
	char *dupname;

	entity = g_new0 (xmlEntity, 1);
	entity->type = type;
	entity->length = strlen ((char*)name);
	dupname = g_strdup ((char*)name);
	entity->name = (xmlChar*)dupname;
	entity->ExternalID = (xmlChar*)g_strdup ((char*)publicId);
	entity->SystemID = (xmlChar*)g_strdup ((char*)systemId);
	if (content)
		{
			entity->content = (xmlChar*)xmlMemStrdup ((char*)content);
			entity->length = strlen ((char*)content);
		}
	g_hash_table_insert (entities, dupname, entity);
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

static void rsvg_SAX_handler_struct_init()
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
 * Set the base URI for this SVG
 *
 * @handle: A #RsvgHandle
 * @base_uri: 
 *
 * Since: 2.9 (really present in 2.8 as well)
 */
void rsvg_handle_set_base_uri (RsvgHandle *handle,
							   const char *base_uri)
{
	if (base_uri) {
		if (handle->base_uri)
			g_free (handle->base_uri);
		handle->base_uri = g_strdup (base_uri);
		rsvg_defs_set_base_uri(handle->defs, handle->base_uri);
	}
}

/**
 * Gets the base uri for this RsvgHandle.
 * @handle: A #RsvgHandle
 *
 * Returns: the base uri, possibly null
 * Since: 2.9 (really present in 2.8 as well)
 */
G_CONST_RETURN char *rsvg_handle_get_base_uri (RsvgHandle *handle)
{
	return handle->base_uri;
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
	GError *real_error;
	g_return_val_if_fail (handle != NULL, FALSE);
	
	handle->error = &real_error;
	if (handle->ctxt == NULL)
		{
			handle->ctxt = xmlCreatePushParserCtxt (&rsvgSAXHandlerStruct, handle, NULL, 0, NULL);
			handle->ctxt->replaceEntities = TRUE;
		}
	
	xmlParseChunk (handle->ctxt, (char*)buf, count, 0);
	
	handle->error = NULL;
	/* FIXME: Error handling not implemented. */
	/*  if (*real_error != NULL)
		{
		g_propagate_error (error, real_error);
		return FALSE;
		}*/
  return TRUE;
}

static gboolean
rsvg_handle_close_impl (RsvgHandle  *handle,
						GError     **error)
{
	GError *real_error;
	
	handle->error = &real_error;
	
	if (handle->ctxt != NULL)
		{
			xmlDocPtr xmlDoc;

			xmlDoc = handle->ctxt->myDoc;

			xmlParseChunk (handle->ctxt, "", 0, TRUE);
			xmlFreeParserCtxt (handle->ctxt);
			xmlFreeDoc(xmlDoc);
		}
  
	/* FIXME: Error handling not implemented. */
	/*
	  if (real_error != NULL)
	  {
      g_propagate_error (error, real_error);
      return FALSE;
      }*/
	rsvg_defs_resolve_all(handle->defs);

	handle->finished = TRUE;

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

static void
rsvg_handle_free_impl (RsvgHandle *handle)
{
	g_hash_table_foreach (handle->entities, rsvg_ctx_free_helper, NULL);
	g_hash_table_destroy (handle->entities);
	rsvg_defs_free (handle->defs);
	g_hash_table_destroy (handle->css_props);
	
	if (handle->user_data_destroy)
		(* handle->user_data_destroy) (handle->user_data);

	if (handle->title)
		g_string_free (handle->title, TRUE);
	if (handle->desc)
		g_string_free (handle->desc, TRUE);
	if (handle->metadata)
		g_string_free (handle->metadata, TRUE);
	if (handle->base_uri)
		g_free (handle->base_uri);

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
	if (handle->metadata)
		return handle->metadata->str;
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
	if (handle->title)
		return handle->title->str;
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
	if (handle->desc)
		return handle->desc->str;
	else
		return NULL;
}

/**
 * rsvg_handle_new:
 *
 * Returns a new rsvg handle.  Must be freed with @rsvg_handle_free.  This
 * handle can be used for dynamically loading an image.  You need to feed it
 * data using @rsvg_handle_write, then call @rsvg_handle_close when done.  No
 * more than one image can be loaded with one handle.
 *
 * Returns: A new #RsvgHandle
 **/
RsvgHandle *
rsvg_handle_new (void)
{
	RsvgHandle *handle;
	
	handle = g_new0 (RsvgHandle, 1);

	handle->defs = rsvg_defs_new ();
	handle->handler_nest = 0;
	handle->entities = g_hash_table_new (g_str_hash, g_str_equal);
	handle->dpi_x = internal_dpi_x;
	handle->dpi_y = internal_dpi_y;
	
	handle->css_props = g_hash_table_new_full (g_str_hash, g_str_equal,
											   g_free, g_free);
	rsvg_SAX_handler_struct_init();
	
	handle->ctxt = NULL;
	handle->currentnode = NULL;
	handle->treebase = NULL;

	handle->dimensions = NULL;
	handle->finished = 0;
	handle->first_write = TRUE;

	return handle;
}

void
rsvg_handle_get_dimensions(RsvgHandle * handle, RsvgDimensionData * output)
{
	RsvgNodeSvg * sself;

	sself = (RsvgNodeSvg *)handle->treebase;	
	if(!sself) {
		memset(output, 0, sizeof(RsvgDimensionData));
		return;
	}

	if (sself->hasw && sself->hash)
		{
			output->width  = sself->w;
			output->height = sself->h;
		}
	else if (sself->has_vbox && sself->vbw > 0. && sself->vbh > 0.)
		{
			output->width  = (int)floor (sself->vbw);
			output->height = (int)floor (sself->vbh);
		}
	else
		{
			output->width = 512;
			output->height = 512;
		}

	output->em = output->width;
	output->ex = output->height;

	if (handle->size_func) {
		(* handle->size_func) (&output->width, &output->height, handle->user_data);
	}	
}

/** 
 * rsvg_set_default_dpi
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
rsvg_set_default_dpi (double dpi_x, double dpi_y)
{
	if (dpi_x <= 0.)
		internal_dpi_x = RSVG_DEFAULT_DPI_X;
	else
		internal_dpi_x = dpi_x;

	if (dpi_y <= 0.)
		internal_dpi_y = RSVG_DEFAULT_DPI_Y;
	else
		internal_dpi_y = dpi_y;
}

/**
 * rsvg_handle_set_dpi
 * @handle: An #RsvgHandle
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi will 
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi (RsvgHandle * handle, double dpi_x, double dpi_y)
{
	g_return_if_fail (handle != NULL);
	
    if (dpi_x <= 0.)
        handle->dpi_x = internal_dpi_x;
    else
        handle->dpi_x = dpi_x;
	
	if (dpi_y <= 0.)
        handle->dpi_y = internal_dpi_y;
    else
        handle->dpi_y = dpi_y;
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
 **/
void
rsvg_handle_set_size_callback (RsvgHandle     *handle,
							   RsvgSizeFunc    size_func,
							   gpointer        user_data,
							   GDestroyNotify  user_data_destroy)
{
	g_return_if_fail (handle != NULL);
	
	if (handle->user_data_destroy)
		(* handle->user_data_destroy) (handle->user_data);
	
	handle->size_func = size_func;
	handle->user_data = user_data;
	handle->user_data_destroy = user_data_destroy;
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
	if (handle->first_write) {
		handle->first_write = FALSE;

		/* test for GZ marker. todo: store the first 2 bytes in the odd circumstance that someone calls
		 * write() in 1 byte increments */
		if ((count >= 2) && (buf[0] == (guchar)0x1f) && (buf[1] == (guchar)0x8b)) {
			handle->is_gzipped = TRUE;

#ifdef HAVE_SVGZ
			handle->gzipped_data = GSF_OUTPUT (gsf_output_memory_new ());
#endif
		}
	}

	if (handle->is_gzipped) {
#ifdef HAVE_SVGZ
		return gsf_output_write (handle->gzipped_data, count, buf);
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
#if HAVE_SVGZ
	if (handle->is_gzipped) {
		GsfInput * gzip;
		const guchar * bytes;
		gsize size;
		gsize remaining;
		
		bytes = gsf_output_memory_get_bytes (GSF_OUTPUT_MEMORY (handle->gzipped_data));
		size = gsf_output_size (handle->gzipped_data);

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
		gsf_output_close (handle->gzipped_data);
	}
#endif

	return rsvg_handle_close_impl (handle, error);
}

/**
 * rsvg_handle_free:
 * @handle: An #RsvgHandle
 *
 * Frees #handle.
 **/
void
rsvg_handle_free (RsvgHandle *handle)
{
#if HAVE_SVGZ
	if (handle->is_gzipped)
		g_object_unref (G_OBJECT (handle->gzipped_data));
#endif

	rsvg_handle_free_impl (handle);
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
