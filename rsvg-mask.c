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

#include "rsvg-private.h"
#include "rsvg-mask.h"
#include "rsvg-styles.h"
#include "rsvg-art-draw.h"
#include "rsvg-css.h"
#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_svp_ops.h>
#include <string.h>

static void 
rsvg_mask_free (RsvgDefVal * self)
{
	RsvgMask *z = (RsvgMask *)self;
	g_ptr_array_free(z->super.children, TRUE);
	rsvg_state_finalize (&z->super.super.state);
	g_free (z);
}

void 
rsvg_mask_render (RsvgMask *self, GdkPixbuf *tos, GdkPixbuf *nos, RsvgDrawingCtx *ctx)
{
	art_u8 *tos_pixels, *nos_pixels, *mask_pixels;
	int width;
	int height;
	int rowstride;
	int x, y;
	
	GdkPixbuf *save, *mask;
	RsvgDefsDrawable *drawable;	

	drawable = (RsvgDefsDrawable*)self;
	
	mask = _rsvg_pixbuf_new_cleared(GDK_COLORSPACE_RGB, 1, 8, 
									gdk_pixbuf_get_width(tos), 
									gdk_pixbuf_get_height(tos));
	save = ctx->pixbuf;

	ctx->pixbuf = mask;

	rsvg_state_push(ctx);
	rsvg_defs_drawable_draw (drawable, ctx, 0);
	rsvg_state_pop(ctx);

	ctx->pixbuf = save;

	if (tos == NULL || nos == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	if (!gdk_pixbuf_get_has_alpha (nos))
		{
			g_warning (_("push/pop transparency group on non-alpha buffer nyi"));
			return;
		}
	
	width = gdk_pixbuf_get_width (tos);
	height = gdk_pixbuf_get_height (tos);
	rowstride = gdk_pixbuf_get_rowstride (tos);
	
	tos_pixels = gdk_pixbuf_get_pixels (tos);
	nos_pixels = gdk_pixbuf_get_pixels (nos);
	mask_pixels = gdk_pixbuf_get_pixels (mask);
	
	for (y = 0; y < height; y++)
		{
			for (x = 0; x < width; x++)
				{
					guchar r, g, b, rm, gm, bm, am;
					guint a;
					guint luminance;
					a = tos_pixels[4 * x + 3];
					if (a)
						{
							r = tos_pixels[4 * x];
							g = tos_pixels[4 * x + 1];
							b = tos_pixels[4 * x + 2];

							rm = mask_pixels[4 * x];
							gm = mask_pixels[4 * x + 1];
							bm = mask_pixels[4 * x + 2];
							am = mask_pixels[4 * x + 3];

							luminance = (rm * 2125 + gm * 7154 + bm * 0721) / 10000;

							a = a * luminance / 255 * am / 255;

							art_rgba_run_alpha (nos_pixels + 4 * x, r, g, b, a, 1);
						}
				}
			tos_pixels += rowstride;
			nos_pixels += rowstride;
			mask_pixels += rowstride;
		}
	g_object_unref (G_OBJECT (mask));
}

static void 
rsvg_defs_drawable_mask_draw (RsvgDefsDrawable * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	rsvg_state_reinherit_top(ctx, &self->state, 0);

	if (state->opacity != 0xff || state->filter)
		rsvg_push_discrete_layer (ctx);

	for (i = 0; i < group->children->len; i++)
		{
			rsvg_state_push(ctx);

			rsvg_defs_drawable_draw (g_ptr_array_index(group->children, i), 
									 ctx, 0);
	
			rsvg_state_pop(ctx);
		}			

	if (state->opacity != 0xff || state->filter)
		rsvg_pop_discrete_layer (ctx);
}


static RsvgMask *
rsvg_new_mask (void)
{
	RsvgMask *mask;
	
	mask = g_new (RsvgMask, 1);
	mask->maskunits = objectBoundingBox;
	mask->contentunits = userSpaceOnUse;
	mask->x = 0;
	mask->y = 0;
	mask->width = 1;
	mask->height = 1;
	mask->super.children = g_ptr_array_new ();

	return mask;
}

void 
rsvg_start_mask (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *id = NULL, *klazz = NULL, *value;
	RsvgMask *mask;
	double font_size;
	
	font_size = rsvg_state_current_font_size (ctx);
	mask = rsvg_new_mask ();
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "maskUnits")))
				{
					if (!strcmp (value, "userSpaceOnUse"))
						mask->maskunits = userSpaceOnUse;
					else
						mask->maskunits = objectBoundingBox;
				}
			if ((value = rsvg_property_bag_lookup (atts, "maskContentUnits")))
				{
					if (!strcmp (value, "objectBoundingBox"))
						mask->contentunits = objectBoundingBox;
					else
						mask->contentunits = userSpaceOnUse;
				}
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				mask->x =
					rsvg_css_parse_normalized_length (value,
													  ctx->dpi_x,
													  1,
													  font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				mask->y =
					rsvg_css_parse_normalized_length (value,
													  ctx->dpi_y,
													  1,
													  font_size);
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				mask->width =
					rsvg_css_parse_normalized_length (value,
													  ctx->dpi_x,
													  1,
													  font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				mask->height =
					rsvg_css_parse_normalized_length (value,
													  ctx->dpi_y,
													  1,
													  font_size);					
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
		}


	rsvg_state_init(&mask->super.super.state);
	rsvg_parse_style_attrs (ctx, &mask->super.super.state, "mask", klazz, id, atts);

	mask->super.super.parent = (RsvgDefsDrawable *)ctx->current_defs_group;

	ctx->current_defs_group = &mask->super;
	
	/* set up the defval stuff */
	mask->super.super.super.type = RSVG_DEF_MASK;
	mask->super.super.super.free = &rsvg_mask_free;
	mask->super.super.draw = &rsvg_defs_drawable_mask_draw;

	rsvg_defs_set (ctx->defs, id, &mask->super.super.super);
}

void 
rsvg_end_mask (RsvgHandle *ctx)
{
	ctx->current_defs_group = ((RsvgDefsDrawable *)ctx->current_defs_group)->parent;
}

RsvgDefsDrawable *
rsvg_mask_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p))
				p++;

			for (ix = 0; p[ix]; ix++)
				if (p[ix] == ')')
					break;
			
			if (p[ix] == ')')
				{
					name = g_strndup (p, ix);
					val = rsvg_defs_lookup (defs, name);
					g_free (name);
					
					if (val && val->type == RSVG_DEF_MASK)
						return (RsvgDefsDrawable *) val;
				}
		}
	return NULL;
}

static void 
rsvg_clip_path_free (RsvgDefVal * self)
{
	RsvgClipPath *z = (RsvgClipPath *)self;
	g_ptr_array_free(z->super.children, TRUE);
	rsvg_state_finalize (&z->super.super.state);
	g_free (z);
}

ArtSVP *
rsvg_clip_path_render (RsvgClipPath * self, RsvgDrawingCtx *ctx)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	ArtSVP *svp, *svpx;
	svpx = NULL;
	
	rsvg_state_reinherit_top(ctx, &self->super.super.state, 0);

	if (self->units == objectBoundingBox)
		{
			state->affine[0] = ctx->bbox.x1 - ctx->bbox.x0;
			state->affine[1] = 0;
			state->affine[2] = 0;
			state->affine[3] = ctx->bbox.y1 - ctx->bbox.y0;
			state->affine[4] = ctx->bbox.x0;
			state->affine[5] = ctx->bbox.y0;
		}

	for (i = 0; i < group->children->len; i++)
		{
			rsvg_state_push(ctx);

			svp = rsvg_defs_drawable_draw_as_svp (g_ptr_array_index(group->children, i), 
												  ctx, 0);
			
			if (svp != NULL)
				{
					if (svpx != NULL)
						{
							ArtSVP * svpn;
							svpn = art_svp_union(svpx, svp);
							art_free(svpx);
							art_free(svp);
							svpx = svpn;
						}
					else
						svpx = svp;
				}

			rsvg_state_pop(ctx);
		}

	return svpx;
}


static RsvgClipPath *
rsvg_new_clip_path (void)
{
	RsvgClipPath *clip_path;
	
	clip_path = g_new (RsvgClipPath, 1);
	clip_path->super.children = g_ptr_array_new ();
	clip_path->units = userSpaceOnUse;
	return clip_path;
}

void 
rsvg_start_clip_path (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *id = NULL, *klazz = NULL, *value = NULL;
	RsvgClipPath *clip_path;
	double font_size;
	
	font_size = rsvg_state_current_font_size (ctx);
	clip_path = rsvg_new_clip_path ();
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "clipPathUnits")))
				{
					if (!strcmp (value, "objectBoundingBox"))
						clip_path->units = objectBoundingBox;
					else
						clip_path->units = userSpaceOnUse;		
				}				
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
		}

	rsvg_state_init (&clip_path->super.super.state);

	rsvg_parse_style_attrs (ctx, &clip_path->super.super.state, "clipPath", klazz, id, atts);

	clip_path->super.super.parent = (RsvgDefsDrawable *)ctx->current_defs_group;

	ctx->current_defs_group = &clip_path->super;
	
	/* set up the defval stuff */
	clip_path->super.super.super.type = RSVG_DEF_CLIP_PATH;
	clip_path->super.super.super.free = &rsvg_clip_path_free;
	rsvg_defs_set (ctx->defs, id, &clip_path->super.super.super);
}

void 
rsvg_end_clip_path (RsvgHandle *ctx)
{
	ctx->current_defs_group = ((RsvgDefsDrawable *)ctx->current_defs_group)->parent;
}

RsvgDefsDrawable *
rsvg_clip_path_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgDefVal *val;
			
			while (g_ascii_isspace (*p))
				p++;

			for (ix = 0; p[ix]; ix++)
				if (p[ix] == ')')
					break;
			
			if (p[ix] == ')')
				{
					name = g_strndup (p, ix);
					val = rsvg_defs_lookup (defs, name);
					g_free (name);
					
					if (val && val->type == RSVG_DEF_CLIP_PATH)
						return (RsvgDefsDrawable *) val;
				}
		}
	return NULL;
}

ArtSVP *
rsvg_rect_clip_path(double x, double y, double w, double h, RsvgDrawingCtx * ctx)
{	
	GString * d = NULL;
	ArtSVP * output = NULL;
	char buf [G_ASCII_DTOSTR_BUF_SIZE];

	/* emulate a rect using a path */
	d = g_string_new ("M ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x + w));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y+h));

	g_string_append (d, " H ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));

	g_string_append (d, " V ");
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));

	g_string_append (d, " Z");

	output = rsvg_render_path_as_svp (ctx, d->str);
	g_string_free (d, TRUE);
	return output;
}

ArtSVP *
rsvg_clip_path_merge(ArtSVP * first, ArtSVP * second, char operation)
{
	ArtSVP * tmppath;
	if (first != NULL && second != NULL)
		{
			if (operation == 'i')
				tmppath = art_svp_intersect(first, second);
			else
				tmppath = art_svp_union(first, second);
			art_free(first);
			art_free(second);
			return tmppath;
		}
	else if (first != NULL)
		return first;
	else
		return second;
}
