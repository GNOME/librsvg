/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-filter.c: Provides filters
 
   Copyright (C) 2004 Caleb Moore
  
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
  
   Author: Caleb Moore <calebmm@tpg.com.au>
*/

#include "rsvg-filter.h"
#include "rsvg-private.h"
#include "rsvg-css.h"

typedef struct _RsvgFilterContext RsvgFilterContext;

struct _RsvgFilterContext {
	RsvgFilter *filter;
	GHashTable *results;
	GdkPixbuf *source;
	GdkPixbuf *bg;
	GdkPixbuf *lastresult;
	double affine[6];
};

typedef struct _RsvgFilterPrimitive RsvgFilterPrimitive;
typedef struct _RsvgFilterPrimitiveBlend RsvgFilterPrimitiveBlend;

struct _RsvgFilterPrimitive {
	double x, y; 
	double width, height; 
	GString *in;
	GString *result;  

	void (*free) (RsvgFilterPrimitive *self);
	void (*render) (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
};

GdkPixbuf *
rsvg_filter_get_in (GString *name, RsvgFilterContext *ctx);

void 
rsvg_filter_primitive_free(RsvgFilterPrimitive *self);

void 
rsvg_filter_primitive_render(RsvgFilterPrimitive *self, 
							 RsvgFilterContext * ctx);

GdkPixbuf *
rsvg_filter_get_in (GString *name, RsvgFilterContext *ctx);

void 
rsvg_filter_primitive_render(RsvgFilterPrimitive *self, 
							 RsvgFilterContext * ctx){
	self->render(self, ctx);
}

void 
rsvg_filter_primitive_free(RsvgFilterPrimitive *self){
	self->free(self);
}


typedef enum {
	normal, multiply, screen, darken, lighten
} RsvgFilterPrimitiveBlendMode;

struct _RsvgFilterPrimitiveBlend {
	RsvgFilterPrimitiveBlendMode mode;
	RsvgFilterPrimitive super;
	GString *in2;
};

void
rsvg_filter_fix_coordinate_system (RsvgFilterContext *ctx, 
								   RsvgState *state);

void
rsvg_filter_free_pair(gpointer key, gpointer value, gpointer user_data);


void 
rsvg_filter_store_result (GString *name, GdkPixbuf *result, RsvgFilterContext *ctx);

RsvgFilter *
rsvg_new_filter (void);

void
rsvg_filter_fix_coordinate_system (RsvgFilterContext *ctx, 
								   RsvgState *state){
	int i, j;
	int x, y, height, width;
	guchar *pixels; 
	int stride;
	int currentindex;

	i = j = 0;

	if (ctx->filter->filterunits == userSpaceOnUse){
		for (i = 0; i < 6; i++)
			ctx->affine[i] = state->affine[i];
		return;
	}
	
	/*First for object bounding box coordinates we need to know how much of the 
	  source has been drawn on*/
	
	pixels = gdk_pixbuf_get_pixels(ctx->source);
	stride = gdk_pixbuf_get_rowstride(ctx->source);
	x = y = height = width = -1;

	/*move in from the top to find the y value*/
	for (i = 0; i < gdk_pixbuf_get_height(ctx->source); i++) {
		for (j = 0; j < gdk_pixbuf_get_width(ctx->source); j++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0)
				y = i;
		}
		if (y != -1)
			break;
	}
	
	/*if there are no pixels, there is no bounding box so instead of making everything 
	  segfault, lets just pretend we are using userSpaceOnUse*/
	if (j == gdk_pixbuf_get_width(ctx->source)) {
		ctx->filter->filterunits = userSpaceOnUse;
		rsvg_filter_fix_coordinate_system (ctx, state);
	}

	/*move in from the bottom to find the height*/
	for (i = gdk_pixbuf_get_height(ctx->source); i >= 0; i++) {
		for (j = 0; j < gdk_pixbuf_get_width(ctx->source); j++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0)
				height = i - y;
		}
		if (height != -1)
			break;
	}

	/*move in from the left to find the x value*/
	for (j = 0; j < gdk_pixbuf_get_width(ctx->source); j++) {
		for (i = y; i < height; i++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0)
				x = j;
		}
		if (height != -1)
			break;
	}

	/*move in from the right side to find the width*/
	for (j = gdk_pixbuf_get_width(ctx->source); j >= 0; j++) {
		for (i = y; i < height; i++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0)
				width = j - x;
		}
		if (height != -1)
			break;
	}

	ctx->affine[0] = width;
	ctx->affine[1] = 0.;		
	ctx->affine[2] = 0.;
	ctx->affine[3] = height;
	ctx->affine[4] = x;
	ctx->affine[5] = y;

	return;
}

void
rsvg_filter_free_pair(gpointer key, gpointer value, gpointer user_data)
{
	g_object_unref(G_OBJECT(value));
	g_string_free(key, 1);
}

void 
rsvg_filter_render (RsvgFilter *self, GdkPixbuf *source, GdkPixbuf *bg, RsvgHandle *context){
	RsvgFilterContext *ctx;
	RsvgFilterPrimitive *current;	
	guint i;

	ctx = g_new(RsvgFilterContext, 1);
	ctx->filter = self;
	ctx->source = source;
	ctx->bg = bg;
	ctx->results = g_hash_table_new(g_str_hash, g_str_equal);
	rsvg_filter_fix_coordinate_system(ctx, rsvg_state_current (context));

	for (i = 0; i < self->primitives->len; i++)
		{
			current = g_ptr_array_index(self->primitives, i);
			rsvg_filter_primitive_render(current, ctx);
		}

	g_hash_table_foreach (ctx->results, rsvg_filter_free_pair, NULL);
	g_hash_table_destroy (ctx->results);
}

void 
rsvg_filter_store_result (GString *name, GdkPixbuf *result, RsvgFilterContext *ctx){
	g_object_ref(G_OBJECT(result));
	g_hash_table_insert (ctx->results, name, result);
	ctx->lastresult = result;
}

GdkPixbuf *
rsvg_filter_get_in (GString *name, RsvgFilterContext *ctx){
	GdkPixbuf *output;

	if (!strcmp(name->str, "SourceGraphic"))
		{
			g_object_ref(G_OBJECT(ctx->source));			
			return ctx->source;
		}
	if (!strcmp(name->str, "BackgroundImage"))
		{
			g_object_ref(G_OBJECT(ctx->bg));
			return ctx->bg;
		}
	if (!strcmp(name->str, ""))
		{
			g_object_ref(G_OBJECT(ctx->lastresult));
			return ctx->lastresult;
		}
	/*if (!strcmp(name->str, "SourceAlpha"))
		return ctx->source;
	if (!strcmp(name->str, "BackgroundAlpha"))
	return ctx->source;*/

	output = g_hash_table_lookup (ctx->results, name);
	g_object_ref(G_OBJECT(output));
	return output;
}

void 
rsvg_filter_free (RsvgDefVal *dself) {
	RsvgFilter * self;
	self = (RsvgFilter *) dself;
	guint i;
	RsvgFilterPrimitive *current;
	for (i = 0; i < self->primitives->len; i++)
		{
			current = g_ptr_array_index(self->primitives, i);
			rsvg_filter_primitive_free(current);
		}
}

RsvgFilter *
rsvg_new_filter (void) {
	RsvgFilter * filter;
	filter = g_new(RsvgFilter, 1);
	filter->filterunits = objectBoundingBox;
	filter->primitiveunits = userSpaceOnUse;
	filter->x = -0.1;
	filter->y = -0.1;
	filter->width = 1.2;
	filter->height = 1.2;
	filter->primitives = g_ptr_array_new();
	return filter;
}

void 
rsvg_start_filter (RsvgHandle *ctx, const xmlChar **atts) {
	int i;
	const char * klazz = NULL; 
	char * id = NULL;
	RsvgFilter *filter;
	double font_size;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = rsvg_new_filter();

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "filterUnits"))
						if (!strcmp ((char *)atts[i], "userSpaceOnUse"))
							filter->filterunits = userSpaceOnUse;
						else
							filter->filterunits = objectBoundingBox;

					else if (!strcmp ((char *)atts[i], "primitiveUnits"))
						if (!strcmp ((char *)atts[i], "objectBoundingBox"))
							filter->primitiveunits = objectBoundingBox;
						else
							filter->primitiveunits = userSpaceOnUse;

					else if (!strcmp ((char *)atts[i], "x"))
						filter->x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);

					else if (!strcmp ((char *)atts[i], "y"))
						filter->y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);

					else if (!strcmp ((char *)atts[i], "width"))
						filter->width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);

					else if (!strcmp ((char *)atts[i], "height"))
						filter->height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);

					else if (!strcmp ((char *)atts[i], "filterRes"))
						;
					else if (!strcmp ((char *)atts[i], "xlink::href"))
						;
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (char *)atts[i + 1];
				}
		}
	ctx->currentfilter = filter;

	/*set up the defval stuff*/
	filter->super.type = RSVG_DEF_FILTER;
	filter->super.free = &rsvg_filter_free;
	rsvg_defs_set (ctx->defs, id, &filter->super);
}

void 
rsvg_end_filter (RsvgHandle *ctx) {
	ctx->currentfilter = NULL;
}

RsvgFilter *
rsvg_filter_parse (const RsvgDefs *defs, const char *str)
{
	if (!strcmp (str, "none"))
		return NULL;
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p)) p++;
			if (*p != '#')
				return NULL;
			p++;
			for (ix = 0; p[ix]; ix++)
				if (p[ix] == ')') break;
			if (p[ix] != ')')
				return NULL;
			name = g_strndup (p, ix);
			val = rsvg_defs_lookup (defs, name);
			g_free (name);
			if (val == NULL)
				return NULL;
	 
			if (val->type == RSVG_DEF_FILTER)
				return (RsvgFilter *)val;
			return NULL;
		}
	return NULL;
}
