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
rsvg_mask_free (RsvgNode * self)
{
	RsvgMask *z = (RsvgMask *)self;
	g_ptr_array_free(z->super.children, TRUE);
	rsvg_state_finalize (z->super.super.state);
	g_free(z->super.super.state);
	g_free (z);
}

static void 
rsvg_node_mask_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	RsvgNodeGroup *group = (RsvgNodeGroup*)self;
	guint i;

	rsvg_state_reinherit_top(ctx, self->state, 0);

	rsvg_push_discrete_layer (ctx);

	for (i = 0; i < group->children->len; i++)
		{
			rsvg_state_push(ctx);

			rsvg_node_draw (g_ptr_array_index(group->children, i), 
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
	mask->super.super.state = g_new(RsvgState, 1);
	mask->super.children = g_ptr_array_new ();
	mask->super.super.type = RSVG_NODE_MASK;
	mask->super.super.free = rsvg_mask_free;
	mask->super.super.draw = rsvg_node_mask_draw;
	mask->super.super.add_child = rsvg_node_group_add_child;
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


	rsvg_state_init(mask->super.super.state);
	rsvg_parse_style_attrs (ctx, mask->super.super.state, "mask", klazz, id, atts);

	mask->super.super.parent = (RsvgNode *)ctx->currentnode;

	ctx->currentnode = &mask->super.super;

	rsvg_defs_set (ctx->defs, id, &mask->super.super);
}

void 
rsvg_end_mask (RsvgHandle *ctx)
{
	ctx->currentnode = ((RsvgNode *)ctx->currentnode)->parent;
}

RsvgNode *
rsvg_mask_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgNode *val;
			
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
					
					if (val && val->type == RSVG_NODE_MASK)
						return (RsvgNode *) val;
				}
		}
	return NULL;
}

static void 
rsvg_clip_path_free (RsvgNode * self)
{
	RsvgClipPath *z = (RsvgClipPath *)self;
	g_ptr_array_free(z->super.children, TRUE);
	rsvg_state_finalize (z->super.super.state);
	g_free(z->super.super.state);
	g_free (z);
}

static RsvgClipPath *
rsvg_new_clip_path (void)
{
	RsvgClipPath *clip_path;
	
	clip_path = g_new (RsvgClipPath, 1);
	clip_path->super.children = g_ptr_array_new ();
	clip_path->units = userSpaceOnUse;
	clip_path->super.super.state = g_new(RsvgState, 1);
	clip_path->super.super.type = RSVG_NODE_CLIP_PATH;
	clip_path->super.super.free = rsvg_clip_path_free;
	clip_path->super.super.add_child = rsvg_node_group_add_child;
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

	rsvg_state_init (clip_path->super.super.state);

	rsvg_parse_style_attrs (ctx, clip_path->super.super.state, "clipPath", klazz, id, atts);

	clip_path->super.super.parent = (RsvgNode *)ctx->currentnode;

	ctx->currentnode = &clip_path->super.super;
	
	/* set up the defval stuff */
	rsvg_defs_set (ctx->defs, id, &clip_path->super.super);
}

void 
rsvg_end_clip_path (RsvgHandle *ctx)
{
	ctx->currentnode = ((RsvgNode *)ctx->currentnode)->parent;
}

RsvgNode *
rsvg_clip_path_parse (const RsvgDefs * defs, const char *str)
{
	if (!strncmp (str, "url(", 4))
		{
			const char *p = str + 4;
			int ix;
			char *name;
			RsvgNode *val;
			
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
					
					if (val && val->type == RSVG_NODE_CLIP_PATH)
						return (RsvgNode *) val;
				}
		}
	return NULL;
}
