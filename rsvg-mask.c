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
#include "rsvg-shapes.h"
#include "rsvg-css.h"
#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_svp_ops.h>
#include <string.h>

static void 
rsvg_mask_free (RsvgDefVal * self)
{
	RsvgMask *z = (RsvgMask *)self;
	g_ptr_array_free(z->super.children, TRUE);
	g_free (z);
}

void 
rsvg_mask_render (RsvgMask *self, GdkPixbuf *tos, GdkPixbuf *nos, RsvgHandle *ctx)
{
	art_u8 *tos_pixels, *nos_pixels, *mask_pixels;
	int width;
	int height;
	int rowstride;
	int x, y;
	
	GdkPixbuf *save, *mask;
	RsvgDefsDrawable *drawable;	

	drawable = (RsvgDefsDrawable*)self;
	
	mask = gdk_pixbuf_new (GDK_COLORSPACE_RGB, 1, 8, 
			       gdk_pixbuf_get_width(tos), 
			       gdk_pixbuf_get_height(tos));

	gdk_pixbuf_fill(mask, 0x00000000);	
	save = ctx->pixbuf;

	ctx->pixbuf = mask;


/* push the state stack */
	if (ctx->n_state == ctx->n_state_max)
		ctx->state = g_renew (RsvgState, ctx->state, 
							  ctx->n_state_max <<= 1);
	if (ctx->n_state)
		rsvg_state_inherit (&ctx->state[ctx->n_state],
									&ctx->state[ctx->n_state - 1]);
	else
				rsvg_state_init (ctx->state);
	ctx->n_state++;
	
	rsvg_defs_drawable_draw (drawable, ctx, 0);
	
	/* pop the state stack */
	ctx->n_state--;
	rsvg_state_finalize (&ctx->state[ctx->n_state]);

	

	ctx->pixbuf = save;

	if (tos == NULL || nos == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}
	
	if (!gdk_pixbuf_get_has_alpha (nos))
		{
			g_warning ("push/pop transparency group on non-alpha buffer nyi");
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
					gdouble luminance;
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

							luminance = ((gdouble)rm * 0.2125 + 
										 (gdouble)gm * 0.7154 + 
										 (gdouble)bm * 0.0721) / 255.;

							a = (guint)((gdouble)a * luminance
										* (gdouble)am / 255.);

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
rsvg_defs_drawable_mask_draw (RsvgDefsDrawable * self, RsvgHandle *ctx, 
							  int dominate)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	/* combine state definitions */
	if (ctx->n_state > 1)
		rsvg_state_dominate(state, &ctx->state[ctx->n_state - 2]);

	if (state->opacity != 0xff || state->filter)
		rsvg_push_discrete_layer (ctx);

	for (i = 0; i < group->children->len; i++)
		{
			/* push the state stack */
			if (ctx->n_state == ctx->n_state_max)
				ctx->state = g_renew (RsvgState, ctx->state, 
									  ctx->n_state_max <<= 1);
			if (ctx->n_state)
				rsvg_state_inherit (&ctx->state[ctx->n_state],
									&ctx->state[ctx->n_state - 1]);
			else
				rsvg_state_init (ctx->state);
			ctx->n_state++;

			rsvg_defs_drawable_draw (g_ptr_array_index(group->children, i), 
									 ctx, 0);
	
			/* pop the state stack */
			ctx->n_state--;
			rsvg_state_finalize (&ctx->state[ctx->n_state]);
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
	const char *id = NULL, *value;
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
		}

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
	ctx->current_defs_group = NULL;
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
							
							if (val && val->type == RSVG_DEF_MASK)
									return (RsvgDefsDrawable *) val;
						}
				}
		}
	return NULL;
}

static void 
rsvg_clip_path_free (RsvgDefVal * self)
{
	RsvgClipPath *z = (RsvgClipPath *)self;
	g_ptr_array_free(z->super.children, TRUE);
	g_free (z);
}

ArtSVP *
rsvg_clip_path_render (RsvgClipPath * self, RsvgHandle *ctx)
{
	RsvgState *state = rsvg_state_current (ctx);
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	ArtSVP *svp, *svpx;
	svpx = NULL;

	/* combine state definitions */
	if (ctx->n_state > 1)
		rsvg_state_dominate(state, &ctx->state[ctx->n_state - 2]);

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
			/* push the state stack */
			if (ctx->n_state == ctx->n_state_max)
				ctx->state = g_renew (RsvgState, ctx->state, 
									  ctx->n_state_max <<= 1);
			if (ctx->n_state)
				rsvg_state_inherit (&ctx->state[ctx->n_state],
									&ctx->state[ctx->n_state - 1]);
			else
				rsvg_state_init (ctx->state);
			ctx->n_state++;

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

			/* pop the state stack */
			ctx->n_state--;
			rsvg_state_finalize (&ctx->state[ctx->n_state]);
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
	const char *id = NULL, *value = NULL;
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
		}

	rsvg_parse_style_pairs (ctx, rsvg_state_current(ctx), atts);

	ctx->current_defs_group = &clip_path->super;
	
	/* set up the defval stuff */
	clip_path->super.super.super.type = RSVG_DEF_CLIP_PATH;
	clip_path->super.super.super.free = &rsvg_clip_path_free;
	rsvg_defs_set (ctx->defs, id, &clip_path->super.super.super);
}

void 
rsvg_end_clip_path (RsvgHandle *ctx)
{
	ctx->current_defs_group = NULL;
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
							
							if (val && val->type == RSVG_DEF_CLIP_PATH)
									return (RsvgDefsDrawable *) val;
						}
				}
		}
	return NULL;
}
