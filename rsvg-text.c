/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-text.c: Text handling routines for RSVG

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

#include <string.h>

#include "rsvg-private.h"
#include "rsvg-styles.h"
#include "rsvg-filter.h"
#include "rsvg-text.h"
#include "rsvg-css.h"

#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_render_mask.h>

#include <pango/pangoft2.h>

#include "rsvg-shapes.h"

char *
rsvg_make_valid_utf8 (const char *str)
{
	GString *string;
	const char *remainder, *invalid;
	int remaining_bytes, valid_bytes;
	
	string = NULL;
	remainder = str;
	remaining_bytes = strlen (str);
	
	while (remaining_bytes != 0)
		{
			if (g_utf8_validate (remainder, remaining_bytes, &invalid))
				break;
			valid_bytes = invalid - remainder;
      
			if (string == NULL) 
				string = g_string_sized_new (remaining_bytes);
			
			g_string_append_len (string, remainder, valid_bytes);
			g_string_append_c (string, '?');
			
			remaining_bytes -= valid_bytes + 1;
			remainder = invalid + 1;
		}
	
	if (string == NULL) 
		return g_strdup (str);
	
	g_string_append (string, remainder);
	
	return g_string_free (string, FALSE);
}

typedef struct _RsvgSaxHandlerText {
	RsvgSaxHandler super;
	RsvgSaxHandler *parent;
	RsvgHandle *ctx;
	gdouble x, y;
} RsvgSaxHandlerText;

static void
rsvg_text_handler_free (RsvgSaxHandler *self)
{
	g_free (self);
}

static void
rsvg_text_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	RsvgSaxHandlerText *z = (RsvgSaxHandlerText *)self;
	RsvgHandle *ctx = z->ctx;
	char *string, *tmp;
	int beg, end;
	RsvgState *state;
	
	state = rsvg_state_current (ctx);
	if (state->fill == NULL && state->font_size <= 0)
		return;

	/* not quite up to spec, but good enough */
	if (!state->visible)
		return;
	
	/* Copy ch into string, chopping off leading and trailing whitespace */
	for (beg = 0; beg < len; beg++)
		if (!g_ascii_isspace (ch[beg]))
			break;
	
	for (end = len; end > beg; end--)
		if (!g_ascii_isspace (ch[end - 1]))
			break;
	
	if (end - beg == 0)
		{
			/* TODO: be smarter with some "last was space" logic */
			end = 1; beg = 0;
			string = g_strdup (" ");
		}
	else
		{
			string = g_malloc (end - beg + 1);
			memcpy (string, ch + beg, end - beg);
			string[end - beg] = 0;
		}
	
	if (!g_utf8_validate (string, -1, NULL))
		{
			tmp = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = tmp;
		}

	rsvg_text_render_text (ctx, state, string, NULL, z->x, z->y);

	g_free (string);
}

void
rsvg_start_tspan (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double affine[6] ;
	double x, y, dx, dy;
	RsvgState *state;
	const char * klazz = NULL, * id = NULL, *value;
	x = y = dx = dy = 0.;
	
	state = rsvg_state_current (ctx);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->width, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->height, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				dx = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->width, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				dy = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->height, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, state, "tspan", klazz, id, atts);
		}
	
	/* todo: transform() is illegal here */
	x += dx ;
	y += dy ;
	
	if (x > 0 && y > 0)
		{
			art_affine_translate (affine, x, y);
			art_affine_multiply (state->affine, affine, state->affine);
		}
}

static void
rsvg_text_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
	RsvgSaxHandlerText *z = (RsvgSaxHandlerText *)self;
	RsvgHandle *ctx = z->ctx;

	/* push the state stack */
	if (ctx->n_state == ctx->n_state_max)
		ctx->state = g_renew (RsvgState, ctx->state, ctx->n_state_max <<= 1);
	if (ctx->n_state)
		rsvg_state_clone (&ctx->state[ctx->n_state],
						  &ctx->state[ctx->n_state - 1]);
	else
		rsvg_state_init (ctx->state);
	ctx->n_state++;
  
	/* this should be the only thing starting inside of text */
	if (!strcmp ((char *)name, "tspan"))
		rsvg_start_tspan (ctx, atts);
}

static void
rsvg_text_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerText *z = (RsvgSaxHandlerText *)self;
	RsvgHandle *ctx = z->ctx;

	if (!strcmp ((char *)name, "tspan"))
		{
			/* advance the text offset */
			RsvgState *tspan = &ctx->state[ctx->n_state - 1];
			RsvgState *text  = &ctx->state[ctx->n_state - 2];
			text->text_offset += (tspan->text_offset - text->text_offset);
		}
	else if (!strcmp ((char *)name, "text"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = z->parent;
				}
		} 
	
	/* pop the state stack */
	ctx->n_state--;
	rsvg_state_finalize (&ctx->state[ctx->n_state]);
}

void
rsvg_start_text (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x, y, dx, dy;
	const char * klazz = NULL, * id = NULL, *value;
	RsvgState *state;
	
	RsvgSaxHandlerText *handler = g_new0 (RsvgSaxHandlerText, 1);
	handler->super.free = rsvg_text_handler_free;
	handler->super.characters = rsvg_text_handler_characters;
	handler->super.start_element = rsvg_text_handler_start;
	handler->super.end_element   = rsvg_text_handler_end;
	handler->ctx = ctx;
	
	x = y = dx = dy = 0.;
	
	state = rsvg_state_current (ctx);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->width, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->height, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				dx = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->width, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				dy = rsvg_css_parse_normalized_length (value, ctx->dpi, (gdouble)ctx->height, state->font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, state, "text", klazz, id, atts);
		}

	x += dx ;
	y += dy ;
	
	handler->x = x;
	handler->y = y;

	handler->parent = ctx->handler;
	ctx->handler = &handler->super;
}
