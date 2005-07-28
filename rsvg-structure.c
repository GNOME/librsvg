/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-structure.c: Rsvg's structual elements

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 - 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003 - 2005 Caleb Moore <c.moore@student.unsw.edu.au>

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

   Authors: Raph Levien <raph@artofcode.com>, 
            Dom Lachowicz <cinamod@hotmail.com>, 
            Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "rsvg-structure.h"
#include "rsvg-image.h"
#include "rsvg-css.h"

#include <stdio.h>

void 
rsvg_node_draw (RsvgNode * self, RsvgDrawingCtx *ctx,
						 int dominate)
{
	RsvgState *state;

	state = self->state;

	if (0 /*!state->visible*/)
		return;

	self->draw(self, ctx, dominate);
}

/* generic function for drawing all of the children of a particular node */ 
void 
_rsvg_node_draw_children (RsvgNode * self, RsvgDrawingCtx *ctx, 
						  int dominate)
{
	guint i;
	if (dominate != -1)
		{
			rsvg_state_reinherit_top(ctx, self->state, dominate);
			
			rsvg_push_discrete_layer (ctx);	
		}
	for (i = 0; i < self->children->len; i++)
		{
			rsvg_state_push(ctx);
			rsvg_node_draw (g_ptr_array_index(self->children, i), 
									 ctx, 0);
			rsvg_state_pop(ctx);
		}			
	if (dominate != -1)
		rsvg_pop_discrete_layer (ctx);
}

/* generic function that doesn't draw anything at all */
static void 
_rsvg_node_draw_nothing (RsvgNode * self, RsvgDrawingCtx *ctx, 
						 int dominate)
{
}

static void
_rsvg_node_dont_set_atts (RsvgNode * node, RsvgHandle * ctx, RsvgPropertyBag * atts)
{
}

void
_rsvg_node_init(RsvgNode *self)
{
	self->children = g_ptr_array_new();
	self->state = g_new(RsvgState, 1);
	rsvg_state_init(self->state);
	self->type = RSVG_NODE_PATH;
	self->free = _rsvg_node_free;
	self->draw = _rsvg_node_draw_nothing;
	self->set_atts = _rsvg_node_dont_set_atts;
}

void 
_rsvg_node_free (RsvgNode *self)
{
	if (self->state != NULL)
		{
			rsvg_state_finalize (self->state);
			g_free(self->state);
		}
	if (self->children != NULL)
		g_ptr_array_free(self->children, TRUE);
	g_free (self);
}

static void
rsvg_node_group_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "g", klazz, id, atts);
		}	
}

RsvgNode *
rsvg_new_group (void)
{
	RsvgNodeGroup *group;
	group = g_new (RsvgNodeGroup, 1);
	_rsvg_node_init(&group->super);
	group->super.type = RSVG_NODE_PATH;
	group->super.draw = _rsvg_node_draw_children;
	group->super.set_atts = rsvg_node_group_set_atts;
	return &group->super;
}

void
rsvg_pop_def_group (RsvgHandle *ctx)
{
	if (ctx->currentnode != NULL)
		ctx->currentnode = ctx->currentnode->parent;
}

void 
rsvg_node_group_pack (RsvgNode *self, RsvgNode *child)
{
	g_ptr_array_add(self->children, child);
	child->parent = self;
}

static void 
rsvg_node_use_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	RsvgNodeUse *use = (RsvgNodeUse*)self;
	RsvgNode * child;
	RsvgState * state;
	double affine[6];

	rsvg_state_reinherit_top(ctx,  self->state, dominate);

	rsvg_push_discrete_layer (ctx);


	child = use->link;

	/* If it can find nothing to draw... draw nothing */
	if (!use->link)
		return;
	state = rsvg_state_current(ctx);	
	if (child->type != RSVG_NODE_SYMBOL)
		{
			_rsvg_affine_translate(affine, use->x, use->y);
			_rsvg_affine_multiply(state->affine, affine, state->affine);

			rsvg_state_push(ctx);	
			rsvg_node_draw (child, ctx, 1);
			rsvg_state_pop(ctx);
		}
	else
		{
			RsvgNodeSymbol *symbol = 
				(RsvgNodeSymbol*)child;
			double x, y, width, height;
			x = use->x;
			y = use->y;
			width = use->w;
			height = use->h;
			
			if (symbol->has_vbox){
				rsvg_preserve_aspect_ratio
					(symbol->preserve_aspect_ratio, 
					 symbol->width, symbol->height, 
					 &width, &height, &x, &y);

				_rsvg_affine_translate(affine, x, y);
				_rsvg_affine_multiply(state->affine, affine, state->affine);
				_rsvg_affine_scale(affine, width / symbol->width,
								   height / symbol->height);
				_rsvg_affine_multiply(state->affine, affine, state->affine);
				_rsvg_affine_translate(affine, -symbol->x, 
									   -symbol->y);
				_rsvg_affine_multiply(state->affine, affine, state->affine);

				if (!state->overflow || 
					(!state->has_overflow && child->state->overflow))
					rsvg_add_clipping_rect (ctx, symbol->x, symbol->y,
											symbol->width, symbol->height);
			} else {
				_rsvg_affine_translate(affine, use->x, use->y);
				_rsvg_affine_multiply(state->affine, affine, state->affine);
			}

			rsvg_state_push(ctx);
			_rsvg_node_draw_children(child, ctx, 1);
			rsvg_state_pop(ctx);

		}
	rsvg_pop_discrete_layer (ctx);
}	

static void 
rsvg_node_use_free (RsvgNode *self)
{
	RsvgNodeUse *z = (RsvgNodeUse *)self;
	rsvg_state_finalize (z->super.state);
	g_free (z->super.state);
	g_free (z);
}

static void
rsvg_node_svg_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							 int dominate)
{
	RsvgNodeSvg * sself;
	RsvgState *state;
	gdouble affine[6];
	guint i;
	sself = (RsvgNodeSvg *)self;

	rsvg_state_reinherit_top(ctx, self->state, dominate);

	rsvg_push_discrete_layer (ctx);

	state = rsvg_state_current (ctx);

	if (!state->overflow && self->parent)
		{
			rsvg_add_clipping_rect(ctx, sself->x, sself->y, sself->w, sself->h);
		}

	if (sself->has_vbox && sself->hasw && sself->hash)
		{
			affine[0] = sself->w / sself->vbw;
			affine[1] = 0;
			affine[2] = 0;
			affine[3] = sself->h / sself->vbh;
			affine[4] = sself->x - sself->vbx * sself->w / sself->vbw;
			affine[5] = sself->y - sself->vby * sself->h / sself->vbh;
			_rsvg_affine_multiply(state->affine, affine, 
								  state->affine);
		}
	else
		{
			affine[0] = 1;
			affine[1] = 0;
			affine[2] = 0;
			affine[3] = 1;
			affine[4] = sself->x;
			affine[5] = sself->y;
			_rsvg_affine_multiply(state->affine, affine, 
								  state->affine);
		}

	for (i = 0; i < self->children->len; i++)
		{
			rsvg_state_push(ctx);

			rsvg_node_draw (g_ptr_array_index(self->children, i), 
									 ctx, 0);
	
			rsvg_state_pop(ctx);
		}			

	rsvg_pop_discrete_layer (ctx);
}

static void
rsvg_node_svg_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * id, *value;
	RsvgNodeSvg * svg = (RsvgNodeSvg *)self;

	id = NULL;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					svg->has_vbox = rsvg_css_parse_vbox (value, 
														 &svg->vbx, 
														 &svg->vby,
														 &svg->vbw, 
														 &svg->vbh);
					if (svg->has_vbox)
						{
							ctx->width = svg->vbw;
							ctx->height = svg->vbh;
						}
				}
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				{
					svg->w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, ctx->width, 1);
					svg->hasw = TRUE;
					if (!svg->has_vbox)
						ctx->width = svg->w; 
				}
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				{
					svg->h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, ctx->height, 1);
					svg->hash = TRUE;
					if (!svg->has_vbox)
						ctx->height = svg->h;
				}
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				svg->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, ctx->width, 1);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				svg->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, ctx->height, 1);
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, &svg->super);
				}
		}
}

RsvgNode *
rsvg_new_svg (void)
{
	RsvgNodeSvg * svg;
	svg = g_new (RsvgNodeSvg, 1);
	_rsvg_node_init(&svg->super);
	svg->has_vbox = FALSE;
	svg->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	svg->x = 0; svg->y = 0; svg->w = -1; svg->h = -1;
	svg->hasw = svg->hash = FALSE;
	svg->vbx = 0; svg->vby = 0; svg->vbw = 0; svg->vbh = 0;
	svg->super.type = RSVG_NODE_PATH;
	svg->super.draw = rsvg_node_svg_draw;
	svg->super.set_atts = rsvg_node_svg_set_atts;
	return &svg->super;
}

static void 
rsvg_node_use_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *value = NULL, *klazz = NULL, *id = NULL;
	double font_size;	
	font_size = rsvg_state_current_font_size(ctx);
	RsvgNodeUse * use;

	use = (RsvgNodeUse *)self;
	if (rsvg_property_bag_size(atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				use->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				use->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "width")))
				use->w = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "height")))
				use->h = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, &use->super);
				}
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				rsvg_defs_add_resolver (ctx->defs, &use->link, value);
			rsvg_parse_style_attrs (ctx, self->state, "use", klazz, id, atts);
		}
	
}

RsvgNode *
rsvg_new_use ()
{
	RsvgNodeUse * use;
	use = g_new (RsvgNodeUse, 1);
	_rsvg_node_init(&use->super);
	use->super.type = RSVG_NODE_PATH;
	use->super.free = rsvg_node_use_free;
	use->super.draw = rsvg_node_use_draw;
	use->super.set_atts = rsvg_node_use_set_atts;
	use->x = 0;
	use->y = 0;
	use->w = 0;
	use->h = 0;
	use->link = NULL;
	return (RsvgNode *)use;
}

static void 
rsvg_node_symbol_set_atts(RsvgNode *self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	RsvgNodeSymbol *symbol = (RsvgNodeSymbol *)self;

	const char * klazz = NULL, *value, *id = NULL;

	if (rsvg_property_bag_size(atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, &symbol->super);
				}
			if ((value = rsvg_property_bag_lookup (atts, "viewBox")))
				{
					symbol->has_vbox = rsvg_css_parse_vbox (value, 
															&symbol->x, 
															&symbol->y,
															&symbol->width, 
															&symbol->height);
					if (symbol->has_vbox)
						{
							ctx->width = symbol->width;
							ctx->height = symbol->height;
						}
				}
			if ((value = rsvg_property_bag_lookup (atts, "preserveAspectRatio")))
				symbol->preserve_aspect_ratio = rsvg_css_parse_aspect_ratio (value);			
		}

	rsvg_parse_style_attrs (ctx, self->state, "symbol", klazz, id, atts);
}


RsvgNode *
rsvg_new_symbol(void)
{
	RsvgNodeSymbol * symbol;
	symbol = g_new (RsvgNodeSymbol, 1);
	_rsvg_node_init(&symbol->super);
	symbol->has_vbox = 0;
	symbol->preserve_aspect_ratio = RSVG_ASPECT_RATIO_XMID_YMID;
	symbol->super.type = RSVG_NODE_SYMBOL;
	symbol->super.draw = _rsvg_node_draw_nothing;
	symbol->super.set_atts = rsvg_node_symbol_set_atts;
	return &symbol->super;
}

RsvgNode *
rsvg_new_defs ()
{
	RsvgNodeGroup *group;
	group = g_new (RsvgNodeGroup, 1);
	_rsvg_node_init(&group->super);
	group->super.type = RSVG_NODE_PATH;
	group->super.draw = _rsvg_node_draw_nothing;
	group->super.set_atts = rsvg_node_group_set_atts;
	return &group->super;
}

static void 
_rsvg_node_switch_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
								 int dominate)
{
	guint i;

	rsvg_state_reinherit_top(ctx, self->state, dominate);

	rsvg_push_discrete_layer (ctx);	

	for (i = 0; i < self->children->len; i++)
		{
			RsvgNode * drawable = g_ptr_array_index(self->children, i);

			if (drawable->state->cond_true) {
				rsvg_state_push(ctx);
				rsvg_node_draw (g_ptr_array_index(self->children, i), 
										 ctx, 0);
				rsvg_state_pop(ctx);

				break; /* only render the 1st one */
			}
		}			

	rsvg_pop_discrete_layer (ctx);
}

RsvgNode *
rsvg_new_switch (void)
{
	RsvgNodeGroup *group;
	group = g_new (RsvgNodeGroup, 1);
	_rsvg_node_init(&group->super);
	group->super.type = RSVG_NODE_PATH;
	group->super.draw = _rsvg_node_switch_draw;
	group->super.set_atts = rsvg_node_group_set_atts;
	return &group->super;
}
