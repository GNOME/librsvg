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
#include <libart_lgpl/art_rgba.h>

#include <math.h>

typedef struct _RsvgFilterContext RsvgFilterContext;

struct _RsvgFilterContext {
	gint width, height;
	RsvgFilter *filter;
	GHashTable *results;
	GdkPixbuf *source;
	GdkPixbuf *bg;
	GdkPixbuf *lastresult;
	double affine[6];
	double paffine[6];
};

typedef struct _RsvgFilterPrimitive RsvgFilterPrimitive;
typedef struct _RsvgFilterPrimitiveBlend RsvgFilterPrimitiveBlend;

struct _RsvgFilterPrimitive {
	double x, y, width, height; 
	GString *in;
	GString *result;  
	gboolean sizedefaults;

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

typedef struct {
	gint x1, y1, x2, y2;
} FPBox;

FPBox 
rsvg_filter_primitive_get_bounds(RsvgFilterPrimitive *self, RsvgFilterContext *ctx);

FPBox 
rsvg_filter_primitive_get_bounds(RsvgFilterPrimitive *self, RsvgFilterContext *ctx){
	FPBox output;

	if (self->sizedefaults){
		output.x1 = ctx->affine[0] * ctx->filter->x + ctx->affine[4];
		output.y1 = ctx->affine[3] * ctx->filter->y + ctx->affine[5];
		output.x2 = ctx->affine[0] * (ctx->filter->x + ctx->filter->width) + ctx->affine[4];
		output.y2 = ctx->affine[3] * (ctx->filter->y + ctx->filter->height) + ctx->affine[5];
		
		if (output.x1 < 0)
			output.x1 = 0;
		if (output.x2 >= ctx->width)
			output.x2 = ctx->width-1;
		if (output.y1 < 0)
			output.y1 = 0;
		if (output.y2 >= ctx->height)
			output.y2 = ctx->height-1;
		
		return output;
	}
	output.x1 = ctx->paffine[0] * self->x + ctx->paffine[4];
	output.y1 = ctx->paffine[3] * self->y + ctx->paffine[5];
	output.x2 = ctx->paffine[0] * (self->x + self->width) + ctx->paffine[4];
	output.y2 = ctx->paffine[3] * (self->y + self->height) + ctx->paffine[5];
	

	if (output.x1 < ctx->affine[0] * ctx->filter->x + ctx->affine[4])
		output.x1 = ctx->affine[0] * ctx->filter->x + ctx->affine[4];
	if (output.x2 > ctx->affine[0] * (ctx->filter->x + ctx->filter->width) + ctx->affine[4])
		output.x2 = ctx->affine[0] * (ctx->filter->x + ctx->filter->width) + ctx->affine[4];
	if (output.y1 < ctx->affine[3] * ctx->filter->y + ctx->affine[5])
		output.y1 = ctx->affine[3] * ctx->filter->y + ctx->affine[5];
	if (output.y2 > ctx->affine[3] * (ctx->filter->y + ctx->filter->height) + ctx->affine[5])
		output.y2 = ctx->affine[3] * (ctx->filter->y + ctx->filter->height) + ctx->affine[5];

	if (output.x1 < 0)
		output.x1 = 0;
	if (output.x2 >= ctx->width)
		output.x2 = ctx->width-1;
	if (output.y1 < 0)
		output.y1 = 0;
	if (output.y2 >= ctx->height)
		output.y2 = ctx->height-1;

	return output;
}

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
alpha_blt(GdkPixbuf *src, gint srcx, gint srcy, gint srcwidth, 
		  gint srcheight, GdkPixbuf *dst, gint dstx, gint dsty);

void
clear_pixbuf(GdkPixbuf *pb);

void
clear_pixbuf(GdkPixbuf *pb){
	gint i;
	guchar * data;
	data = gdk_pixbuf_get_pixels(pb);

	for (i=0; i<gdk_pixbuf_get_width(pb)*gdk_pixbuf_get_height(pb)*4;i++)
		{
			data[i] = 0;
		}
}

void
alpha_blt(GdkPixbuf *src, gint srcx, gint srcy, gint srcwidth, 
		  gint srcheight, GdkPixbuf *dst, gint dstx, gint dsty)
{
	gint rightx;
	gint bottomy;
	gint dstwidth;
	gint dstheight;

	gint srcoffsetx;
	gint srcoffsety;
	gint dstoffsetx;
	gint dstoffsety;

	dstheight = srcheight;
	dstwidth = srcwidth;

	rightx = srcx + srcwidth;
	bottomy = srcy + srcheight;

	if (rightx > gdk_pixbuf_get_width (src))
		rightx = gdk_pixbuf_get_width (src);
	if (bottomy > gdk_pixbuf_get_height (src))
		bottomy = gdk_pixbuf_get_height (src);
	srcwidth = rightx - srcx;
	srcheight = bottomy - srcy;

	rightx = dstx + dstwidth;
	bottomy = dsty + dstheight;
	if (rightx > gdk_pixbuf_get_width (dst))
		rightx = gdk_pixbuf_get_width (dst);
	if (bottomy > gdk_pixbuf_get_height (dst))
		bottomy = gdk_pixbuf_get_height (dst);
	dstwidth = rightx - dstx;
	dstheight = bottomy - dsty;

	if (dstwidth < srcwidth)
		srcwidth = dstwidth;
	if (dstheight < srcheight)
		srcheight = dstheight;

	if (srcx < 0)
		srcoffsetx = 0 - srcx;
	else 
		srcoffsetx = 0;
	if (srcy < 0)
		srcoffsety = 0 - srcy;
	else 
		srcoffsety = 0;
	if (dstx < 0)
		dstoffsetx = 0 - dstx;
	else 
		dstoffsetx = 0;
	if (dsty < 0)
		dstoffsety = 0 - dsty;
	else 
		dstoffsety = 0;

	if (dstoffsetx > srcoffsetx)
		srcoffsetx = dstoffsetx;
	if (dstoffsety > srcoffsety)
		srcoffsety = dstoffsety;

	gint x, y, srcrowstride, dstrowstride, sx, sy, dx, dy;
	guchar *src_pixels, *dst_pixels;

	srcrowstride = gdk_pixbuf_get_rowstride(src);	
	dstrowstride = gdk_pixbuf_get_rowstride(dst);

	src_pixels = gdk_pixbuf_get_pixels (src);
	dst_pixels = gdk_pixbuf_get_pixels (dst);

	for (y = srcoffsety; y < srcheight; y++)
		for (x = srcoffsetx; x < srcwidth; x++)
			{
				sx = x + srcx;
				sy = y + srcy;
				dx = x + dstx;
				dy = y + dsty;
				guchar r, g, b, a;
				a = src_pixels[4 * sx + sy * srcrowstride +  3];
				if (a)
					{
						r = src_pixels[4 * sx + sy * srcrowstride];
						g = src_pixels[4 * sx + 1 + sy * srcrowstride];
						b = src_pixels[4 * sx + 2 + sy * srcrowstride];
						art_rgba_run_alpha (dst_pixels + 4 * dx + 
											dy * dstrowstride, 
											r, g, b, a, 1);
					}
			}
}

void
rsvg_filter_fix_coordinate_system (RsvgFilterContext *ctx, 
								   RsvgState *state){
	int i, j;
	int x, y, height, width;
	guchar *pixels; 
	int stride;
	int currentindex;

	i = j = 0;

	x = y = width = height = 0;

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
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0){
				y = i;
				break;
			}
		}
		if (y != -1)
			break;
	}
	
	
	/*move in from the bottom to find the height*/
	for (i = gdk_pixbuf_get_height(ctx->source) - 1; i >= 0; i--) {
		for (j = 0; j < gdk_pixbuf_get_width(ctx->source); j++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0){	
				height = i - y;
				break;
			}
			
		}
		if (height != -1)
			break;
	}
	
		/*move in from the left to find the x value*/
	for (j = 0; j < gdk_pixbuf_get_width(ctx->source); j++) {
		for (i = y; i < (height + y); i++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0){
				x = j;
				break;
			}
		}
		if (x != -1)
			break;
	}
	
	/*move in from the right side to find the width*/
	for (j = gdk_pixbuf_get_width(ctx->source) - 1; j >= 0; j--) {
		for (i = y; i < (height + y); i++) {
			currentindex = i * stride + j * 4;
			if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0 ||
				pixels[currentindex + 2] != 0 || pixels[currentindex + 3] != 0){
				width = j - x;
				break;
			}
		}
		if (width != -1)
				break;
	}
	
	ctx->width = gdk_pixbuf_get_width(ctx->source);
	ctx->height = gdk_pixbuf_get_height(ctx->source);	

	if (ctx->filter->filterunits == userSpaceOnUse){
		for (i = 0; i < 6; i++)
			ctx->affine[i] = state->affine[i];
	} 
	else {
		ctx->affine[0] = width;
		ctx->affine[1] = 0.;		
		ctx->affine[2] = 0.;
		ctx->affine[3] = height;
		ctx->affine[4] = x;
		ctx->affine[5] = y;
	}

	if (ctx->filter->primitiveunits == userSpaceOnUse){
		for (i = 0; i < 6; i++)
			ctx->paffine[i] = state->affine[i];
	} 
	else {
		ctx->paffine[0] = width;
		ctx->paffine[1] = 0.;		
		ctx->paffine[2] = 0.;
		ctx->paffine[3] = height;
		ctx->paffine[4] = x;
		ctx->paffine[5] = y;
	}
	
	return;
}

void
rsvg_filter_free_pair(gpointer key, gpointer value, gpointer user_data)
{
	g_object_unref(G_OBJECT(value));
	g_free((gchar *)key);
}

/**
 * rsvg_filter_render: Copy the source to the bg using a filter.
 * @self: a pointer to the filter to use
 * @source: a pointer to the source pixbuf
 * @bg: the background pixbuf
 * @context: the context
 *
 * This function will create a context for itself, set up the coordinate systems
 * execute all its little primatives and then clean up its own mess
 **/

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

	g_object_ref(G_OBJECT(source));
	ctx->lastresult = source;
	
	rsvg_filter_fix_coordinate_system(ctx, rsvg_state_current (context));

	for (i = 0; i < self->primitives->len; i++)
		{
			current = g_ptr_array_index(self->primitives, i);
			rsvg_filter_primitive_render(current, ctx);
		}
	g_hash_table_foreach (ctx->results, rsvg_filter_free_pair, NULL);
	g_hash_table_destroy (ctx->results);

	alpha_blt(ctx->lastresult, 0, 0, gdk_pixbuf_get_width(source), gdk_pixbuf_get_height(source), bg, 0, 0);
	g_object_unref(G_OBJECT(ctx->lastresult));
}

/**
 * rsvg_filter_store_result: Files a result into a context.
 * @name: The name of the result
 * @result: The pointer to the result
 * @ctx: the context that this was called in
 *
 * Puts the new result into the hash for easy finding later, also
 * Stores it as the last result
 **/

void 
rsvg_filter_store_result (GString *name, GdkPixbuf *result, RsvgFilterContext *ctx){
	g_object_unref(G_OBJECT(ctx->lastresult)); 

	if (strcmp(name->str, "")){
		g_object_ref(G_OBJECT(result)); /*increments the references for the table*/
		g_hash_table_insert (ctx->results, g_strdup(name->str), result);
	}

	g_object_ref(G_OBJECT(result)); /*increments the references for the last result*/
	ctx->lastresult = result;
}

GdkPixbuf *
pixbuf_get_alpha(GdkPixbuf *pb);

GdkPixbuf *
pixbuf_get_alpha(GdkPixbuf *pb){
	gint i,j;
	guchar * data;
	guchar * pbdata;
	GdkPixbuf *output;

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 gdk_pixbuf_get_width(pb), 
							 gdk_pixbuf_get_height(pb));

	data = gdk_pixbuf_get_pixels(output);
	pbdata = gdk_pixbuf_get_pixels(pb);
	
	for (i=0; i<gdk_pixbuf_get_width(pb)*gdk_pixbuf_get_width(pb);i++){
		for (j=0; j< 3;j++)
			data[i*4 + j] = 0;
		data[i*4 + 3] = pbdata[i*4 + 3];
	}
	
	return output;
}

/**
 * rsvg_filter_get_in: Gets a pixbuf for a primative.
 * @name: The name of the pixbuf
 * @ctx: the context that this was called in
 *
 * Returns a pointer to the result that the name refers to, a special
 * Pixbuf if the name is a special keyword or NULL if nothing was found
 **/

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
	if (!strcmp(name->str, "") || !strcmp(name->str, "none"))
		{
			g_object_ref(G_OBJECT(ctx->lastresult));
			return ctx->lastresult;
		}
	if (!strcmp(name->str, "SourceAlpha"))
		return pixbuf_get_alpha(ctx->source);
	if (!strcmp(name->str, "BackgroundAlpha"))
		return pixbuf_get_alpha(ctx->bg);

	output = g_hash_table_lookup (ctx->results, name->str);
	g_object_ref(G_OBJECT(output));

	if (output != NULL){
		return output;
	}
	g_object_ref(G_OBJECT(ctx->lastresult));
	return ctx->lastresult;
}

/**
 * rsvg_filter_parse: Looks up an allready created filter.
 * @defs: a pointer to the hash of definitions
 * @str: a string with the name of the filter to be looked up
 *
 * Returns a pointer to the filter that the name refers to, or NULL
 * if none was found
 **/

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


/**
 * rsvg_new_filter: Creates a black filter
 *
 * Creates a blank filter and assigns default values to everything
 **/

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

/**
 * rsvg_filter_free: Free a filter.
 * @dself: The defval to be freed 
 *
 * Frees a filter and all primatives associated with this filter, this is 
 * to be set as its free function to be used with rsvg defs
 **/

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

/**
 * rsvg_start_filter: Create a filter from xml arguments.
 * @ctx: the current rsvg handle
 * @atts: the xml attributes that set the filter's properties
 *
 * Creates a new filter and sets it as a def
 * Also sets the context's current filter pointer to point to the
 * newly created filter so that all subsiquent primatives are
 * added to this filter until the filter is ended
 **/

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

/**
 * rsvg_end_filter: Create a filter from xml arguments.
 * @ctx: the current rsvg handle
 *
 * Ends the current filter block by setting the currentfilter ot null
 **/

void 
rsvg_end_filter (RsvgHandle *ctx) {
	ctx->currentfilter = NULL;
}

typedef enum {
	normal, multiply, screen, darken, lighten
} RsvgFilterPrimitiveBlendMode;

struct _RsvgFilterPrimitiveBlend {
	RsvgFilterPrimitive super;
	RsvgFilterPrimitiveBlendMode mode;
	GString *in2;
};

void rsvg_start_filter_primitive_blend (RsvgHandle *ctx, const xmlChar **atts);
void rsvg_filter_primitive_blend_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
void rsvg_filter_primitive_blend_free (RsvgFilterPrimitive * self);

void 
rsvg_filter_primitive_blend_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {
	guchar i;
	gint x,y;
	gint rowstride, height, width;
	FPBox boundarys;

	guchar *in_pixels;
	guchar *in2_pixels;
	guchar *output_pixels;

	RsvgFilterPrimitiveBlend *bself;

	GdkPixbuf *output;
	GdkPixbuf *in;
	GdkPixbuf *in2;

	bself = (RsvgFilterPrimitiveBlend *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);

	in = rsvg_filter_get_in(self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels(in);	
	in2 = rsvg_filter_get_in(bself->in2, ctx);
	in2_pixels = gdk_pixbuf_get_pixels(in2);

	height = gdk_pixbuf_get_height(in);
	width = gdk_pixbuf_get_width(in);

	rowstride = gdk_pixbuf_get_rowstride(in);

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 width, height);

	clear_pixbuf(output);

	output_pixels = gdk_pixbuf_get_pixels(output);
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2 ; x++)
			{
				double qr, cr, qa, qb, ca, cb;
				qa = (double)in_pixels[4 * x + y * rowstride +  3] / 255.0;
				qb = (double)in2_pixels[4 * x + y * rowstride +  3] / 255.0;
				qr = 1 - (1 - qa) * (1 - qb);
				cr = 0;
				for (i = 0; i < 3; i++)
					{
						ca = (double)in_pixels[4 * x + y * rowstride +  i] * qa / 255.0;
						cb = (double)in2_pixels[4 * x + y * rowstride +  i] * qb / 255.0;
						switch (bself->mode){
						case normal:
							cr = (1 - qa) * cb + ca;
							break;
						case multiply:
							cr = (1 - qa) * cb + (1 - qb) * ca + ca * cb;
							break;
						case screen:
							cr = cb + ca - ca * cb;
							break;
						case darken:
							cr = MIN ((1 - qa) * cb + ca, (1 - qb) * ca + cb);
							break;
						case lighten: 	
							cr = MAX ((1 - qa) * cb + ca, (1 - qb) * ca + cb);
							break;
						}
						cr *= 255.0 / qr;
						if (cr > 255)
							cr = 255;
						if (cr < 0)
							cr = 0;
						output_pixels[4 * x + y * rowstride + i] = (guchar)cr;
						
					}
				output_pixels[4 * x + y * rowstride + 3] = qr * 255.0;
			}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(in));
	g_object_unref(G_OBJECT(in2));
	g_object_unref(G_OBJECT(output));
}

void 
rsvg_filter_primitive_blend_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveBlend *bself;
	bself = (RsvgFilterPrimitiveBlend *)self;
	g_string_free(self->result, TRUE);
	g_string_free(self->in, TRUE);
	g_string_free(bself->in2, TRUE);	
	g_free(bself);
}

void 
rsvg_start_filter_primitive_blend (RsvgHandle *ctx, const xmlChar **atts) {
	int i;
	double font_size;
	RsvgFilterPrimitiveBlend * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveBlend, 1);
	filter->mode = normal;

	filter->super.in = g_string_new("none");
	filter->in2 = g_string_new("none");
	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "mode"))
						if (!strcmp ((char *)atts[i + 1], "multiply"))
							filter->mode = multiply;
						else if (!strcmp ((char *)atts[i + 1], "screen"))
							filter->mode = screen;
						else if (!strcmp ((char *)atts[i + 1], "darken"))
							filter->mode = darken;
						else if (!strcmp ((char *)atts[i + 1], "lighten"))
							filter->mode = lighten;
						else 
							filter->mode = normal;
					
					else if (!strcmp ((char *)atts[i], "in"))
						g_string_assign(filter->super.in, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "in2"))
						g_string_assign(filter->in2, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}			
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_blend_render;
	filter->super.free = &rsvg_filter_primitive_blend_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
}


typedef struct _RsvgFilterPrimitiveConvolveMatrix RsvgFilterPrimitiveConvolveMatrix;

struct _RsvgFilterPrimitiveConvolveMatrix {
	RsvgFilterPrimitive super;
	double *KernelMatrix;
	double divisor;
	gint orderx, ordery;
	double bias;
	gint targetx, targety;
	gboolean preservealpha;
};

void rsvg_start_filter_convolve_matrix (RsvgHandle *ctx, const xmlChar **atts);
void rsvg_filter_primitive_convolve_matrix_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
void rsvg_filter_primitive_convolve_matrix_free (RsvgFilterPrimitive * self);

void 
rsvg_filter_primitive_convolve_matrix_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {
	guchar ch;
	gint x,y;
	gint i, j;
	gint rowstride, height, width;
	FPBox boundarys;

	guchar *in_pixels;
	guchar *output_pixels;

	RsvgFilterPrimitiveConvolveMatrix *cself;

	GdkPixbuf *output;
	GdkPixbuf *in;

	gint sx, sy, kx, ky;
	guchar sval;
	double kval, sum;

	cself = (RsvgFilterPrimitiveConvolveMatrix *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);

	in = rsvg_filter_get_in(self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels(in);	

	height = gdk_pixbuf_get_height(in);
	width = gdk_pixbuf_get_width(in);

	rowstride = gdk_pixbuf_get_rowstride(in);

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 width, height);

	clear_pixbuf(output);

	output_pixels = gdk_pixbuf_get_pixels(output);
	
	gint tempresult;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2 ; x++){
			for (ch = 0; ch < 3 + !cself->preservealpha; ch++){
				sum = 0;
				for (i = 0; i < cself->ordery; i++)
					for (j = 0; j < cself->orderx; j++){
						sx = x - cself->targetx + j;
						sy = y - cself->targety + i;
						if (sx < boundarys.x1 || sx > boundarys.x2 || 
							sy < boundarys.y1 || sy > boundarys.y2)
							continue;
						kx = cself->orderx - j - 1;
						ky = cself->ordery - i - 1;
						sval = in_pixels[4 * sx + sy * rowstride + ch];
						kval = cself->KernelMatrix[kx + ky * cself->orderx];
						sum += (double)sval * kval;
					}
				tempresult =  sum / cself->divisor + cself->bias; 
				if (tempresult > 255)
					tempresult = 255;
				if (tempresult < 0)
					tempresult = 0;
		
				output_pixels[4 * x + y * rowstride + ch] = tempresult;
			}
			if (cself->preservealpha)
				output_pixels[4 * x + y * rowstride + 3] = 
					in_pixels[4 * x + y * rowstride + 3];
		}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(in));
	g_object_unref(G_OBJECT(output));
}

void 
rsvg_filter_primitive_convolve_matrix_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveConvolveMatrix *cself;
	cself = (RsvgFilterPrimitiveConvolveMatrix *)self;
	g_string_free(self->result, TRUE);
	g_string_free(self->in, TRUE);	
	g_free(cself->KernelMatrix);
	g_free(cself);
}

typedef struct _RsvgFilterPrimitiveGaussianBlur RsvgFilterPrimitiveGaussianBlur;

struct _RsvgFilterPrimitiveGaussianBlur {
	RsvgFilterPrimitive super;
	double sdx, sdy;
};

void 
rsvg_start_filter_primitive_convolve_matrix (RsvgHandle *ctx, const xmlChar **atts) {
	int i, j;
	double font_size;
	RsvgFilterPrimitiveConvolveMatrix * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveConvolveMatrix, 1);

	filter->super.in = g_string_new("none");
	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;


	filter->divisor = 0;
	filter->targetx = 0;
	filter->targety = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "in"))
						g_string_assign(filter->super.in, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}			
					else if (!strcmp ((char *)atts[i], "targetX"))
						filter->targetx = atoi((char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "targetY"))
						filter->targety = atoi((char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "bias"))
						filter->bias = atof((char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "preserveAlpha"))
						if (!strcmp ((char *)atts[i + 1], "true"))
							filter->preservealpha = TRUE;
						else
							filter->preservealpha = FALSE;
					else if (!strcmp ((char *)atts[i], "divisor"))
						filter->divisor = atof((char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "order")){
						double tempx, tempy;
						rsvg_css_parse_number_optional_number((char *)atts[i + 1], &tempx, &tempy);
						filter->orderx = tempx;
						filter->ordery = tempy;
						
						/*double *KernelMatrix;
						 */
					}
					else if (!strcmp ((char *)atts[i], "kernelMatrix"))
						filter->KernelMatrix = rsvg_css_parse_number_list((char *)atts[i + 1], NULL);
				}
			
		}
	if (filter->divisor == 0){
		for (j = 0; j < filter->orderx; j++)
			for (i = 0; i < filter->ordery; i++)
				filter->divisor += filter->KernelMatrix[j + i * filter->orderx];
	}
	if (filter->divisor == 0)
		filter->divisor = 1;
	

	filter->super.render = &rsvg_filter_primitive_convolve_matrix_render;
	filter->super.free = &rsvg_filter_primitive_convolve_matrix_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
}

void 
rsvg_filter_primitive_gaussian_blur_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);

void 
rsvg_filter_primitive_gaussian_blur_free (RsvgFilterPrimitive *self);

void 
rsvg_start_filter_primitive_gaussian_blur (RsvgHandle *ctx, const xmlChar **atts) {
	int i;

	double font_size;
	RsvgFilterPrimitiveGaussianBlur * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveGaussianBlur, 1);

	filter->super.in = g_string_new("none");
	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;
	filter->sdx = 0;
	filter->sdy = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "in"))
						g_string_assign(filter->super.in, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}			
					else if (!strcmp ((char *)atts[i], "stdDeviation"))
						rsvg_css_parse_number_optional_number((char *)atts[i + 1], &filter->sdx, &filter->sdy);	
				}
		}

	filter->super.render = &rsvg_filter_primitive_gaussian_blur_render;
	filter->super.free = &rsvg_filter_primitive_gaussian_blur_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
}

void 
rsvg_filter_primitive_gaussian_blur_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {
	guchar ch;
	gint x,y;
	gint i, j;
	gint rowstride, height, width;
	FPBox boundarys;

	guchar *in_pixels;
	guchar *output_pixels;

	RsvgFilterPrimitiveGaussianBlur *cself;

	GdkPixbuf *output;
	GdkPixbuf *in;

	gint sx, sy, kx, ky, kw, kh;
	guchar sval;
	double kval, sum;

	cself = (RsvgFilterPrimitiveGaussianBlur *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);

	in = rsvg_filter_get_in(self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels(in);	

	height = gdk_pixbuf_get_height(in);
	width = gdk_pixbuf_get_width(in);

	rowstride = gdk_pixbuf_get_rowstride(in);

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 width, height);
	clear_pixbuf(output);

	double sdx, sdy;

	/*scale the SD values*/
	sdx = cself->sdx * ctx->paffine[0];
	sdy = cself->sdy * ctx->paffine[3];

	double pi = 3.141592653589793238;

	kw = kh = 0;

	/*find out the required x size for the kernel matrix*/

	for (i = 1; i < 20; i++){
		if (exp(-(i*i)/(2*sdx*sdx))/sqrt(2*pi*sdx*sdx) < 0.0001){
			kw = 2 * (i - 1);
			break;
		}
	}

	/*find out the required y size for the kernel matrix*/
	for (i = 1; i < 20; i++){
		if (exp(-(i*i)/(2*sdy*sdy))/sqrt(2*pi*sdy*sdy) < 0.0001){
			kh = 2 * (i - 1);
			break;
		}
	}

	double *KernelMatrix;
	KernelMatrix = g_new(double, kw * kh);

	/*create the kernel matrix*/
	for (i = 0; i < kh; i++){
		for (j = 0; j < kw; j++){
			KernelMatrix[j + i * kw] = 
				(exp(-((j-kw/2)*(j-kw/2))/(2*sdx*sdx))/sqrt(2*pi*sdx*sdx))*
				(exp(-((i-kh/2)*(i-kh/2))/(2*sdy*sdy))/sqrt(2*pi*sdy*sdy));
		}
	}

	/*find out the total of the values of the matrix*/
	double divisor;
	divisor = 0;
	for (j = 0; j < kw; j++)
		for (i = 0; i < kh; i++)
			divisor += KernelMatrix[j + i * kw];


	output_pixels = gdk_pixbuf_get_pixels(output);
	
	gint tempresult;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2 ; x++)
			for (ch = 0; ch < 4; ch++){
				sum = 0;
				for (i = 0; i < kh; i++)
					for (j = 0; j < kw; j++){
						sx = x + j;
						sy = y + i;
						if (sx < boundarys.x1 || sx > boundarys.x2 || 
							sy < boundarys.y1 || sy > boundarys.y2)
							continue;
						kx = kw - j - 1;
						ky = kh - i - 1;
						sval = in_pixels[4 * sx + sy * rowstride + ch];
						kval = KernelMatrix[kx + ky * kw];
						sum += (double)sval * kval;
					}
				tempresult =  sum / divisor; 
				if (tempresult > 255)
					tempresult = 255;
				if (tempresult < 0)
					tempresult = 0;
				
				output_pixels[4 * x + y * rowstride + ch] = tempresult;
			}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(in));
	g_object_unref(G_OBJECT(output));
	g_free(KernelMatrix);
}

void 
rsvg_filter_primitive_gaussian_blur_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveGaussianBlur *cself;
	cself = (RsvgFilterPrimitiveGaussianBlur *)self;
	g_string_free(self->result, TRUE);
	g_string_free(self->in, TRUE);	
	g_free(cself);
}

typedef struct _RsvgFilterPrimitiveOffset RsvgFilterPrimitiveOffset;

struct _RsvgFilterPrimitiveOffset {
	RsvgFilterPrimitive super;
	gint dx, dy;
};

void 
rsvg_filter_primitive_offset_free (RsvgFilterPrimitive * self);

void 
rsvg_filter_primitive_offset_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
	
void 
rsvg_filter_primitive_offset_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {
	guchar ch;
	gint x,y;
	gint rowstride, height, width;
	FPBox boundarys;

	guchar *in_pixels;
	guchar *output_pixels;

	RsvgFilterPrimitiveOffset *oself;

	GdkPixbuf *output;
	GdkPixbuf *in;

	oself = (RsvgFilterPrimitiveOffset *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);

	in = rsvg_filter_get_in(self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels(in);	

	height = gdk_pixbuf_get_height(in);
	width = gdk_pixbuf_get_width(in);

	rowstride = gdk_pixbuf_get_rowstride(in);

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 width, height);

	clear_pixbuf(output);

	output_pixels = gdk_pixbuf_get_pixels(output);

	int ox, oy;

	ox = ctx->paffine[0] * oself->dx;
	oy = ctx->paffine[3] * oself->dy;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2 ; x++){
			if (x - ox< boundarys.x1 || y - oy >= boundarys.x2)
				continue;
			if (y - oy < boundarys.y1 || y - oy >= boundarys.y2)
				continue;
			for (ch = 0; ch < 4; ch++){
				output_pixels[y * rowstride + x * 4 + ch] =
					in_pixels[(y - oy) * rowstride + (x - ox) * 4 + ch];
			}
		}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(in));
	g_object_unref(G_OBJECT(output));
}


void 
rsvg_filter_primitive_offset_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveOffset *oself;
	oself = (RsvgFilterPrimitiveOffset *)self;
	g_string_free(self->result, TRUE);
	g_string_free(self->in, TRUE);	
	g_free(oself);
}

void 
rsvg_start_filter_primitive_offset (RsvgHandle *ctx, const xmlChar **atts) {
	int i;

	double font_size;
	RsvgFilterPrimitiveOffset * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveOffset, 1);

	filter->super.in = g_string_new("none");
	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;
	filter->dy = 0;
	filter->dx = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "in"))
						g_string_assign(filter->super.in, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}	
					else if (!strcmp ((char *)atts[i], "dx"))
						filter->dx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
					else if (!strcmp ((char *)atts[i], "dy"))
						filter->dy = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
				}
		}

	filter->super.render = &rsvg_filter_primitive_offset_render;
	filter->super.free = &rsvg_filter_primitive_offset_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
}

typedef struct _RsvgFilterPrimitiveMerge RsvgFilterPrimitiveMerge;

struct _RsvgFilterPrimitiveMerge {
	RsvgFilterPrimitive super;
	GPtrArray *nodes;
};

void 
rsvg_filter_primitive_merge_free (RsvgFilterPrimitive * self);

void 
rsvg_filter_primitive_merge_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
	
void 
rsvg_filter_primitive_merge_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {	
	guint i;
	FPBox boundarys;

	RsvgFilterPrimitiveMerge *mself;

	GdkPixbuf *output;
	GdkPixbuf *in;

	mself = (RsvgFilterPrimitiveMerge *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);


	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 ctx->width, ctx->height);
	clear_pixbuf(output);

	for (i = 0; i < mself->nodes->len; i++){
		in = rsvg_filter_get_in(g_ptr_array_index(mself->nodes, i), ctx);
		alpha_blt(in, boundarys.x1, boundarys.y1, boundarys.x2 - boundarys.x1, boundarys.y2 - boundarys.y1, output, boundarys.x1, boundarys.y1);
		g_object_unref(G_OBJECT(in));
	}
		
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(output));
}


void 
rsvg_filter_primitive_merge_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveMerge *mself;
	mself = (RsvgFilterPrimitiveMerge *)self;
	g_string_free(self->result, TRUE);
	guint i;
	for (i = 0; i < mself->nodes->len; i++)
		g_string_free(g_ptr_array_index(mself->nodes, i), TRUE);
	g_ptr_array_free(mself->nodes, FALSE);
	g_free(mself);
}

void 
rsvg_start_filter_primitive_merge (RsvgHandle *ctx, const xmlChar **atts) {
	int i;

	double font_size;
	RsvgFilterPrimitiveMerge * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveMerge, 1);

	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;
	filter->nodes = g_ptr_array_new();

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}	
				}
		}

	filter->super.render = &rsvg_filter_primitive_merge_render;
	filter->super.free = &rsvg_filter_primitive_merge_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
	ctx->currentmergefilter = filter;
}

void 
rsvg_start_filter_primitive_merge_node (RsvgHandle *ctx, const xmlChar **atts) {
	int i;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "in"))
						g_ptr_array_add(((RsvgFilterPrimitiveMerge *)(ctx->currentmergefilter))->nodes, 
										g_string_new((char *)atts[i + 1]));
				}
		}
}


typedef struct _RsvgFilterPrimitiveColourMatrix RsvgFilterPrimitiveColourMatrix;

struct _RsvgFilterPrimitiveColourMatrix {
	RsvgFilterPrimitive super;
	double *KernelMatrix;
	double divisor;
	gint orderx, ordery;
	double bias;
	gint targetx, targety;
	gboolean preservealpha;
};

void rsvg_start_filter_colour_matrix (RsvgHandle *ctx, const xmlChar **atts);
void rsvg_filter_primitive_colour_matrix_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx);
void rsvg_filter_primitive_colour_matrix_free (RsvgFilterPrimitive * self);

void 
rsvg_filter_primitive_colour_matrix_render (RsvgFilterPrimitive *self, RsvgFilterContext * ctx) {
	guchar ch;
	gint x,y;
	gint i;
	gint rowstride, height, width;
	FPBox boundarys;

	guchar *in_pixels;
	guchar *output_pixels;

	RsvgFilterPrimitiveColourMatrix *cself;

	GdkPixbuf *output;
	GdkPixbuf *in;

	double sum;

	cself = (RsvgFilterPrimitiveColourMatrix *) self;
	boundarys = rsvg_filter_primitive_get_bounds(self, ctx);

	in = rsvg_filter_get_in(self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels(in);	

	height = gdk_pixbuf_get_height(in);
	width = gdk_pixbuf_get_width(in);

	rowstride = gdk_pixbuf_get_rowstride(in);

	output = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
							 width, height);


	clear_pixbuf(output);

	output_pixels = gdk_pixbuf_get_pixels(output);
	
	gint tempresult;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2 ; x++){
			for (ch = 0; ch < 4; ch++){
				sum = 0;
				for (i = 0; i < 4; i++)
					{
					sum += cself->KernelMatrix[ch * 5 + i] * 
						in_pixels[4 * x + y * rowstride + i];
					}
				sum += cself->KernelMatrix[ch * 5 + 4];			

				tempresult =  sum; 
				if (tempresult > 255)
					tempresult = 255;
				if (tempresult < 0)
					tempresult = 0;	
				output_pixels[4 * x + y * rowstride + ch] = tempresult;
			}
		}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref(G_OBJECT(in));
	g_object_unref(G_OBJECT(output));
}

void 
rsvg_filter_primitive_colour_matrix_free (RsvgFilterPrimitive * self){
	RsvgFilterPrimitiveColourMatrix *cself;
	cself = (RsvgFilterPrimitiveColourMatrix *)self;
	g_string_free(self->result, TRUE);
	g_string_free(self->in, TRUE);	
	g_free(cself->KernelMatrix);
	g_free(cself);
}


void 
rsvg_start_filter_primitive_colour_matrix (RsvgHandle *ctx, const xmlChar **atts) {
	int i;
	int type;
	double font_size;
	RsvgFilterPrimitiveColourMatrix * filter;

	if (ctx->n_state > 0)
		font_size = rsvg_state_current (ctx)->font_size;
	else
		font_size = 12.0;

	filter = g_new(RsvgFilterPrimitiveColourMatrix, 1);

	filter->super.in = g_string_new("none");
	filter->super.result = g_string_new("none");
	filter->super.sizedefaults = 1;

	type = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "in"))
						g_string_assign(filter->super.in, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "result"))
						g_string_assign(filter->super.result, (char *)atts[i + 1]);

					else if (!strcmp ((char *)atts[i], "x")){
						filter->super.x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "y")){
						filter->super.y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					}
					else if (!strcmp ((char *)atts[i], "width")){
						filter->super.width = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;					
					}
					else if (!strcmp ((char *)atts[i], "height")){
						filter->super.height = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, font_size);
						filter->super.sizedefaults = 0;
					} 	
					else if (!strcmp ((char *)atts[i], "values"))
						filter->KernelMatrix = rsvg_css_parse_number_list((char *)atts[i + 1], NULL);
					else if(!strcmp ((char *)atts[i], "type")){
						if(!strcmp ((char *)atts[i + 1], "matrix"))
							type = 0;
						else if (!strcmp ((char *)atts[i+1], "saturate"))
							type = 1;
						else if (!strcmp ((char *)atts[i+1], "hueRotate"))
							type = 2;
						else if (!strcmp ((char *)atts[i+1], "luminanceToAlpha"))
							type = 3;
						else
							type = 0;
					}
				}
			
		}
	
	if (type == 1){
		float s;
		s = filter->KernelMatrix[0];
		g_free(filter->KernelMatrix);
		filter->KernelMatrix = g_new(double, 20);
		filter->KernelMatrix[0] = 0.213 + 0.787 * s;
		filter->KernelMatrix[1] = 0.715 - 0.715 * s;
		filter->KernelMatrix[2] = 0.072 - 0.072 * s;
		filter->KernelMatrix[3] = 0;
		filter->KernelMatrix[4] = 0;
		filter->KernelMatrix[5] = 0.213 - 0.213 * s;
		filter->KernelMatrix[6] = 0.715 + 0.285 * s;
		filter->KernelMatrix[7] = 0.072 - 0.072 * s;
		filter->KernelMatrix[8] = 0;
		filter->KernelMatrix[9] = 0;
		filter->KernelMatrix[10] = 0.213 - 0.213 * s;
		filter->KernelMatrix[11] = 0.715 - 0.715 * s;
		filter->KernelMatrix[12] = 0.072 + 0.928 * s;
		filter->KernelMatrix[13] = 0;
		filter->KernelMatrix[14] = 0;
		filter->KernelMatrix[15] = 0;
		filter->KernelMatrix[16] = 0;
		filter->KernelMatrix[17] = 0;
		filter->KernelMatrix[18] = 1;
		filter->KernelMatrix[19] = 0;
	}
	if (type == 2){
		double cosval, sinval;
		cosval = cos(filter->KernelMatrix[0]);
		sinval = sin(filter->KernelMatrix[0]);
		g_free(filter->KernelMatrix);
		filter->KernelMatrix = g_new(double, 20);

		filter->KernelMatrix[0] = 0.213+cosval*0.787+sinval*-0.213;
		filter->KernelMatrix[1] = 0.715+cosval*-0.715+sinval*-0.715;
		filter->KernelMatrix[2] = 0.072+cosval*-0.072+sinval*0.928;
		filter->KernelMatrix[3] = 0;
		filter->KernelMatrix[4] = 0;
		filter->KernelMatrix[5] = 0.213+cosval*-0.213+sinval*0.143;
		filter->KernelMatrix[6] = 0.715+cosval*0.285+sinval*0.140;
		filter->KernelMatrix[7] = 0.072+cosval*-0.072+sinval*-0.283;
		filter->KernelMatrix[8] = 0;
		filter->KernelMatrix[9] = 0;
		filter->KernelMatrix[10] = 0.213+cosval*-0.213+sinval*-0.787;
		filter->KernelMatrix[11] = 0.715+cosval*-0.715+sinval*0.715;
		filter->KernelMatrix[12] = 0.072+cosval*0.928+sinval*0.072;
		filter->KernelMatrix[13] = 0;
		filter->KernelMatrix[14] = 0;
		filter->KernelMatrix[15] = 0;
		filter->KernelMatrix[16] = 0;
		filter->KernelMatrix[17] = 0;
		filter->KernelMatrix[18] = 1;
		filter->KernelMatrix[19] = 0;
	}	
	if (type == 3){
		if (filter->KernelMatrix != NULL)
			g_free(filter->KernelMatrix);
		filter->KernelMatrix = g_new(double, 20);
		filter->KernelMatrix[0] = 0;
		filter->KernelMatrix[1] = 0;
		filter->KernelMatrix[2] = 0;
		filter->KernelMatrix[3] = 0;
		filter->KernelMatrix[4] = 0;
		filter->KernelMatrix[5] = 0;
		filter->KernelMatrix[6] = 0;
		filter->KernelMatrix[7] = 0;
		filter->KernelMatrix[8] = 0;
		filter->KernelMatrix[9] = 0;
		filter->KernelMatrix[10] = 0;
		filter->KernelMatrix[11] = 0;
		filter->KernelMatrix[12] = 0;
		filter->KernelMatrix[13] = 0;
		filter->KernelMatrix[14] = 0;
		filter->KernelMatrix[15] = 0.2125;
		filter->KernelMatrix[16] = 0.7154;
		filter->KernelMatrix[17] = 0.0721;
		filter->KernelMatrix[18] = 0;
		filter->KernelMatrix[19] = 0;
	}

	filter->super.render = &rsvg_filter_primitive_colour_matrix_render;
	filter->super.free = &rsvg_filter_primitive_colour_matrix_free;

	g_ptr_array_add(((RsvgFilter *)(ctx->currentfilter))->primitives, &filter->super);
}
