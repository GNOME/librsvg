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

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif /*  M_PI  */

#define PERFECTBLUR 0

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterContext RsvgFilterContext;

struct _RsvgFilterContext
{
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

struct _RsvgFilterPrimitive
{
	double x, y, width, height;
	GString *in;
	GString *result;
	gboolean sizedefaults;
	
	void (*free) (RsvgFilterPrimitive * self);
	void (*render) (RsvgFilterPrimitive * self, RsvgFilterContext * ctx);
};

typedef struct
{
	gint x1, y1, x2, y2;
} FPBox;

/*************************************************************/
/*************************************************************/

static void
rsvg_filter_primitive_render (RsvgFilterPrimitive * self,
			      RsvgFilterContext * ctx)
{
	self->render (self, ctx);
}

static void
rsvg_filter_primitive_free (RsvgFilterPrimitive * self)
{
	self->free (self);
}

static FPBox
rsvg_filter_primitive_get_bounds (RsvgFilterPrimitive * self,
				  RsvgFilterContext * ctx)
{
	FPBox output;
	int skip;
	skip = 0;	

	if (self == NULL)
		skip = 1;
	else if (self->sizedefaults)
		skip = 1;

	if (skip)
		{
			output.x1 = ctx->affine[0] * ctx->filter->x + ctx->affine[4];
			output.y1 = ctx->affine[3] * ctx->filter->y + ctx->affine[5];
			output.x2 =
				ctx->affine[0] * (ctx->filter->x + ctx->filter->width) +
				ctx->affine[4];
			output.y2 =
				ctx->affine[3] * (ctx->filter->y + ctx->filter->height) +
				ctx->affine[5];

			if (output.x1 < 0)
				output.x1 = 0;
			if (output.x2 >= ctx->width)
				output.x2 = ctx->width - 1;
			if (output.y1 < 0)
				output.y1 = 0;
			if (output.y2 >= ctx->height)
				output.y2 = ctx->height - 1;		

			return output;
		}
	
	output.x1 = ctx->paffine[0] * self->x + ctx->paffine[4];
	output.y1 = ctx->paffine[3] * self->y + ctx->paffine[5];
	output.x2 = ctx->paffine[0] * (self->x + self->width) + ctx->paffine[4];
	output.y2 = ctx->paffine[3] * (self->y + self->height) + ctx->paffine[5];
	
	if (output.x1 < ctx->affine[0] * ctx->filter->x + ctx->affine[4])
		output.x1 = ctx->affine[0] * ctx->filter->x + ctx->affine[4];
	if (output.x2 >
		ctx->affine[0] * (ctx->filter->x + ctx->filter->width) + ctx->affine[4])
		output.x2 =
			ctx->affine[0] * (ctx->filter->x + ctx->filter->width) + ctx->affine[4];
	if (output.y1 < ctx->affine[3] * ctx->filter->y + ctx->affine[5])
		output.y1 = ctx->affine[3] * ctx->filter->y + ctx->affine[5];
	if (output.y2 > ctx->affine[3] * (ctx->filter->y + ctx->filter->height) +
		ctx->affine[5])
		output.y2 = ctx->affine[3] * (ctx->filter->y + ctx->filter->height) +
			ctx->affine[5];
	
	if (output.x1 < 0)
		output.x1 = 0;
	if (output.x2 >= ctx->width)
		output.x2 = ctx->width - 1;
	if (output.y1 < 0)
		output.y1 = 0;
	if (output.y2 >= ctx->height)
		output.y2 = ctx->height - 1;
	
	return output;
}

static GdkPixbuf *
gdk_pixbuf_new_cleared (GdkColorspace colorspace, gboolean has_alpha, int bits_per_sample,
						int width, int height)
{
	GdkPixbuf *pb;
	guchar *data;

	pb = gdk_pixbuf_new (colorspace, has_alpha, bits_per_sample, width, height);
	data = gdk_pixbuf_get_pixels (pb);
	memset(data, 0, width * height * 4);

	return pb;
}

static guchar
gdk_pixbuf_get_interp_pixel(guchar * src, gdouble ox, gdouble oy, guchar ch, FPBox boundarys, guint rowstride)
{
	double xmod, ymod;
	double dist1, dist2, dist3, dist4;
	double c, c1, c2, c3, c4;

	xmod = fmod(ox, 1.0);
	ymod = fmod(oy, 1.0);

	dist1 = (1 - xmod) * (1 - ymod);
	dist2 = (xmod) * (1 - ymod);
	dist3 = (xmod) * (ymod);
	dist4 = (1 - xmod) * (ymod);

	if (floor(ox) <= boundarys.x1 || floor(ox) >= boundarys.x2 || 
		floor(oy) <= boundarys.y1 || floor(oy) >= boundarys.y2)
		c1 = 0;
	else
		c1 = src[(guint)floor(oy) * rowstride + (guint)floor(ox) * 4 + ch];

	if (ceil(ox) <= boundarys.x1 || ceil(ox) >= boundarys.x2 || 
		floor(oy) <= boundarys.y1 || floor(oy) >= boundarys.y2)
		c2 = 0;
	else
		c2 = src[(guint)floor(oy) * rowstride + (guint)ceil(ox) * 4 + ch];

	if (ceil(ox) <= boundarys.x1 || ceil(ox) >= boundarys.x2 || 
		ceil(oy) <= boundarys.y1 || ceil(oy) >= boundarys.y2)
		c3 = 0;
	else
		c3 = src[(guint)ceil(oy) * rowstride + (guint)ceil(ox) * 4 + ch];
	
	if (floor(ox) <= boundarys.x1 || floor(ox) >= boundarys.x2 || 
		ceil(oy) <= boundarys.y1 || ceil(oy) >= boundarys.y2)
		c4 = 0;
	else
		c4 = src[(guint)ceil(oy) * rowstride + (guint)floor(ox) * 4 + ch];

	c = (c1 * dist1 + c2 * dist2 + c3 * dist3 + c4 * dist4) / (dist1 + dist2 + dist3 + dist4);

	return (guchar)c;
}

static void
alpha_blt (GdkPixbuf * src, gint srcx, gint srcy, gint srcwidth,
		   gint srcheight, GdkPixbuf * dst, gint dstx, gint dsty)
{
	gint rightx;
	gint bottomy;
	gint dstwidth;
	gint dstheight;
	
	gint srcoffsetx;
	gint srcoffsety;
	gint dstoffsetx;
	gint dstoffsety;
	
	gint x, y, srcrowstride, dstrowstride, sx, sy, dx, dy;
	guchar *src_pixels, *dst_pixels;
	
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
	
	srcrowstride = gdk_pixbuf_get_rowstride (src);
	dstrowstride = gdk_pixbuf_get_rowstride (dst);
	
	src_pixels = gdk_pixbuf_get_pixels (src);
	dst_pixels = gdk_pixbuf_get_pixels (dst);
	
	for (y = srcoffsety; y < srcheight; y++)
		for (x = srcoffsetx; x < srcwidth; x++)
			{
				guchar r, g, b, a;

				sx = x + srcx;
				sy = y + srcy;
				dx = x + dstx;
				dy = y + dsty;
				a = src_pixels[4 * sx + sy * srcrowstride + 3];
				if (a)
					{
						r = src_pixels[4 * sx + sy * srcrowstride];
						g = src_pixels[4 * sx + 1 + sy * srcrowstride];
						b = src_pixels[4 * sx + 2 + sy * srcrowstride];
						art_rgba_run_alpha (dst_pixels + 4 * dx +
											dy * dstrowstride, r, g, b, a, 1);
					}
			}
}

static void
rsvg_filter_fix_coordinate_system (RsvgFilterContext * ctx, RsvgState * state)
{
	int i, j;
	int x, y, height, width;
	guchar *pixels;
	int stride;
	int currentindex;
	
	i = j = 0;
	
	x = y = width = height = 0;
	
	/* First for object bounding box coordinates we need to know how much of the 
	   source has been drawn on */
	pixels = gdk_pixbuf_get_pixels (ctx->source);
	stride = gdk_pixbuf_get_rowstride (ctx->source);
	x = y = height = width = -1;
	

	if (ctx->filter->filterunits == objectBoundingBox || 
		ctx->filter->primitiveunits != objectBoundingBox)
		{
			/* move in from the top to find the y value */
			for (i = 0; i < gdk_pixbuf_get_height (ctx->source); i++)
				{
					for (j = 0; j < gdk_pixbuf_get_width (ctx->source); j++)
						{
							currentindex = i * stride + j * 4;
							if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0
								|| pixels[currentindex + 2] != 0
								|| pixels[currentindex + 3] != 0)
								{
									y = i;
									break;
								}
						}
					if (y != -1)
						break;
				}
			
			/* move in from the bottom to find the height */
			for (i = gdk_pixbuf_get_height (ctx->source) - 1; i >= 0; i--)
				{
					for (j = 0; j < gdk_pixbuf_get_width (ctx->source); j++)
						{
							currentindex = i * stride + j * 4;
							if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0
								|| pixels[currentindex + 2] != 0
								|| pixels[currentindex + 3] != 0)
								{
									height = i - y;
									break;
								}
							
						}
					if (height != -1)
						break;
				}
			
			/* move in from the left to find the x value */
			for (j = 0; j < gdk_pixbuf_get_width (ctx->source); j++)
				{
					for (i = y; i < (height + y); i++)
						{
							currentindex = i * stride + j * 4;
							if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0
								|| pixels[currentindex + 2] != 0
								|| pixels[currentindex + 3] != 0)
								{
									x = j;
									break;
								}
						}
					if (x != -1)
						break;
				}
			
			/* move in from the right side to find the width */
			for (j = gdk_pixbuf_get_width (ctx->source) - 1; j >= 0; j--)
				{
					for (i = y; i < (height + y); i++)
						{
							currentindex = i * stride + j * 4;
							if (pixels[currentindex + 0] != 0 || pixels[currentindex + 1] != 0
								|| pixels[currentindex + 2] != 0
								|| pixels[currentindex + 3] != 0)
								{
									width = j - x;
									break;
								}
						}
					if (width != -1)
						break;
				}
		}			

	ctx->width = gdk_pixbuf_get_width (ctx->source);
	ctx->height = gdk_pixbuf_get_height (ctx->source);
	
	if (ctx->filter->filterunits == userSpaceOnUse)
		{
			for (i = 0; i < 6; i++)
				ctx->affine[i] = state->affine[i];
		}
	else
		{
			ctx->affine[0] = width;
			ctx->affine[1] = 0.;
			ctx->affine[2] = 0.;
			ctx->affine[3] = height;
			ctx->affine[4] = x;
			ctx->affine[5] = y;
		}
	
	if (ctx->filter->primitiveunits == userSpaceOnUse)
		{
			for (i = 0; i < 6; i++)
				ctx->paffine[i] = state->affine[i];
		}
	else
		{
			ctx->paffine[0] = width;
			ctx->paffine[1] = 0.;
			ctx->paffine[2] = 0.;
			ctx->paffine[3] = height;
			ctx->paffine[4] = x;
			ctx->paffine[5] = y;
		}
}

static void
rsvg_filter_free_pair (gpointer key, gpointer value, gpointer user_data)
{
	g_object_unref (G_OBJECT (value));
	g_free ((gchar *) key);
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
rsvg_filter_render (RsvgFilter * self, GdkPixbuf * source, GdkPixbuf * output, 
					GdkPixbuf * bg, RsvgHandle * context)
{
	RsvgFilterContext *ctx;
	RsvgFilterPrimitive *current;
	guint i;
	FPBox bounds;
	
	ctx = g_new (RsvgFilterContext, 1);
	ctx->filter = self;
	ctx->source = source;
	ctx->bg = bg;
	ctx->results = g_hash_table_new (g_str_hash, g_str_equal);
	
	g_object_ref (G_OBJECT (source));
	ctx->lastresult = source;
	
	rsvg_filter_fix_coordinate_system (ctx, rsvg_state_current (context));
	
	for (i = 0; i < self->primitives->len; i++)
		{
			current = g_ptr_array_index (self->primitives, i);
			rsvg_filter_primitive_render (current, ctx);
		}

	g_hash_table_foreach (ctx->results, rsvg_filter_free_pair, NULL);
	g_hash_table_destroy (ctx->results);

	bounds = rsvg_filter_primitive_get_bounds (NULL, ctx);	

	alpha_blt (ctx->lastresult, bounds.x1, bounds.y1, bounds.x2 - bounds.x1,
			   bounds.y2 - bounds.y1, output, bounds.x1, bounds.y1);
	g_object_unref (G_OBJECT (ctx->lastresult));
	g_free(ctx);
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
static void
rsvg_filter_store_result (GString * name, GdkPixbuf * result,
						  RsvgFilterContext * ctx)
{
	g_object_unref (G_OBJECT (ctx->lastresult));
	
	if (strcmp (name->str, ""))
		{
			g_object_ref (G_OBJECT (result));	/* increments the references for the table */
			g_hash_table_insert (ctx->results, g_strdup (name->str), result);
		}
	
	g_object_ref (G_OBJECT (result));	/* increments the references for the last result */
	ctx->lastresult = result;
}

static GdkPixbuf *
pixbuf_get_alpha (GdkPixbuf * pb)
{
	guchar *data;
	guchar *pbdata;
	GdkPixbuf *output;
	
	gsize i, pbsize;

	pbsize = gdk_pixbuf_get_width (pb) * gdk_pixbuf_get_height (pb);

	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8,
									 gdk_pixbuf_get_width (pb),
									 gdk_pixbuf_get_height (pb));
	
	data = gdk_pixbuf_get_pixels (output);
	pbdata = gdk_pixbuf_get_pixels (pb);
	
	for (i = 0; i < pbsize; i++)
		data[i * 4 + 3] = pbdata[i * 4 + 3];
	
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
static GdkPixbuf *
rsvg_filter_get_in (GString * name, RsvgFilterContext * ctx)
{
	GdkPixbuf *output;

	if (!strcmp (name->str, "SourceGraphic"))
		{
			g_object_ref (G_OBJECT (ctx->source));
			return ctx->source;
		}
	else if (!strcmp (name->str, "BackgroundImage"))
		{
			g_object_ref (G_OBJECT (ctx->bg));
			return ctx->bg;
		}
	else if (!strcmp (name->str, "") || !strcmp (name->str, "none"))
		{
			g_object_ref (G_OBJECT (ctx->lastresult));
			return ctx->lastresult;
		}
	else if (!strcmp (name->str, "SourceAlpha"))
		return pixbuf_get_alpha (ctx->source);
	else if (!strcmp (name->str, "BackgroundAlpha"))
		return pixbuf_get_alpha (ctx->bg);
	
	output = g_hash_table_lookup (ctx->results, name->str);
	g_object_ref (G_OBJECT (output));
	
	if (output != NULL)
			return output;

	g_object_ref (G_OBJECT (ctx->lastresult));
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
rsvg_filter_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p))
				p++;

			if (*p == '#')
				{
					p++;
					for (ix = 0; p[ix]; ix++)
						if (p[ix] == ')')
							break;

					if (p[ix] == ')')
						{
							name = g_strndup (p, ix);
							val = rsvg_defs_lookup (defs, name);
							g_free (name);
							
							if (val && val->type == RSVG_DEF_FILTER)
								return (RsvgFilter *) val;
						}
				}
		}
	
	return NULL;
}

/**
 * rsvg_new_filter: Creates a black filter
 *
 * Creates a blank filter and assigns default values to everything
 **/
static RsvgFilter *
rsvg_new_filter (void)
{
	RsvgFilter *filter;

	filter = g_new (RsvgFilter, 1);
	filter->filterunits = objectBoundingBox;
	filter->primitiveunits = userSpaceOnUse;
	filter->x = -0.1;
	filter->y = -0.1;
	filter->width = 1.2;
	filter->height = 1.2;
	filter->primitives = g_ptr_array_new ();

	return filter;
}

/**
 * rsvg_filter_free: Free a filter.
 * @dself: The defval to be freed 
 *
 * Frees a filter and all primatives associated with this filter, this is 
 * to be set as its free function to be used with rsvg defs
 **/
static void
rsvg_filter_free (RsvgDefVal * dself)
{
	RsvgFilterPrimitive *current;
	RsvgFilter *self;
	guint i;
	
	self = (RsvgFilter *) dself;
	
	for (i = 0; i < self->primitives->len; i++)
		{
			current = g_ptr_array_index (self->primitives, i);
			rsvg_filter_primitive_free (current);
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
rsvg_start_filter (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	const char *klazz = NULL;
	char *id = NULL;
	RsvgFilter *filter;
	double font_size;
	
	font_size = rsvg_state_current_font_size (ctx);
	filter = rsvg_new_filter ();
	

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "filterUnits"))
						{
							if (!strcmp ((char *) atts[i + 1], "userSpaceOnUse"))
								filter->filterunits = userSpaceOnUse;
							else
								filter->filterunits = objectBoundingBox;
						}
					else if (!strcmp ((char *) atts[i], "primitiveUnits"))
						{
							if (!strcmp ((char *) atts[i + 1], "objectBoundingBox"))
								filter->primitiveunits = objectBoundingBox;
							else
								filter->primitiveunits = userSpaceOnUse;
						}
					else if (!strcmp ((char *) atts[i], "x"))
						filter->x =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);
					else if (!strcmp ((char *) atts[i], "y"))
						filter->y =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);
					else if (!strcmp ((char *) atts[i], "width"))
						filter->width =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);
					else if (!strcmp ((char *) atts[i], "height"))
						filter->height =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);					
					else if (!strcmp ((char *) atts[i], "filterRes"))
						;
					else if (!strcmp ((char *) atts[i], "xlink::href"))
						;
					else if (!strcmp ((char *) atts[i], "class"))
						klazz = (char *) atts[i + 1];
					else if (!strcmp ((char *) atts[i], "id"))
						id = (char *) atts[i + 1];
				}
		}
	ctx->currentfilter = filter;
	
	/* set up the defval stuff */
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
rsvg_end_filter (RsvgHandle * ctx)
{
	ctx->currentfilter = NULL;
}

/*************************************************************/
/*************************************************************/

typedef enum
{
  normal, multiply, screen, darken, lighten
}
RsvgFilterPrimitiveBlendMode;

typedef struct _RsvgFilterPrimitiveBlend RsvgFilterPrimitiveBlend;
struct _RsvgFilterPrimitiveBlend
{
	RsvgFilterPrimitive super;
	RsvgFilterPrimitiveBlendMode mode;
	GString *in2;
};

static void
rsvg_filter_primitive_blend_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	guchar i;
	gint x, y;
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
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	in2 = rsvg_filter_get_in (bself->in2, ctx);
	in2_pixels = gdk_pixbuf_get_pixels (in2);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				double qr, cr, qa, qb, ca, cb;

				qa = (double) in_pixels[4 * x + y * rowstride + 3] / 255.0;
				qb = (double) in2_pixels[4 * x + y * rowstride + 3] / 255.0;
				qr = 1 - (1 - qa) * (1 - qb);
				cr = 0;
				for (i = 0; i < 3; i++)
					{
						ca = (double) in_pixels[4 * x + y * rowstride + i] * qa / 255.0;
						cb = (double) in2_pixels[4 * x + y * rowstride + i] * qb / 255.0;
						switch (bself->mode)
							{
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
						output_pixels[4 * x + y * rowstride + i] = (guchar) cr;
						
					}
				output_pixels[4 * x + y * rowstride + 3] = qr * 255.0;
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (in2));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_blend_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveBlend *bself;
	
	bself = (RsvgFilterPrimitiveBlend *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_string_free (bself->in2, TRUE);
	g_free (bself);
}

void
rsvg_start_filter_primitive_blend (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	double font_size;
	RsvgFilterPrimitiveBlend *filter;
	
	font_size = rsvg_state_current_font_size (ctx);

	filter = g_new (RsvgFilterPrimitiveBlend, 1);
	filter->mode = normal;
	filter->super.in = g_string_new ("none");
	filter->in2 = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "mode")) 
						{
							if (!strcmp ((char *) atts[i + 1], "multiply"))
								filter->mode = multiply;
							else if (!strcmp ((char *) atts[i + 1], "screen"))
								filter->mode = screen;
							else if (!strcmp ((char *) atts[i + 1], "darken"))
								filter->mode = darken;
							else if (!strcmp ((char *) atts[i + 1], "lighten"))
								filter->mode = lighten;
							else
								filter->mode = normal;
						}
					else if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "in2"))
						g_string_assign (filter->in2, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_blend_render;
	filter->super.free = &rsvg_filter_primitive_blend_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveConvolveMatrix RsvgFilterPrimitiveConvolveMatrix;

struct _RsvgFilterPrimitiveConvolveMatrix
{
	RsvgFilterPrimitive super;
	double *KernelMatrix;
	double divisor;
	gint orderx, ordery;
	double dx, dy;
	double bias;
	gint targetx, targety;
	gboolean preservealpha;
	gint edgemode;
};

static void
rsvg_filter_primitive_convolve_matrix_render (RsvgFilterPrimitive * self,
											  RsvgFilterContext * ctx)
{
	guchar ch;
	gint x, y;
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
	double kval, sum, dx, dy, targetx, targety;
	
	gint tempresult;
	
	cself = (RsvgFilterPrimitiveConvolveMatrix *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	targetx = cself->targetx * ctx->paffine[0];
	targety = cself->targety * ctx->paffine[3];

	if (cself->dx != 0 || cself->dy != 0)
		{
			dx = cself->dx * ctx->paffine[0];
			dy = cself->dy * ctx->paffine[3];
		}
	else
		dx = dy = 1;

	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				for (ch = 0; ch < 3 + !cself->preservealpha; ch++)
					{
						sum = 0;
						for (i = 0; i < cself->ordery; i++)
							for (j = 0; j < cself->orderx; j++)
								{
									sx = x - targetx + j * dx;
									sy = y - targety + i * dy;
									if (cself->edgemode == 0)
										{
											if (sx < boundarys.x1)
												sx = boundarys.x1;
											if (sx >= boundarys.x2)
												sx = boundarys.x2 - 1;
											if (sy < boundarys.y1)
												sy = boundarys.y1;
											if (sy >= boundarys.y2)
												sy = boundarys.y2 - 1;
										}
									else if (cself->edgemode == 1)
										{
											if (sx < boundarys.x1 || (sx >= boundarys.x2))
												sx = boundarys.x1 + (sx - boundarys.x1) %
													(boundarys.x2 - boundarys.x1);
											if (sy < boundarys.y1 || (sy >= boundarys.y2))
												sy = boundarys.y1 + (sy - boundarys.y1) %
													(boundarys.y2 - boundarys.y1);
										}
									else if (cself->edgemode == 2)
										if (sx < boundarys.x1 || (sx >= boundarys.x2) || 
											sy < boundarys.y1 || (sy >= boundarys.y2))
										continue;

									kx = cself->orderx - j - 1;
									ky = cself->ordery - i - 1;
									sval = in_pixels[4 * sx + sy * rowstride + ch];
									kval = cself->KernelMatrix[kx + ky * cself->orderx];
									sum += (double) sval *kval;
								}
						tempresult = sum / cself->divisor + cself->bias;

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
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_convolve_matrix_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveConvolveMatrix *cself;

	cself = (RsvgFilterPrimitiveConvolveMatrix *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (cself->KernelMatrix);
	g_free (cself);
}

void
rsvg_start_filter_primitive_convolve_matrix (RsvgHandle * ctx,
											 const xmlChar ** atts)
{
	int i, j, listlen;
	double font_size;
	RsvgFilterPrimitiveConvolveMatrix *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveConvolveMatrix, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;	
	
	filter->divisor = 0;
	filter->bias = 0;
	filter->targetx = 0;
	filter->targety = 0;
	filter->dx = 0;
	filter->dy = 0;
	
	filter->edgemode = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "targetX"))
						filter->targetx = atoi ((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "targetY"))
						filter->targety = atoi ((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "bias"))
						filter->bias = atof ((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "preserveAlpha"))
						{
							if (!strcmp ((char *) atts[i + 1], "true"))
								filter->preservealpha = TRUE;
							else
								filter->preservealpha = FALSE;
						}
					else if (!strcmp ((char *) atts[i], "divisor"))
						filter->divisor = atof ((char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "order"))
						{
							double tempx, tempy;
							rsvg_css_parse_number_optional_number ((char *) atts[i + 1],
																   &tempx, &tempy);
							filter->orderx = tempx;
							filter->ordery = tempy;
							
						}
						else if (!strcmp ((char *) atts[i], "kernelUnitLength"))
							rsvg_css_parse_number_optional_number ((char *) atts[i + 1],
																   &filter->dx, &filter->dy);
							
					else if (!strcmp ((char *) atts[i], "kernelMatrix"))
						filter->KernelMatrix =
							rsvg_css_parse_number_list ((char *) atts[i + 1], &listlen);

					if (!strcmp ((char *) atts[i], "edgeMode")) 
						{
							if (!strcmp ((char *) atts[i + 1], "wrap"))
								filter->edgemode = 1;
							else if (!strcmp ((char *) atts[i + 1], "none"))
								filter->edgemode = 2;
							else
								filter->edgemode = 0;
						}
				}			
		}

	if (filter->divisor == 0)
		{
			for (j = 0; j < filter->orderx; j++)
				for (i = 0; i < filter->ordery; i++)
					filter->divisor += filter->KernelMatrix[j + i * filter->orderx];
		}

	if (filter->divisor == 0)
		filter->divisor = 1;
		
	if (listlen < filter->orderx * filter->ordery)
		filter->orderx = filter->ordery = 0;

	filter->super.render = &rsvg_filter_primitive_convolve_matrix_render;
	filter->super.free = &rsvg_filter_primitive_convolve_matrix_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveGaussianBlur
RsvgFilterPrimitiveGaussianBlur;

struct _RsvgFilterPrimitiveGaussianBlur
{
	RsvgFilterPrimitive super;
	double sdx, sdy;
};


#if PERFECTBLUR != 0
static void
true_blur (GdkPixbuf *in, GdkPixbuf *output, gfloat sdx, 
		   gfloat sdy, FPBox boundarys)
{
	guchar ch;
	gint x, y;
	gint i, j;
	gint rowstride, height, width;
	
	guchar *in_pixels;
	guchar *output_pixels;

	gint sx, sy, kx, ky, kw, kh;
	guchar sval;
	double kval, sum;
	
	double *KernelMatrix;
	double divisor;
	
	gint tempresult;

	kw = kh = 0;

	in_pixels = gdk_pixbuf_get_pixels (in);
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	/* find out the required x size for the kernel matrix */
	
	for (i = 1; i < 20; i++)
		{
			if (exp (-(i * i) / (2 * sdx * sdx)) / sqrt (2 * M_PI * sdx * sdx) <
				0.0001)
				{
					break;
				}
		}
	if (i < 1)
		i = 1;

	kw = 2 * (i - 1);

	/* find out the required y size for the kernel matrix */
	for (i = 1; i < 20; i++)
		{
		if (exp (-(i * i) / (2 * sdy * sdy)) / sqrt (2 * M_PI * sdy * sdy) <
			0.0001)
			{
				break;
			}
    }
	
	if (i < 1)
		i = 1;

	kh = 2 * (i - 1);

	KernelMatrix = g_new (double, kw * kh);
	
	/* create the kernel matrix */
	for (i = 0; i < kh; i++)
		{
			for (j = 0; j < kw; j++)
				{
					KernelMatrix[j + i * kw] =
						(exp (-((j - kw / 2) * (j - kw / 2)) / (2 * sdx * sdx)) /
						 sqrt (2 * M_PI * sdx * sdx)) *
						(exp (-((i - kh / 2) * (i - kh / 2)) / (2 * sdy * sdy)) /
						 sqrt (2 * M_PI * sdy * sdy));
				}
		}
	
	/* find out the total of the values of the matrix */
	divisor = 0;
	for (j = 0; j < kw; j++)
		for (i = 0; i < kh; i++)
			divisor += KernelMatrix[j + i * kw];
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			for (ch = 0; ch < 4; ch++)
				{
					sum = 0;
					for (i = 0; i < kh; i++)
						for (j = 0; j < kw; j++)
							{
								sx = x + j - kw / 2;
								sy = y + i - kh / 2;

								if (sx < boundarys.x1)
									sx = boundarys.x1;
								if (sx >= boundarys.x2)
									sx = boundarys.x2 - 1;
								if (sy < boundarys.y1)
									sy = boundarys.y1;
								if (sy >= boundarys.y2)
									sy = boundarys.y2 - 1;

								kx = kw - j - 1;
								ky = kh - i - 1;
								sval = in_pixels[4 * sx + sy * rowstride + ch];
								kval = KernelMatrix[kx + ky * kw];
								sum += (double) sval * kval;
							}

					tempresult = sum / divisor;
					if (tempresult > 255)
						tempresult = 255;
					if (tempresult < 0)
						tempresult = 0;
					
					output_pixels[4 * x + y * rowstride + ch] = tempresult;
				}
	g_free (KernelMatrix);
}

#endif

static void
box_blur (GdkPixbuf *in, GdkPixbuf *output, GdkPixbuf *intermediate, gint kw, 
		  gint kh, FPBox boundarys)
{
	guchar ch;
	gint x, y;
	gint rowstride, height, width;
	
	guchar *in_pixels;
	guchar *output_pixels;

	gint sum;	

	gint divisor;

	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);

	in_pixels = gdk_pixbuf_get_pixels (in);
	output_pixels = gdk_pixbuf_get_pixels (intermediate);
	
	rowstride = gdk_pixbuf_get_rowstride (in);


	if (kw >= 1)	
		{
			for (ch = 0; ch < 4; ch++)
				{
					for (y = boundarys.y1; y < boundarys.y2; y++)
						{
							sum = 0;
							divisor = 0;
							for (x = boundarys.x1; x < boundarys.x1 + kw; x++)
								{
									divisor++;
									sum += in_pixels[4 * x + y * rowstride + ch];
									if (x - kw / 2 >= 0 && x - kw / 2 < boundarys.x2)
										{
											output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / divisor;
										}
								}
							for (x = boundarys.x1 + kw; x < boundarys.x2; x++)
								{
									sum -= in_pixels[4 * (x - kw) + y * rowstride + ch];
									sum += in_pixels[4 * x + y * rowstride + ch];
									output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / divisor;
								}
							for (x = boundarys.x2; x < boundarys.x2 + kw; x++)
								{
									divisor--;
									sum -= in_pixels[4 * (x - kw) + y * rowstride + ch];
									if (x - kw / 2 >= 0 && x - kw / 2 < boundarys.x2)
										{
											output_pixels[4 * (x - kw / 2) + y * rowstride + ch] = sum / divisor;
										}
								}
						}
				}
		}
	else
		intermediate = in;
	
	in_pixels = gdk_pixbuf_get_pixels (intermediate);
	output_pixels = gdk_pixbuf_get_pixels (output);

	if (kh >= 1)
		{
			for (ch = 0; ch < 4; ch++)
				{
					for (x = boundarys.x1; x < boundarys.x2; x++)
						{
							sum = 0;
							divisor = 0;
							
							for (y = boundarys.y1; y < boundarys.y1 + kh; y++)
								{
									divisor++;
									sum += in_pixels[4 * x + y * rowstride + ch];
									if (y - kh / 2 >= 0 && y - kh / 2 < boundarys.y2)
										{
											output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / divisor;
										}
								}
							for (y = boundarys.y1 + kh; y < boundarys.y2; y++)
								{
									sum -= in_pixels[4 * x + (y - kh) * rowstride + ch];
									sum += in_pixels[4 * x + y * rowstride + ch];
									output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / divisor;
								}
							for (y = boundarys.y2; y < boundarys.y2 + kh; y++)
								{
									divisor--;
									sum -= in_pixels[4 * x + (y - kh) * rowstride + ch];
									if (y - kh / 2 >= 0 && y - kh / 2 < boundarys.y2)
										{
											output_pixels[4 * x + (y - kh / 2) * rowstride + ch] = sum / divisor;
										}
								}
						}
				}
			
		}
	else
		{
			gdk_pixbuf_copy_area(intermediate, 0,0, width, height, 
								 output, 0, 0);
		}

}

static void
fast_blur (GdkPixbuf *in, GdkPixbuf *output, gfloat sx, 
		   gfloat sy, FPBox boundarys)
{
	GdkPixbuf *intermediate1;
	GdkPixbuf *intermediate2;
	gint kx, ky;

	kx = floor(sx * 3*sqrt(2*M_PI)/4 + 0.5);
	ky = floor(sy * 3*sqrt(2*M_PI)/4 + 0.5);

	intermediate1 = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, 
											gdk_pixbuf_get_width (in),
											gdk_pixbuf_get_height (in));
	intermediate2 = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, 
											gdk_pixbuf_get_width (in),
											gdk_pixbuf_get_height (in));

	box_blur (in, intermediate2, intermediate1, kx, 
			  ky, boundarys);
	box_blur (intermediate2, intermediate2, intermediate1, kx, 
			  ky, boundarys);
	box_blur (intermediate2, output, intermediate1, kx, 
			  ky, boundarys);

	g_object_unref (G_OBJECT (intermediate1));
	g_object_unref (G_OBJECT (intermediate2));
}

static void
rsvg_filter_primitive_gaussian_blur_render (RsvgFilterPrimitive * self,
											RsvgFilterContext * ctx)
{
	RsvgFilterPrimitiveGaussianBlur *cself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	FPBox boundarys;
	gfloat sdx, sdy;
	
	cself = (RsvgFilterPrimitiveGaussianBlur *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, 
									 gdk_pixbuf_get_width (in),
									 gdk_pixbuf_get_height (in));
	
	/* scale the SD values */
	sdx = cself->sdx * ctx->paffine[0];
	sdy = cself->sdy * ctx->paffine[3];
	
#if PERFECTBLUR != 0
	if (sdx * sdy <= PERFECTBLUR)
		true_blur (in, output, sdx, 
				   sdy, boundarys);
	else
		fast_blur (in, output, sdx, 
				   sdy, boundarys);
#else
	fast_blur (in, output, sdx, 
				   sdy, boundarys);
#endif

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_gaussian_blur_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveGaussianBlur *cself;
	
	cself = (RsvgFilterPrimitiveGaussianBlur *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (cself);
}

void
rsvg_start_filter_primitive_gaussian_blur (RsvgHandle * ctx,
										   const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveGaussianBlur *filter;

	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveGaussianBlur, 1);

	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->sdx = 0;
	filter->sdy = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "stdDeviation"))
						rsvg_css_parse_number_optional_number ((char *) atts[i + 1],
															   &filter->sdx,
															   &filter->sdy);
				}
		}

	filter->super.render = &rsvg_filter_primitive_gaussian_blur_render;
	filter->super.free = &rsvg_filter_primitive_gaussian_blur_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveOffset RsvgFilterPrimitiveOffset;

struct _RsvgFilterPrimitiveOffset
{
	RsvgFilterPrimitive super;
	gint dx, dy;
};

static void
rsvg_filter_primitive_offset_render (RsvgFilterPrimitive * self,
									 RsvgFilterContext * ctx)
{
	guchar ch;
	gint x, y;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveOffset *oself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	int ox, oy;
	
	oself = (RsvgFilterPrimitiveOffset *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	ox = ctx->paffine[0] * oself->dx;
	oy = ctx->paffine[3] * oself->dy;
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				if (x - ox < boundarys.x1 || x - ox >= boundarys.x2)
					continue;
				if (y - oy < boundarys.y1 || y - oy >= boundarys.y2)
					continue;
		
				for (ch = 0; ch < 4; ch++)
					{
						output_pixels[y * rowstride + x * 4 + ch] =
							in_pixels[(y - oy) * rowstride + (x - ox) * 4 + ch];
					}
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_offset_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveOffset *oself;
	
	oself = (RsvgFilterPrimitiveOffset *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (oself);
}

void
rsvg_start_filter_primitive_offset (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveOffset *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveOffset, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->dy = 0;
	filter->dx = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "dx"))
						filter->dx =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);
					else if (!strcmp ((char *) atts[i], "dy"))
						filter->dy =
							rsvg_css_parse_normalized_length ((char *) atts[i + 1],
															  ctx->dpi,
															  1,
															  font_size);
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_offset_render;
	filter->super.free = &rsvg_filter_primitive_offset_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveMerge RsvgFilterPrimitiveMerge;

struct _RsvgFilterPrimitiveMerge
{
	RsvgFilterPrimitive super;
	GPtrArray *nodes;
};

static void
rsvg_filter_primitive_merge_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	guint i;
	FPBox boundarys;
	
	RsvgFilterPrimitiveMerge *mself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	mself = (RsvgFilterPrimitiveMerge *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, ctx->width, ctx->height);
	
	for (i = 0; i < mself->nodes->len; i++)
		{
			in = rsvg_filter_get_in (g_ptr_array_index (mself->nodes, i), ctx);
			alpha_blt (in, boundarys.x1, boundarys.y1, boundarys.x2 - boundarys.x1,
					   boundarys.y2 - boundarys.y1, output, boundarys.x1,
					   boundarys.y1);
			g_object_unref (G_OBJECT (in));
		}
	
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_merge_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveMerge *mself;
	guint i;
	
	mself = (RsvgFilterPrimitiveMerge *) self;
	g_string_free (self->result, TRUE);
	
	for (i = 0; i < mself->nodes->len; i++)
		g_string_free (g_ptr_array_index (mself->nodes, i), TRUE);
	g_ptr_array_free (mself->nodes, FALSE);
	g_free (mself);
}

void
rsvg_start_filter_primitive_merge (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveMerge *filter;
	
	font_size = rsvg_state_current_font_size (ctx);

	filter = g_new (RsvgFilterPrimitiveMerge, 1);
	
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->nodes = g_ptr_array_new ();

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_merge_render;
	filter->super.free = &rsvg_filter_primitive_merge_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
	ctx->currentsubfilter = filter;
}

void
rsvg_start_filter_primitive_merge_node (RsvgHandle * ctx,
										const xmlChar ** atts)
{
	int i;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_ptr_array_add (((RsvgFilterPrimitiveMerge *) (ctx->
																		currentsubfilter))->
										 nodes, g_string_new ((char *) atts[i + 1]));
				}
		}
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveColourMatrix
RsvgFilterPrimitiveColourMatrix;

struct _RsvgFilterPrimitiveColourMatrix
{
	RsvgFilterPrimitive super;
	double *KernelMatrix;
	double divisor;
	gint orderx, ordery;
	double bias;
	gint targetx, targety;
	gboolean preservealpha;
};

static void
rsvg_filter_primitive_colour_matrix_render (RsvgFilterPrimitive * self,
											RsvgFilterContext * ctx)
{
	guchar ch;
	gint x, y;
	gint i;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveColourMatrix *cself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	double sum;
	
	gint tempresult;

	cself = (RsvgFilterPrimitiveColourMatrix *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);	
	output_pixels = gdk_pixbuf_get_pixels (output);   
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				for (ch = 0; ch < 4; ch++)
					{
						sum = 0;
						for (i = 0; i < 4; i++)
							{
								sum += cself->KernelMatrix[ch * 5 + i] *
									in_pixels[4 * x + y * rowstride + i];
							}
						sum += cself->KernelMatrix[ch * 5 + 4];
						
						tempresult = sum;
						if (tempresult > 255)
							tempresult = 255;
						if (tempresult < 0)
							tempresult = 0;
						output_pixels[4 * x + y * rowstride + ch] = tempresult;
					}
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_colour_matrix_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveColourMatrix *cself;

	cself = (RsvgFilterPrimitiveColourMatrix *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	if (cself->KernelMatrix)
		g_free (cself->KernelMatrix);
	g_free (cself);
}

void
rsvg_start_filter_primitive_colour_matrix (RsvgHandle * ctx,
										   const xmlChar ** atts)
{
	gint i, type, listlen;
	double font_size;
	RsvgFilterPrimitiveColourMatrix *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveColourMatrix, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	
	filter->KernelMatrix = NULL;

	type = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "values"))
						{
							filter->KernelMatrix =
								rsvg_css_parse_number_list ((char *) atts[i + 1], &listlen);
						}
					else if (!strcmp ((char *) atts[i], "type"))
						{
							if (!strcmp ((char *) atts[i + 1], "matrix"))
								type = 0;
							else if (!strcmp ((char *) atts[i + 1], "saturate"))
								type = 1;
							else if (!strcmp ((char *) atts[i + 1], "hueRotate"))
								type = 2;
							else if (!strcmp ((char *) atts[i + 1], "luminanceToAlpha"))
								type = 3;
							else
								type = 0;
						}
				}			
		}

	if (type == 0)
		{
			if (listlen != 20)
				{
					if (filter->KernelMatrix != NULL)
						g_free (filter->KernelMatrix);
					filter->KernelMatrix = g_new0 (double, 20);
				}
		}
	else if (type == 1)
		{
			float s;
			if (listlen != 0)
				{
					s = filter->KernelMatrix[0];
					g_free (filter->KernelMatrix);
				}
			else
				s = 1;
			filter->KernelMatrix = g_new0 (double, 20);

			filter->KernelMatrix[0] = 0.213 + 0.787 * s;
			filter->KernelMatrix[1] = 0.715 - 0.715 * s;
			filter->KernelMatrix[2] = 0.072 - 0.072 * s;
			filter->KernelMatrix[5] = 0.213 - 0.213 * s;
			filter->KernelMatrix[6] = 0.715 + 0.285 * s;
			filter->KernelMatrix[7] = 0.072 - 0.072 * s;
			filter->KernelMatrix[10] = 0.213 - 0.213 * s;
			filter->KernelMatrix[11] = 0.715 - 0.715 * s;
			filter->KernelMatrix[12] = 0.072 + 0.928 * s;
			filter->KernelMatrix[18] = 1;
		}
	else if (type == 2)
		{
			double cosval, sinval, arg;

			if (listlen != 0)
				{
					arg = filter->KernelMatrix[0];
					g_free (filter->KernelMatrix);
				}
			else
				arg = 0;

			cosval = cos (arg);
			sinval = sin (arg);

			filter->KernelMatrix = g_new0 (double, 20);
			
			filter->KernelMatrix[0] = 0.213 + cosval * 0.787 + sinval * -0.213;
			filter->KernelMatrix[1] = 0.715 + cosval * -0.715 + sinval * -0.715;
			filter->KernelMatrix[2] = 0.072 + cosval * -0.072 + sinval * 0.928;
			filter->KernelMatrix[5] = 0.213 + cosval * -0.213 + sinval * 0.143;
			filter->KernelMatrix[6] = 0.715 + cosval * 0.285 + sinval * 0.140;
			filter->KernelMatrix[7] = 0.072 + cosval * -0.072 + sinval * -0.283;
			filter->KernelMatrix[10] = 0.213 + cosval * -0.213 + sinval * -0.787;
			filter->KernelMatrix[11] = 0.715 + cosval * -0.715 + sinval * 0.715;
			filter->KernelMatrix[12] = 0.072 + cosval * 0.928 + sinval * 0.072;
			filter->KernelMatrix[18] = 1;
		}
	else if (type == 3)
		{
			if (filter->KernelMatrix != NULL)
				g_free (filter->KernelMatrix);

			filter->KernelMatrix = g_new0 (double, 20);

			filter->KernelMatrix[15] = 0.2125;
			filter->KernelMatrix[16] = 0.7154;
			filter->KernelMatrix[17] = 0.0721;
		}
	else 
		{
			g_assert_not_reached();
		}

	filter->super.render = &rsvg_filter_primitive_colour_matrix_render;
	filter->super.free = &rsvg_filter_primitive_colour_matrix_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

struct ComponentTransferData
{
	gdouble *tableValues;
	guint nbTableValues;
	
	gdouble slope;
	gdouble intercept;
	gdouble amplitude;
	gdouble exponent;
	gdouble offset;
};

typedef gdouble (*ComponentTransferFunc) (gdouble C,
										 struct ComponentTransferData *
										 user_data);

typedef struct _RsvgFilterPrimitiveComponentTransfer
RsvgFilterPrimitiveComponentTransfer;


struct _RsvgFilterPrimitiveComponentTransfer
{
	RsvgFilterPrimitive super;
	ComponentTransferFunc Rfunction;
	struct ComponentTransferData Rdata;
	ComponentTransferFunc Gfunction;
	struct ComponentTransferData Gdata;
	ComponentTransferFunc Bfunction;
	struct ComponentTransferData Bdata;
	ComponentTransferFunc Afunction;
	struct ComponentTransferData Adata;
};

static gint
get_component_transfer_table_value (gdouble C,
									struct ComponentTransferData *user_data)
{
	gdouble N;
	gint k;
	N = user_data->nbTableValues;	

	k = floor(C * N);
	k -= 1;
	if (k < 0)
		k = 0;
	return k;
}

static gdouble
identity_component_transfer_func (gdouble C,
								  struct ComponentTransferData *user_data)
{
	return C;
}

static gdouble
table_component_transfer_func (gdouble C,
							   struct ComponentTransferData *user_data)
{
	guint k;
	gdouble vk, vk1;
	gfloat distancefromlast;
	
	if (!user_data->nbTableValues)
		return C;
	
	k = get_component_transfer_table_value (C, user_data);

	if (k == user_data->nbTableValues - 1)
		return user_data->tableValues[k - 1];

	vk = user_data->tableValues[k];
	vk1 = user_data->tableValues[k + 1];
	
	distancefromlast = (C - ((double)k + 1) / (double)user_data->nbTableValues) * (double)user_data->nbTableValues; 

	return (vk + distancefromlast * (vk1 - vk));
}

static gdouble
discrete_component_transfer_func (gdouble C,
								  struct ComponentTransferData *user_data)
{
	gint k;
	
	if (!user_data->nbTableValues)
		return C;
	
	k = get_component_transfer_table_value (C, user_data);
	
	return user_data->tableValues[k];
}

static gdouble
linear_component_transfer_func (gdouble C,
								struct ComponentTransferData *user_data)
{
	return (user_data->slope * C) + user_data->intercept;
}

static gdouble
gamma_component_transfer_func (gdouble C,
							   struct ComponentTransferData *user_data)
{
	return user_data->amplitude * pow (C,
									   user_data->exponent) + user_data->offset;
}

static void 
rsvg_filter_primitive_component_transfer_render (RsvgFilterPrimitive *
												self,
												RsvgFilterContext * ctx)
{
	gint x, y;
	gint temp;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveComponentTransfer *cself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	cself = (RsvgFilterPrimitiveComponentTransfer *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	
	output_pixels = gdk_pixbuf_get_pixels (output);

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				temp = cself->Rfunction((double)in_pixels[y * rowstride + x * 4] / 255.0, &cself->Rdata) * 255.0;
				if (temp > 255)
					temp = 255;
				else if (temp < 0)
					temp = 0;
				output_pixels[y * rowstride + x * 4] = temp;
		
				temp = cself->Gfunction((double)in_pixels[y * rowstride + x * 4 + 1] / 255.0, &cself->Gdata) * 255.0;
				if (temp > 255)
					temp = 255;
				else if (temp < 0)
					temp = 0;
				output_pixels[y * rowstride + x * 4 + 1] = temp;

				temp = cself->Bfunction((double)in_pixels[y * rowstride + x * 4 + 2] / 255.0, &cself->Bdata) * 255.0;
				if (temp > 255)
					temp = 255;
				else if (temp < 0)
					temp = 0;				
				output_pixels[y * rowstride + x * 4 + 2] = temp;

				temp = cself->Afunction((double)in_pixels[y * rowstride + x * 4 + 3] / 255.0, &cself->Adata) * 255.0;
				if (temp > 255)
					temp = 255;
				else if (temp < 0)
					temp = 0;
				output_pixels[y * rowstride + x * 4 + 3] = temp;		
			}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void 
rsvg_filter_primitive_component_transfer_free (RsvgFilterPrimitive *
											   self)
{
	RsvgFilterPrimitiveComponentTransfer *cself;

	cself = (RsvgFilterPrimitiveComponentTransfer *) self;
	g_string_free (self->result, TRUE);
	if (cself->Rdata.nbTableValues)
		g_free (cself->Rdata.tableValues);
	if (cself->Gdata.nbTableValues)
		g_free (cself->Gdata.tableValues);
	if (cself->Bdata.nbTableValues)
		g_free (cself->Bdata.tableValues);
	if (cself->Adata.nbTableValues)
		g_free (cself->Adata.tableValues);
	g_free (cself);
}


void 
rsvg_start_filter_primitive_component_transfer (RsvgHandle * ctx,
												const xmlChar ** atts)
{
	int i;
		double font_size;
	RsvgFilterPrimitiveComponentTransfer *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveComponentTransfer, 1);
	
	filter->super.result = g_string_new ("none");
	filter->super.in = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->Rfunction = identity_component_transfer_func;
	filter->Gfunction = identity_component_transfer_func;
	filter->Bfunction = identity_component_transfer_func;
	filter->Afunction = identity_component_transfer_func;
	filter->Rdata.nbTableValues = 0;
	filter->Gdata.nbTableValues = 0;
	filter->Bdata.nbTableValues = 0;
	filter->Adata.nbTableValues = 0;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_component_transfer_render;
	filter->super.free = &rsvg_filter_primitive_component_transfer_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);

	ctx->currentsubfilter = filter;
}

void 
rsvg_start_filter_primitive_component_transfer_function (RsvgHandle * ctx,
														 const xmlChar ** atts, char channel)
{
	int i;

	ComponentTransferFunc * function;
	struct ComponentTransferData * data;
	
	function = NULL;
	data = NULL;

	if (channel == 'r')
		{
			function = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Rfunction;
			data = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Rdata;
		}
	else if (channel == 'g')
		{
			function = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Gfunction;
			data = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Gdata;
		}
	else if (channel == 'b')
		{
			function = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Bfunction;
			data = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Bdata;
		}
	else if (channel == 'a')
		{
			function = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Afunction;
			data = &((RsvgFilterPrimitiveComponentTransfer *)(ctx->currentsubfilter))->Adata;
		}
	else
		{
			g_assert_not_reached();
		}

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "type"))
						{
							if (!strcmp ((char *) atts[i + 1], "identity"))
								*function = identity_component_transfer_func;
							else if (!strcmp ((char *) atts[i + 1], "table"))
								*function = table_component_transfer_func;
							else if (!strcmp ((char *) atts[i + 1], "discrete"))
								*function = discrete_component_transfer_func;
							else if (!strcmp ((char *) atts[i + 1], "linear"))
								*function = linear_component_transfer_func;
							else if (!strcmp ((char *) atts[i + 1], "gamma"))
								*function = gamma_component_transfer_func;
						}
					else if (!strcmp ((char *) atts[i], "tableValues"))
						{
							data->tableValues = 
								rsvg_css_parse_number_list ((char *) atts[i + 1], 
															&data->nbTableValues);
						}
					else if (!strcmp ((char *) atts[i], "slope"))
						{
							data->slope = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "intercept"))
						{
							data->intercept = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "amplitude"))
						{
							data->amplitude = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "exponent"))
						{
							data->exponent = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "offset"))
						{
							data->offset = g_ascii_strtod(atts[i + 1], NULL); 
						}
				}
		}
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveErode
RsvgFilterPrimitiveErode;

struct _RsvgFilterPrimitiveErode
{
	RsvgFilterPrimitive super;
	double rx, ry;
	int mode;
};

static void
rsvg_filter_primitive_erode_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	guchar ch, extreme;
	gint x, y;
	gint i, j;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveErode *cself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	gint kx, ky;
	guchar val;
	
	cself = (RsvgFilterPrimitiveErode *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	/* scale the radius values */
	kx = cself->rx * ctx->paffine[0];
	ky = cself->ry * ctx->paffine[3];

	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);

	output_pixels = gdk_pixbuf_get_pixels (output);
	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			for (ch = 0; ch < 4; ch++)
				{
					if (cself->mode == 0)
						extreme = 255;
					else
						extreme = 0;
					for (i = -ky; i < ky + 1; i++)
						for (j = -kx; j < kx + 1; j++)
							{
								if (y + i >= height || y + i < 0 || 
									x + j >= width || x + j < 0)
									continue;
								
								val = in_pixels[(y + i) * rowstride 
												+ (x + j) * 4 + ch];
							   

								if (cself->mode == 0)
									{	
										if (extreme > val)
											extreme = val;
									}
								else
									{
										if (extreme < val)
											extreme = val;
									}
								
							}
					output_pixels[y * rowstride + x * 4 + ch] = extreme;
				}
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_erode_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveErode *cself;
	
	cself = (RsvgFilterPrimitiveErode *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (cself);
}

void
rsvg_start_filter_primitive_erode (RsvgHandle * ctx,
								   const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveErode *filter;

	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveErode, 1);

	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->rx = 0;
	filter->ry = 0;
	filter->mode = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "radius"))
						{
						rsvg_css_parse_number_optional_number ((char *) atts[i + 1],
															   &filter->rx,
															   &filter->ry);
						}
					else if (!strcmp ((char *) atts[i], "operator"))
						{
							if (!strcmp ((char *) atts[i + 1], "erode"))
								filter->mode = 0;
							else if (!strcmp ((char *) atts[i + 1], "dilate"))
								filter->mode = 1;
						}
				}
		}

	filter->super.render = &rsvg_filter_primitive_erode_render;
	filter->super.free = &rsvg_filter_primitive_erode_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef enum
{
	COMPOSITE_MODE_OVER, COMPOSITE_MODE_IN, COMPOSITE_MODE_OUT, 
	COMPOSITE_MODE_ATOP, COMPOSITE_MODE_XOR, COMPOSITE_MODE_ARITHMETIC
}
RsvgFilterPrimitiveCompositeMode;

typedef struct _RsvgFilterPrimitiveComposite RsvgFilterPrimitiveComposite;
struct _RsvgFilterPrimitiveComposite
{
	RsvgFilterPrimitive super;
	RsvgFilterPrimitiveCompositeMode mode;
	GString *in2;

	gdouble k1, k2, k3, k4;
};

static void
rsvg_filter_primitive_composite_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	guchar i;
	gint x, y;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *in2_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveComposite *bself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	GdkPixbuf *in2;
	
	bself = (RsvgFilterPrimitiveComposite *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	in2 = rsvg_filter_get_in (bself->in2, ctx);
	in2_pixels = gdk_pixbuf_get_pixels (in2);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	if (bself->mode == COMPOSITE_MODE_ARITHMETIC)
		{
			for (y = boundarys.y1; y < boundarys.y2; y++)
				for (x = boundarys.x1; x < boundarys.x2; x++)
					{
						gdouble ca, cb, cr;
						for (i = 0; i < 3; i++)
							{
								ca = (double) in_pixels[4 * x + y * rowstride + i] / 255.0;
								cb = (double) in2_pixels[4 * x + y * rowstride + i] / 255.0;
							
								cr = bself->k1*ca*cb + bself->k2*ca + bself->k3*cb + bself->k4;

								if (cr > 1)
									cr = 1;
								if (cr < 0)
									cr = 0;
								output_pixels[4 * x + y * rowstride + i] = (guchar)(cr * 255.0);
							}

						ca = (double) in_pixels[4 * x + y * rowstride + 3] / 255.0;
						cb = (double) in2_pixels[4 * x + y * rowstride + 3] / 255.0;
						
						cr = bself->k2*ca + bself->k3*cb;
						
						if (cr > 1)
							cr = 1;
						if (cr < 0)
							cr = 0;
						output_pixels[4 * x + y * rowstride + 3] = (guchar)(cr * 255.0);
					}
			rsvg_filter_store_result (self->result, output, ctx);
			
			g_object_unref (G_OBJECT (in));
			g_object_unref (G_OBJECT (in2));
			g_object_unref (G_OBJECT (output));
			return;
		}

	
	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				double qr, cr, qa, qb, ca, cb, Fa, Fb;

				qa = (double) in_pixels[4 * x + y * rowstride + 3] / 255.0;
				qb = (double) in2_pixels[4 * x + y * rowstride + 3] / 255.0;
				cr = 0;
				Fa = Fb = 0;
				switch (bself->mode)
					{
					case COMPOSITE_MODE_OVER:
						Fa = 1;
						Fb = 1 - qa;
						break;
					case COMPOSITE_MODE_IN:
						Fa = qb;
						Fb = 0;
						break;
					case COMPOSITE_MODE_OUT:
						Fa = 1 - qb;
						Fb = 0;
						break;
					case COMPOSITE_MODE_ATOP:
						Fa = qb;
						Fb = 1 - qa;
						break;
					case COMPOSITE_MODE_XOR:
						Fa = 1 - qb;
						Fb = 1 - qa;
						break;
					case COMPOSITE_MODE_ARITHMETIC:
						break;
					}
				
				qr = Fa * qa + Fb * qb;

				for (i = 0; i < 3; i++)
					{
						ca = (double) in_pixels[4 * x + y * rowstride + i] / 255.0 * qa;
						cb = (double) in2_pixels[4 * x + y * rowstride + i] / 255.0 * qb;
					
						cr = (ca * Fa + cb * Fb) / qr;
						if (cr > 1)
							cr = 1;
						if (cr < 0)
							cr = 0;
						output_pixels[4 * x + y * rowstride + i] = (guchar)(cr * 255.0);
						
					}
				if (qr > 1)
					qr = 1;
				if (qr < 0)
					qr = 0;
				output_pixels[4 * x + y * rowstride + 3] = (guchar)(qr * 255.0);
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (in2));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_composite_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveComposite *bself;
	
	bself = (RsvgFilterPrimitiveComposite *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_string_free (bself->in2, TRUE);
	g_free (bself);
}

void
rsvg_start_filter_primitive_composite (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	double font_size;
	RsvgFilterPrimitiveComposite *filter;
	
	font_size = rsvg_state_current_font_size (ctx);

	filter = g_new (RsvgFilterPrimitiveComposite, 1);
	filter->mode = COMPOSITE_MODE_OVER;
	filter->super.in = g_string_new ("none");
	filter->in2 = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->k1 = 0;
	filter->k2 = 0;
	filter->k3 = 0;
	filter->k4 = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "operator")) 
						{
							if (!strcmp ((char *) atts[i + 1], "in"))
								filter->mode = COMPOSITE_MODE_IN;
							else if (!strcmp ((char *) atts[i + 1], "out"))
								filter->mode = COMPOSITE_MODE_OUT;
							else if (!strcmp ((char *) atts[i + 1], "atop"))
								filter->mode = COMPOSITE_MODE_ATOP;
							else if (!strcmp ((char *) atts[i + 1], "xor"))
								filter->mode = COMPOSITE_MODE_XOR;
							else if (!strcmp ((char *) atts[i + 1], 
											  "arithmetic"))
								filter->mode = COMPOSITE_MODE_ARITHMETIC;
							else
								filter->mode = COMPOSITE_MODE_OVER;
						}
					else if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "in2"))
						g_string_assign (filter->in2, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);					
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "k1"))
						{
							filter->k1 = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "k2"))
						{
							filter->k2 = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "k3"))
						{
							filter->k3 = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "k4"))
						{
							filter->k4 = g_ascii_strtod(atts[i + 1], NULL); 
						}
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_composite_render;
	filter->super.free = &rsvg_filter_primitive_composite_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveFlood
RsvgFilterPrimitiveFlood;

struct _RsvgFilterPrimitiveFlood
{
	RsvgFilterPrimitive super;
	guint32 colour;
	guint opacity;
};

static void
rsvg_filter_primitive_flood_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	guchar i;
	gint x, y;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *output_pixels;
	
	RsvgFilterPrimitiveFlood *bself;
	
	GdkPixbuf *output;
	
	bself = (RsvgFilterPrimitiveFlood *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	height = ctx->height;
	width = ctx->width;
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	rowstride = gdk_pixbuf_get_rowstride (output);
	
	output_pixels = gdk_pixbuf_get_pixels (output);

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				for (i = 0; i < 3; i++)
					{
						output_pixels[4 * x + y * rowstride + i] = ((char *)
							(&bself->colour))[2 - i];
					}
				output_pixels[4 * x + y * rowstride + 3] = bself->opacity;
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_flood_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveFlood *cself;
	
	cself = (RsvgFilterPrimitiveFlood *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (cself);
}

void
rsvg_start_filter_primitive_flood (RsvgHandle * ctx,
								   const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveFlood *filter;

	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveFlood, 1);

	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;

	filter->opacity = 255;
	filter->colour = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{			
					if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "flood-color"))
						{
							filter->colour = rsvg_css_parse_color ((char *) atts[i + 1]);						
						}
					else if (!strcmp ((char *) atts[i], "flood-opacity"))
						{
							filter->opacity = rsvg_css_parse_opacity ((char *) atts[i + 1]);
						}
				}
		}

	filter->super.render = &rsvg_filter_primitive_flood_render;
	filter->super.free = &rsvg_filter_primitive_flood_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveDisplacementMap RsvgFilterPrimitiveDisplacementMap;

struct _RsvgFilterPrimitiveDisplacementMap
{
	RsvgFilterPrimitive super;
	gint dx, dy;
	char xChannelSelector, yChannelSelector;
	GString *in2;
	double scale;
};

static void
rsvg_filter_primitive_displacement_map_render (RsvgFilterPrimitive * self,
											   RsvgFilterContext * ctx)
{
	guchar ch, xch, ych;
	gint x, y;
	gint rowstride, height, width;
	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *in2_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveDisplacementMap *oself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	GdkPixbuf *in2;
	
	double ox, oy;
	
	oself = (RsvgFilterPrimitiveDisplacementMap *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);

	in2 = rsvg_filter_get_in (oself->in2, ctx);
	in2_pixels = gdk_pixbuf_get_pixels (in2);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	switch (oself->xChannelSelector)
		{
		case 'R':
			xch = 0;
			break;
		case 'G':
			xch = 1;
			break;
		case 'B':
			xch = 2;
			break;
		case 'A':
			xch = 3;
			break;
		default:
			xch = 4;
		};

	switch (oself->yChannelSelector)
		{
		case 'R':
			ych = 0;
			break;
		case 'G':
			ych = 1;
			break;
		case 'B':
			ych = 2;
			break;
		case 'A':
			ych = 3;
			break;
		default:
			ych = 4;
		};

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				if (xch != 4)
					ox = x + oself->scale * ctx->paffine[0] * 
						((double)in2_pixels[y * rowstride + x * 4 + xch] / 255.0 - 0.5);
				else
					ox = x;

				if (ych != 4)
					oy = y + oself->scale * ctx->paffine[3] * 
						((double)in2_pixels[y * rowstride + x * 4 + ych] / 255.0 - 0.5);
				else
					oy = y;

				for (ch = 0; ch < 4; ch++)
					{
						output_pixels[y * rowstride + x * 4 + ch] =
							gdk_pixbuf_get_interp_pixel(in_pixels, ox, oy, ch, boundarys, 
														rowstride);
					}
			}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (in2));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_displacement_map_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveDisplacementMap *oself;
	
	oself = (RsvgFilterPrimitiveDisplacementMap *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_string_free (oself->in2, TRUE);
	g_free (oself);
}

void
rsvg_start_filter_primitive_displacement_map (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveDisplacementMap *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveDisplacementMap, 1);
	
	filter->super.in = g_string_new ("none");
	filter->in2 = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->xChannelSelector = ' ';
	filter->yChannelSelector = ' ';
	filter->scale = 0;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "in2"))
						g_string_assign (filter->in2, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "xChannelSelector"))
						filter->xChannelSelector = ((char *) atts[i + 1])[0];
					else if (!strcmp ((char *) atts[i], "yChannelSelector"))
						filter->yChannelSelector = ((char *) atts[i + 1])[0];
					else if (!strcmp ((char *) atts[i], "scale"))
						filter->scale = g_ascii_strtod(atts[i + 1], NULL);
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_displacement_map_render;
	filter->super.free = &rsvg_filter_primitive_displacement_map_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

/* Produces results in the range [1, 2**31 - 2].
   Algorithm is: r = (a * r) mod m
   where a = 16807 and m = 2**31 - 1 = 2147483647
   See [Park & Miller], CACM vol. 31 no. 10 p. 1195, Oct. 1988
   To test: the algorithm should produce the result 1043618065
   as the 10,000th generated number if the original seed is 1.
*/
#define feTurbulence_RAND_m 2147483647 /* 2**31 - 1 */
#define feTurbulence_RAND_a 16807 /* 7**5; primitive root of m */
#define feTurbulence_RAND_q 127773 /* m / a */
#define feTurbulence_RAND_r 2836 /* m % a */
#define feTurbulence_BSize 0x100
#define feTurbulence_BM 0xff
#define feTurbulence_PerlinN 0x1000
#define feTurbulence_NP 12 /* 2^PerlinN */
#define feTurbulence_NM 0xfff

typedef struct _RsvgFilterPrimitiveTurbulence RsvgFilterPrimitiveTurbulence;
struct _RsvgFilterPrimitiveTurbulence
{
	RsvgFilterPrimitive super;

	int uLatticeSelector[feTurbulence_BSize + feTurbulence_BSize + 2];
	double fGradient[4][feTurbulence_BSize + feTurbulence_BSize + 2][2];

	int seed;

	double fBaseFreqX;
	double fBaseFreqY;

	int nNumOctaves;
	gboolean bFractalSum;
	gboolean bDoStitching;
};

struct feTurbulence_StitchInfo
{
	int nWidth; /* How much to subtract to wrap for stitching. */
	int nHeight;
	int nWrapX; /* Minimum value to wrap. */
	int nWrapY;
};

static long feTurbulence_setup_seed(int lSeed)
{
	if (lSeed <= 0) 
		lSeed = -(lSeed % (feTurbulence_RAND_m - 1)) + 1;
	if (lSeed > feTurbulence_RAND_m - 1) 
		lSeed = feTurbulence_RAND_m - 1;
	return lSeed;
}

static long feTurbulence_random(int lSeed)
{
  long result;

  result = feTurbulence_RAND_a * (lSeed % feTurbulence_RAND_q) - feTurbulence_RAND_r * (lSeed / feTurbulence_RAND_q);
  if (result <= 0) 
	  result += feTurbulence_RAND_m;
  return result;
}

static void feTurbulence_init(RsvgFilterPrimitiveTurbulence *filter)
{
	double s;
	int i, j, k, lSeed;
	
	lSeed = feTurbulence_setup_seed(filter->seed);
	for(k = 0; k < 4; k++)
		{
			for(i = 0; i < feTurbulence_BSize; i++)
				{
					filter->uLatticeSelector[i] = i;
					for (j = 0; j < 2; j++)
						filter->fGradient[k][i][j] = (double)(((lSeed = feTurbulence_random(lSeed)) % (feTurbulence_BSize + feTurbulence_BSize)) - feTurbulence_BSize) / feTurbulence_BSize;
					s = (double)(sqrt(filter->fGradient[k][i][0] * filter->fGradient[k][i][0] + filter->fGradient[k][i][1] * filter->fGradient[k][i][1]));
					filter->fGradient[k][i][0] /= s;
					filter->fGradient[k][i][1] /= s;
				}
		}
	
	while(--i)
		{
			k = filter->uLatticeSelector[i];
			filter->uLatticeSelector[i] = filter->uLatticeSelector[j = (lSeed = feTurbulence_random(lSeed)) % feTurbulence_BSize];
			filter->uLatticeSelector[j] = k;
		}
	
	for(i = 0; i < feTurbulence_BSize + 2; i++)
		{
			filter->uLatticeSelector[feTurbulence_BSize + i] = filter->uLatticeSelector[i];
			for(k = 0; k < 4; k++)
				for(j = 0; j < 2; j++)
					filter->fGradient[k][feTurbulence_BSize + i][j] = filter->fGradient[k][i][j];
		}
}

#define feTurbulence_s_curve(t) ( t * t * (3. - 2. * t) )
#define feTurbulence_lerp(t, a, b) ( a + t * (b - a) )

static double feTurbulence_noise2(RsvgFilterPrimitiveTurbulence *filter,
								  int nColorChannel, double vec[2], 
								  struct feTurbulence_StitchInfo *pStitchInfo)
{
	int bx0, bx1, by0, by1, b00, b10, b01, b11;
	double rx0, rx1, ry0, ry1, *q, sx, sy, a, b, t, u, v;
	register int i, j;

	t = vec[0] + feTurbulence_PerlinN;
	bx0 = (int)t;
	bx1 = bx0+1;
	rx0 = t - (int)t;
	rx1 = rx0 - 1.0f;
	t = vec[1] + feTurbulence_PerlinN;
	by0 = (int)t;
	by1 = by0+1;
	ry0 = t - (int)t;
	ry1 = ry0 - 1.0f;

	/* If stitching, adjust lattice points accordingly. */
	if(pStitchInfo != NULL)
		{
			if(bx0 >= pStitchInfo->nWrapX)
				bx0 -= pStitchInfo->nWidth;
			if(bx1 >= pStitchInfo->nWrapX)
				bx1 -= pStitchInfo->nWidth;
			if(by0 >= pStitchInfo->nWrapY)
				by0 -= pStitchInfo->nHeight;
			if(by1 >= pStitchInfo->nWrapY)
				by1 -= pStitchInfo->nHeight;
		}

	bx0 &= feTurbulence_BM;
	bx1 &= feTurbulence_BM;
	by0 &= feTurbulence_BM;
	by1 &= feTurbulence_BM;
	i = filter->uLatticeSelector[bx0];
	j = filter->uLatticeSelector[bx1];
	b00 = filter->uLatticeSelector[i + by0];
	b10 = filter->uLatticeSelector[j + by0];
	b01 = filter->uLatticeSelector[i + by1];
	b11 = filter->uLatticeSelector[j + by1];
	sx = (double)(feTurbulence_s_curve(rx0));
	sy = (double)(feTurbulence_s_curve(ry0));
	q = filter->fGradient[nColorChannel][b00]; u = rx0 * q[0] + ry0 * q[1];
	q = filter->fGradient[nColorChannel][b10]; v = rx1 * q[0] + ry0 * q[1];
	a = feTurbulence_lerp(sx, u, v);
	q = filter->fGradient[nColorChannel][b01]; u = rx0 * q[0] + ry1 * q[1];
	q = filter->fGradient[nColorChannel][b11]; v = rx1 * q[0] + ry1 * q[1];
	b = feTurbulence_lerp(sx, u, v);

	return feTurbulence_lerp(sy, a, b);
}

static double feTurbulence_turbulence(RsvgFilterPrimitiveTurbulence *filter,
									  int nColorChannel, double *point, 
									  double fTileX, double fTileY, double fTileWidth, double fTileHeight)
{
	struct feTurbulence_StitchInfo stitch;
	struct feTurbulence_StitchInfo *pStitchInfo = NULL; /* Not stitching when NULL. */

	double fSum = 0.0f, vec[2], ratio = 1.;
	int nOctave;

	/* Adjust the base frequencies if necessary for stitching. */
	if(filter->bDoStitching)
		{
			/* When stitching tiled turbulence, the frequencies must be adjusted
			   so that the tile borders will be continuous. */
			if(filter->fBaseFreqX != 0.0)
				{
					double fLoFreq = (double)(floor(fTileWidth * filter->fBaseFreqX)) / fTileWidth;
					double fHiFreq = (double)(ceil(fTileWidth * filter->fBaseFreqX)) / fTileWidth;
					if(filter->fBaseFreqX / fLoFreq < fHiFreq / filter->fBaseFreqX)
						filter->fBaseFreqX = fLoFreq;
					else
						filter->fBaseFreqX = fHiFreq;
				}

			if(filter->fBaseFreqY != 0.0)
				{
					double fLoFreq = (double)(floor(fTileHeight * filter->fBaseFreqY)) / fTileHeight;
					double fHiFreq = (double)(ceil(fTileHeight * filter->fBaseFreqY)) / fTileHeight;
					if(filter->fBaseFreqY / fLoFreq < fHiFreq / filter->fBaseFreqY)
						filter->fBaseFreqY = fLoFreq;
					else
						filter->fBaseFreqY = fHiFreq;
				}

			/* Set up initial stitch values. */
			pStitchInfo = &stitch;
			stitch.nWidth = (int)(fTileWidth * filter->fBaseFreqX + 0.5f);
			stitch.nWrapX = fTileX * filter->fBaseFreqX + feTurbulence_PerlinN + stitch.nWidth;
			stitch.nHeight = (int)(fTileHeight * filter->fBaseFreqY + 0.5f);
			stitch.nWrapY = fTileY * filter->fBaseFreqY + feTurbulence_PerlinN + stitch.nHeight;
		}

	vec[0] = point[0] * filter->fBaseFreqX;
	vec[1] = point[1] * filter->fBaseFreqY;

	for(nOctave = 0; nOctave < filter->nNumOctaves; nOctave++)
		{
			if(filter->bFractalSum)
				fSum += (double)(feTurbulence_noise2(filter, nColorChannel, vec, pStitchInfo) / ratio);
			else
				fSum += (double)(fabs(feTurbulence_noise2(filter, nColorChannel, vec, pStitchInfo)) / ratio);

			vec[0] *= 2;
			vec[1] *= 2;
			ratio *= 2;

			if(pStitchInfo != NULL)
				{
					/* Update stitch values. Subtracting PerlinN before the multiplication and
					   adding it afterward simplifies to subtracting it once. */
					stitch.nWidth *= 2;
					stitch.nWrapX = 2 * stitch.nWrapX - feTurbulence_PerlinN;
					stitch.nHeight *= 2;
					stitch.nWrapY = 2 * stitch.nWrapY - feTurbulence_PerlinN;
				}
		}

	return fSum;
}

static void
rsvg_filter_primitive_turbulence_render (RsvgFilterPrimitive * self,
										 RsvgFilterContext * ctx)
{
	RsvgFilterPrimitiveTurbulence *oself;
	gint x, y, tileWidth, tileHeight, rowstride, width, height;
	FPBox boundarys;
	guchar *output_pixels;
	GdkPixbuf *output;

	GdkPixbuf *in;
	
	in = rsvg_filter_get_in (self->in, ctx);
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	rowstride = gdk_pixbuf_get_rowstride (in);

	oself = (RsvgFilterPrimitiveTurbulence *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	tileWidth = (boundarys.x2 - boundarys.x1);
	tileHeight = (boundarys.y2 - boundarys.y1);

	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	output_pixels = gdk_pixbuf_get_pixels (output);

	for (y = 0; y < tileHeight; y++)
		{
			for (x = 0; x < tileWidth; x++)
				{
					gint i;
					double point[2] = {x+boundarys.x1, y+boundarys.y1};
					
					for (i = 0; i < 4; i++)
						{
							double cr;
							
							cr = feTurbulence_turbulence(oself, i, point, (double)x, (double)y, (double)tileWidth, (double)tileHeight);
							
							if(oself->bFractalSum)
								cr = ((cr * 255.) + 255.) / 2.;
							else
								cr = (cr * 255.);

							cr = CLAMP(cr, 0., 255.);

							output_pixels[(4 * (x+boundarys.x1)) + ((y+boundarys.y1) * rowstride) + i] = (guchar)(cr);
						}
				}
		}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_turbulence_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveTurbulence *oself;
	
	oself = (RsvgFilterPrimitiveTurbulence *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (oself);
}

void
rsvg_start_filter_primitive_turbulence (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveTurbulence *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveTurbulence, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->fBaseFreqX = 0;
	filter->fBaseFreqY = 0;
	filter->nNumOctaves = 1;
	filter->seed = 0;
	filter->bDoStitching = 0;
	filter->bFractalSum = 0;

	feTurbulence_init(filter);

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "baseFrequency"))
						rsvg_css_parse_number_optional_number((char *) atts[i + 1], &filter->fBaseFreqX, &filter->fBaseFreqY);
					else if (!strcmp ((char *) atts[i], "numOctaves"))
						filter->nNumOctaves = atoi((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "seed"))
						filter->seed = atoi((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "stitchTiles"))
						filter->bDoStitching = (!strcmp((char *) atts[i + 1], "stitch"));
					else if (!strcmp ((char *) atts[i], "type"))
						filter->bFractalSum = (!strcmp((char *) atts[i + 1], "fractalNoise"));
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_turbulence_render;
	filter->super.free = &rsvg_filter_primitive_turbulence_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveImage RsvgFilterPrimitiveImage;

struct _RsvgFilterPrimitiveImage
{
	RsvgFilterPrimitive super;
	GString *href;
};

#ifndef GDK_PIXBUF_CHECK_VERSION
#define GDK_PIXBUF_CHECK_VERSION(major,minor,micro)    \
    (GDK_PIXBUF_MAJOR > (major) || \
     (GDK_PIXBUF_MAJOR == (major) && GDK_PIXBUF_MINOR > (minor)) || \
     (GDK_PIXBUF_MAJOR == (major) && GDK_PIXBUF_MINOR == (minor) && \
      GDK_PIXBUF_MICRO >= (micro)))
#endif

static void
rsvg_filter_primitive_image_render (RsvgFilterPrimitive * self,
									RsvgFilterContext * ctx)
{
	FPBox boundarys;
	gint width, height;
	
	RsvgFilterPrimitiveImage *oself;
	
	GdkPixbuf *output, *in, *img;
	
	oself = (RsvgFilterPrimitiveImage *) self;

	if(!oself->href)
		return;

	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);

#if GDK_PIXBUF_CHECK_VERSION(2,3,2)
	img = gdk_pixbuf_new_from_file_at_size(oself->href->str,
										   boundarys.x2 - boundarys.x1,
										   boundarys.y2 - boundarys.y1,
										   NULL);
#else
	img = gdk_pixbuf_new_from_file(oself->href->str, NULL);
	if(img)
		{
			GdkPixbuf *scaled;

			scaled = gdk_pixbuf_scale_simple(img, boundarys.x2 - boundarys.x1,
											 boundarys.y2 - boundarys.y1,
											 GDK_INTERP_BILINEAR);

			g_object_unref (G_OBJECT (img));
			img = scaled;
		}
#endif

	if(img)
		{
			gdk_pixbuf_copy_area (img, 0, 0, gdk_pixbuf_get_width(img), gdk_pixbuf_get_height(img),
								  output, boundarys.x1, boundarys.y1);
			g_object_unref (G_OBJECT (img));
		}

	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_image_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveImage *oself;
	
	oself = (RsvgFilterPrimitiveImage *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);

	if(oself->href)
		g_string_free (oself->href, TRUE);

	g_free (oself);
}

void
rsvg_start_filter_primitive_image (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveImage *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveImage, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "xlink:href"))
						{
							filter->href = g_string_new (NULL);
							g_string_assign (filter->href, (char *) atts[i + 1]);
						}
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_image_render;
	filter->super.free = &rsvg_filter_primitive_image_free;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

/*************************************************************/
/*************************************************************/


typedef struct _FactorAndMatrix FactorAndMatrix;

struct _FactorAndMatrix
{
	gint matrix[9];
	gdouble factor;
};

typedef struct _vector3 vector3;

struct _vector3
{
	gdouble x;
	gdouble y;
	gdouble z;
};

static gdouble
norm (vector3 A)
{
	return sqrt(A.x*A.x + A.y*A.y + A.z*A.z);
}

static gdouble
dotproduct (vector3 A, vector3 B)
{
	return A.x*B.x + A.y*B.y + A.z*B.z;
}

static vector3
normalise (vector3 A)
{
	double divisor;
	divisor = norm(A);

	A.x /= divisor;
	A.y /= divisor;
	A.z /= divisor;

	return A;
}

static FactorAndMatrix
get_light_normal_matrix_x (gint n)
{
	const static FactorAndMatrix matrix_list [] =
		{
			{
				{0,  0,  0,
				 0, -2,  2,
				 0, -1,  1},
				2.0/3.0
			},
			{
				{0,  0,  0,
				 -2,  0,  2,
				 -1,  0,  1},
				1.0/3.0
			},
			{
				{0,  0,  0,
				 -2,  2,  0,
				 -1,  1,  0},
				2.0/3.0
			},
			{
				{0, -1,  1,
				 0, -2,  2,
				 0, -1,  1},
				1.0/2.0
			},
			{
				{-1,  0,  1,
				 -2,  0,  2,
				 -1,  0,  1},
				1.0/4.0
			},
			{
				{-1,  1,  0,
				 -2,  2,  0,
				 -1,  1,  0},
				1.0/2.0
			},
			{
				{0, -1,  1,
				 0, -2,  2,
				 0,  0,  0},
				2.0/3.0
			},
			{
				{-1,  0,  1,
				 -2,  0,  2,
				 0,  0,  0},
				1.0/3.0
			},
			{
				{-1,  1,  0,
				 -2,  2,  0,
				 0,  0,  0},
				2.0/3.0
			}
		};

	return matrix_list[n];
}

static FactorAndMatrix
get_light_normal_matrix_y (gint n)
{
	const static FactorAndMatrix matrix_list [] =
		{
			{
				{0,  0,  0,
				 0, -2, -1,
				 0,  2,  1},
				2.0/3.0
			},
			{
				{0,  0,  0,
				 -1, -2, -1,
				 1,  2,  1},
				1.0/3.0
			},
			{
				{0,  0,  0,
				 -1, -2,  0,
				 1,  2,  0},
				2.0/3.0
			},
			{
				
				{0, -2, -1,
				 0,  0,  0,
				 0,  2,  1},
				1.0/2.0
			},
			{
				{-1, -2, -1,
				 0,  0,  0,
				 1,  2,  1},
				1.0/4.0
			},
			{
				{-1, -2,  0,
				 0,  0,  0,
				 1,  2,  0},
				1.0/2.0
			},
			{
				
				{0, -2, -1,
				 0,  2,  1,
				 0,  0,  0},
				2.0/3.0
			},
			{
				{0, -2, -1,
				 1,  2,  1,
				 0,  0,  0},
				1.0/3.0
			},
			{
				{-1, -2,  0,
				 1,  2,  0,
				 0,  0,  0},
				2.0/3.0
				
			}		
		};

	return matrix_list[n];
}

static vector3
get_surface_normal (guchar * I, FPBox boundarys, gint x, gint y, 
					gint dx, gint dy, gdouble surfaceScale, gint rowstride)
{
	gint mrow, mcol;
	FactorAndMatrix fnmx, fnmy;
	gint *Kx, *Ky;
	gdouble factorx, factory;
	gdouble Nx, Ny;
	vector3 output;

	if (x + 1 >= boundarys.x2)
		mcol = 2;
	else if (x - 1 < boundarys.x1)
		mcol = 0;
	else
		mcol = 1;
	if (y + 1 >= boundarys.y2)
		mrow = 2;
	else if (y - 1 < boundarys.y1)
		mrow = 0;
	else
		mrow = 1;

	fnmx = get_light_normal_matrix_x(mrow * 3 + mcol);
	factorx = fnmx.factor / (gdouble)dx;
	Kx = fnmx.matrix;

	fnmy = get_light_normal_matrix_y(mrow * 3 + mcol);
	factory = fnmy.factor / (gdouble)dy;
	Ky = fnmy.matrix;	

    Nx = -surfaceScale * factorx * (gdouble)
		(Kx[0]*I[(x-dx) * 4 + 3 + (y-dy) * rowstride] + 
		 Kx[1]*I[(x)    * 4 + 3 + (y-dy) * rowstride] + 
		 Kx[2]*I[(x+dx) * 4 + 3 + (y-dx) * rowstride] +
		 Kx[3]*I[(x-dx) * 4 + 3 + (y)    * rowstride] + 
		 Kx[4]*I[(x)    * 4 + 3 + (y)    * rowstride] + 
		 Kx[5]*I[(x+dx) * 4 + 3 + (y)    * rowstride] +
		 Kx[6]*I[(x-dx) * 4 + 3 + (y+dy) * rowstride] + 
		 Kx[7]*I[(x)    * 4 + 3 + (y+dy) * rowstride] + 
		 Kx[8]*I[(x+dx) * 4 + 3 + (y+dy) * rowstride]) / 255.0;
	
    Ny = -surfaceScale * factory * (gdouble)
		(Ky[0]*I[(x-dx) * 4 + 3 + (y-dy) * rowstride] + 
		 Ky[1]*I[(x)    * 4 + 3 + (y-dy) * rowstride] + 
		 Ky[2]*I[(x+dx) * 4 + 3 + (y-dx) * rowstride] +
		 Ky[3]*I[(x-dx) * 4 + 3 + (y)    * rowstride] + 
		 Ky[4]*I[(x)    * 4 + 3 + (y)    * rowstride] + 
		 Ky[5]*I[(x+dx) * 4 + 3 + (y)    * rowstride] +
		 Ky[6]*I[(x-dx) * 4 + 3 + (y+dy) * rowstride] + 
		 Ky[7]*I[(x)    * 4 + 3 + (y+dy) * rowstride] + 
		 Ky[8]*I[(x+dx) * 4 + 3 + (y+dy) * rowstride]) / 255.0;

	output.x = Nx;
	output.y = Ny;
	output.z = 1;
	output = normalise(output);
	return output;
}

typedef enum {
	DISTANTLIGHT, POINTLIGHT, SPOTLIGHT
} lightType;

typedef struct _lightSource lightSource;

struct _lightSource
{
	lightType type;
	gdouble x; /*doubles as azimuth*/
	gdouble y; /*dounles as elevation*/
	gdouble z;
	gdouble pointsAtX;
	gdouble pointsAtY;
	gdouble pointsAtZ;
	gdouble specularExponent;
	gdouble limitingconeAngle;
};

static vector3
get_light_direction (lightSource source, gdouble x, gdouble y, gdouble z)
{
	vector3 output;

	switch (source.type)
		{
		case DISTANTLIGHT:
			output.x = cos(source.x)*cos(source.y);
			output.y = sin(source.x)*cos(source.y);
			output.z = sin(source.y);
			break;
		case POINTLIGHT:
		case SPOTLIGHT:
			output.x = source.x - x;
			output.y = source.y - y;
			output.z = source.z - z;
			output = normalise(output);
			break;
		}
	return output;
}

static vector3
get_light_colour(lightSource source, vector3 colour, 
				 gdouble x, gdouble y, gdouble z)
{
	double base;
	vector3 s;
	vector3 output;

	if (source.type != SPOTLIGHT)
		return colour;

	s.x = source.pointsAtX - source.x;
	s.y = source.pointsAtY - source.y;
	s.z = source.pointsAtZ - source.z;
	s = normalise(s);

	base = -dotproduct(get_light_direction (source, x, y, z), s);

	if (base < 0)
		{
			output.x = 0;
			output.y = 0;
			output.z = 0;
			return output;
		}
	
	output.x = colour.x*pow(base, source.specularExponent);
	output.y = colour.x*pow(base, source.specularExponent);
	output.z = colour.x*pow(base, source.specularExponent);

	return output;
}


void 
rsvg_start_filter_primitive_light_source (RsvgHandle * ctx,
										  const xmlChar ** atts, char type)
{
	lightSource * data;
	gint i;

	data = (lightSource *)ctx->currentsubfilter;

	if (type == 's')
		data->type = SPOTLIGHT;
	else if (type == 'd')
		data->type = DISTANTLIGHT;
	else 
		data->type = POINTLIGHT;

	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "azimuth"))
						{
							data->x = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "elevation"))
						{
							data->y = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "x"))
						{
							data->x = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							data->y = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "z"))
						{
							data->z = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "pointsAtX"))
						{
							data->pointsAtX = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "pointsAtY"))
						{
							data->pointsAtY = g_ascii_strtod(atts[i + 1], NULL); 
						}
					else if (!strcmp ((char *) atts[i], "pointsAtZ"))
						{
							data->pointsAtZ = g_ascii_strtod(atts[i + 1], NULL); 
						}
				}
		}
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveDiffuseLighting RsvgFilterPrimitiveDiffuseLighting;

struct _RsvgFilterPrimitiveDiffuseLighting
{
	RsvgFilterPrimitive super;
	gdouble dx, dy;
	double diffuseConstant;
	double surfaceScale;
	lightSource source;
	guint32 lightingcolour;
};

static void
rsvg_filter_primitive_diffuse_lighting_render (RsvgFilterPrimitive * self,
											   RsvgFilterContext * ctx)
{
	gint x, y;
	gdouble z;
	gint rowstride, height, width;
	gdouble factor;
	vector3 lightcolour;
	vector3 colour;

	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveDiffuseLighting *oself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	oself = (RsvgFilterPrimitiveDiffuseLighting *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	colour.x = ((guchar *)(&oself->lightingcolour))[2] / 255.0;
	colour.y = ((guchar *)(&oself->lightingcolour))[1] / 255.0;
	colour.z = ((guchar *)(&oself->lightingcolour))[0] / 255.0;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				z = oself->surfaceScale * in_pixels[y * rowstride + x * 4 + 3] / 255.0;
				lightcolour = get_light_colour(oself->source, colour, x, y, z);
				factor = dotproduct(get_surface_normal(in_pixels, boundarys, x, y, 
													   oself->dx, oself->dy, oself->surfaceScale, 
													   rowstride),
									get_light_direction(oself->source, x, y, z));

				output_pixels[y * rowstride + x * 4    ] = oself->diffuseConstant * factor * 
					lightcolour.x * 255.0;
				output_pixels[y * rowstride + x * 4 + 1] = oself->diffuseConstant * factor * 
					lightcolour.y * 255.0;
				output_pixels[y * rowstride + x * 4 + 2] = oself->diffuseConstant * factor * 
					lightcolour.z * 255.0;
				output_pixels[y * rowstride + x * 4 + 3] = 255;
			}
	
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_diffuse_lighting_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveDiffuseLighting *oself;
	
	oself = (RsvgFilterPrimitiveDiffuseLighting *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (oself);
}

void
rsvg_start_filter_primitive_diffuse_lighting (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveDiffuseLighting *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveDiffuseLighting, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->surfaceScale = 1;
	filter->diffuseConstant = 1;
	filter->dx = 1;
	filter->dy = 1;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "kernelUnitLength"))
						rsvg_css_parse_number_optional_number ((char *) atts[i + 1],
															   &filter->dx, &filter->dy);
					else if (!strcmp ((char *) atts[i], "lighting-color"))
						filter->lightingcolour = rsvg_css_parse_color ((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "diffuseConstant"))
						filter->diffuseConstant = 
							g_ascii_strtod(atts[i + 1], NULL);
					else if (!strcmp ((char *) atts[i], "surfaceScale"))
						filter->surfaceScale = 
							g_ascii_strtod(atts[i + 1], NULL);
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_diffuse_lighting_render;
	filter->super.free = &rsvg_filter_primitive_diffuse_lighting_free;
	ctx->currentsubfilter = &filter->source;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}


/*************************************************************/
/*************************************************************/

typedef struct _RsvgFilterPrimitiveSpecularLighting RsvgFilterPrimitiveSpecularLighting;

struct _RsvgFilterPrimitiveSpecularLighting
{
	RsvgFilterPrimitive super;
	double specularConstant;
	double specularExponent;
	double surfaceScale;
	lightSource source;
	guint32 lightingcolour;
};

static void
rsvg_filter_primitive_specular_lighting_render (RsvgFilterPrimitive * self,
											   RsvgFilterContext * ctx)
{
	gint x, y, temp;
	gdouble z;
	gint rowstride, height, width;
	gdouble factor, max;
	vector3 lightcolour;
	vector3 colour;
	vector3 L;

	FPBox boundarys;
	
	guchar *in_pixels;
	guchar *output_pixels;
	
	RsvgFilterPrimitiveSpecularLighting *oself;
	
	GdkPixbuf *output;
	GdkPixbuf *in;
	
	oself = (RsvgFilterPrimitiveSpecularLighting *) self;
	boundarys = rsvg_filter_primitive_get_bounds (self, ctx);
	
	in = rsvg_filter_get_in (self->in, ctx);
	in_pixels = gdk_pixbuf_get_pixels (in);
	
	height = gdk_pixbuf_get_height (in);
	width = gdk_pixbuf_get_width (in);
	
	rowstride = gdk_pixbuf_get_rowstride (in);
	
	output = gdk_pixbuf_new_cleared (GDK_COLORSPACE_RGB, 1, 8, width, height);
	
	output_pixels = gdk_pixbuf_get_pixels (output);
	
	colour.x = ((guchar *)(&oself->lightingcolour))[2] / 255.0;
	colour.y = ((guchar *)(&oself->lightingcolour))[1] / 255.0;
	colour.z = ((guchar *)(&oself->lightingcolour))[0] / 255.0;

	for (y = boundarys.y1; y < boundarys.y2; y++)
		for (x = boundarys.x1; x < boundarys.x2; x++)
			{
				z = oself->surfaceScale * in_pixels[y * rowstride + x * 4 + 3] / 255.0;
				L = get_light_direction(oself->source, x, y, z);
				L.z += 1;
				L = normalise(L);

				lightcolour = get_light_colour(oself->source, colour, x, y, z);
				factor = dotproduct(get_surface_normal(in_pixels, boundarys, x, y, 
													   1, 1, oself->surfaceScale, 
													   rowstride), L);

				max = 0;
				temp = oself->specularConstant * 
					pow(factor, oself->specularExponent) * lightcolour.x * 255.0;		
				if (temp < 0)
					temp = 0;				
				if (temp > 255)
					temp = 255;
				max = MAX(temp, max);
				output_pixels[y * rowstride + x * 4    ] = temp;
				temp = oself->specularConstant * 
					pow(factor, oself->specularExponent) * lightcolour.y * 255.0;
				if (temp < 0)
					temp = 0;				
				if (temp > 255)
					temp = 255;
				max = MAX(temp, max);
				output_pixels[y * rowstride + x * 4 + 1] = temp;
				temp = oself->specularConstant * 
					pow(factor, oself->specularExponent) * lightcolour.z * 255.0;
				if (temp < 0)
					temp = 0;				
				if (temp > 255)
					temp = 255;
				max = MAX(temp, max);		
				output_pixels[y * rowstride + x * 4 + 2] = temp;

				output_pixels[y * rowstride + x * 4 + 3] = max;
			}
	
	rsvg_filter_store_result (self->result, output, ctx);
	
	g_object_unref (G_OBJECT (in));
	g_object_unref (G_OBJECT (output));
}

static void
rsvg_filter_primitive_specular_lighting_free (RsvgFilterPrimitive * self)
{
	RsvgFilterPrimitiveSpecularLighting *oself;
	
	oself = (RsvgFilterPrimitiveSpecularLighting *) self;
	g_string_free (self->result, TRUE);
	g_string_free (self->in, TRUE);
	g_free (oself);
}

void
rsvg_start_filter_primitive_specular_lighting (RsvgHandle * ctx, const xmlChar ** atts)
{
	int i;
	
	double font_size;
	RsvgFilterPrimitiveSpecularLighting *filter;
	
	font_size = rsvg_state_current_font_size (ctx);
	
	filter = g_new (RsvgFilterPrimitiveSpecularLighting, 1);
	
	filter->super.in = g_string_new ("none");
	filter->super.result = g_string_new ("none");
	filter->super.sizedefaults = 1;
	filter->surfaceScale = 1;
	filter->specularConstant = 1;
	filter->specularExponent = 1;
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *) atts[i], "in"))
						g_string_assign (filter->super.in, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "result"))
						g_string_assign (filter->super.result, (char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "x"))
						{
							filter->super.x =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "y"))
						{
							filter->super.y =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "width"))
						{
							filter->super.width =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "height"))
						{
							filter->super.height =
								rsvg_css_parse_normalized_length ((char *) atts[i + 1],
																  ctx->dpi,
																  1,
																  font_size);
							filter->super.sizedefaults = 0;
						}
					else if (!strcmp ((char *) atts[i], "lighting-color"))
						filter->lightingcolour = rsvg_css_parse_color ((char *) atts[i + 1]);
					else if (!strcmp ((char *) atts[i], "specularConstant"))
						filter->specularConstant = 
							g_ascii_strtod(atts[i + 1], NULL);
					else if (!strcmp ((char *) atts[i], "specularExponent"))
						filter->specularExponent = 
							g_ascii_strtod(atts[i + 1], NULL);
					else if (!strcmp ((char *) atts[i], "surfaceScale"))
						filter->surfaceScale = 
							g_ascii_strtod(atts[i + 1], NULL);
				}
		}
	
	filter->super.render = &rsvg_filter_primitive_specular_lighting_render;
	filter->super.free = &rsvg_filter_primitive_specular_lighting_free;
	ctx->currentsubfilter = &filter->source;
	
	g_ptr_array_add (((RsvgFilter *) (ctx->currentfilter))->primitives,
					 &filter->super);
}

