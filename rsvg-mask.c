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
#include "rsvg-css.h"
#include <string.h>

static void 
rsvg_mask_free (RsvgDefVal * self)
{
	RsvgMask *z = (RsvgMask *)self;
	g_ptr_array_free(z->super.children, TRUE);
	rsvg_state_finalize (&z->super.super.state);
	g_free (z);
}

static void 
rsvg_defs_drawable_mask_draw (RsvgDefsDrawable * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	RsvgDefsDrawableGroup *group = (RsvgDefsDrawableGroup*)self;
	guint i;

	rsvg_state_reinherit_top(ctx, &self->state, 0);

	rsvg_push_discrete_layer (ctx);

	for (i = 0; i < group->children->len; i++)
		{
			rsvg_state_push(ctx);

			rsvg_defs_drawable_draw (g_ptr_array_index(group->children, i), 
									 ctx, 0);
	
			rsvg_state_pop(ctx);
		}			

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
