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
#include "rsvg-text.h"
#include "rsvg-css.h"

#include "rsvg-shapes.h"

#include <ft2build.h>
#include FT_GLYPH_H
#include FT_OUTLINE_H

#include <pango/pangoft2.h>

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

typedef struct _RsvgTspan RsvgTspan;

typedef struct _RsvgTChunk RsvgTChunk;

struct _RsvgTspan {
	gdouble x, y;
	gboolean hasx, hasy;
	gdouble dx, dy;
	RsvgTspan * parent;
	gint parentindex;
	GPtrArray * contents;
	RsvgState state;
};

struct _RsvgTChunk {
	GString * string;
	RsvgTspan * span;
};

static RsvgTChunk *
rsvg_tchunk_new_text(gchar *text)
{
	RsvgTChunk * output;
	output = g_new(RsvgTChunk, 1);
	output->string = g_string_new(text);
	output->span = NULL;
	return output;
}

static RsvgTChunk *
rsvg_tchunk_new_span(RsvgTspan *span)
{
	RsvgTChunk * output;
	output = g_new(RsvgTChunk, 1);
	output->string = NULL;
	output->span = span;
	return output;
}

static void
rsvg_tchunk_free(RsvgTChunk * self);

static void
rsvg_tspan_free(RsvgTspan * self)
{
	unsigned int i;
	rsvg_state_finalize(&self->state);
	for (i = 0; i < self->contents->len; i++) {
		rsvg_tchunk_free (g_ptr_array_index(self->contents, i));
	}
	g_ptr_array_free(self->contents, 1);
	g_free(self);
}

static void
rsvg_tchunk_free(RsvgTChunk * self)
{
	if (self->string)
		g_string_free(self->string, 1);
	if (self->span)
		rsvg_tspan_free(self->span);
	g_free(self);
}

static RsvgTspan *
rsvg_tspan_new()
{
	RsvgTspan * self;
	self = g_new(RsvgTspan, 1);
	self->dx = 0;
	self->dy = 0;
	self->hasx = FALSE;
	self->hasy = FALSE;
	self->parent = NULL;
	self->parentindex = 0;
	self->contents = g_ptr_array_new();
	return self;
}

static void
rsvg_tchunk_remove_leading(RsvgTChunk * self);

static void
rsvg_tspan_remove_leading(RsvgTspan * self)
{
	if (!self)
		return;
	if (!self->contents->len == 0)
		return;
	rsvg_tchunk_remove_leading(g_ptr_array_index(self->contents, 0));
}

static void
rsvg_tchunk_remove_leading(RsvgTChunk * self)
{
	if (self->string)
		if (self->string->str[0] == ' ')
			g_string_erase(self->string, 0, 1);
	if (self->span)
		rsvg_tspan_remove_leading(self->span);
}

static void
rsvg_tchunk_remove_trailing(RsvgTChunk * self)
{
	if (self->string)
		if (self->string->str[self->string->len] == ' ')
			g_string_erase(self->string, self->string->len - 1, 1);
	if (self->span)
		rsvg_tspan_remove_trailing(self->span);
}

static void
rsvg_tspan_remove_trailing(RsvgTspan * self)
{
	if (!self)
		return;
	if (!self->contents->len == 0)
		return;
	rsvg_tchunk_remove_trailing(g_ptr_array_index(self->contents, 
												  self->contents->len - 1));
}

typedef struct _RsvgSaxHandlerText {
	RsvgSaxHandler super;
	RsvgSaxHandler *parent;
	RsvgHandle *ctx;
	GString * id;
	RsvgTspan * tspan;
	RsvgTspan * innerspan;
	RsvgNodeText * block;
} RsvgSaxHandlerText;

static void
rsvg_text_handler_free (RsvgSaxHandler *self)
{
	RsvgSaxHandlerText * z;
	z = (RsvgSaxHandlerText *)self;

	/*maybe this isn't the best place for this*/
	rsvg_tspan_remove_leading(z->tspan);
	rsvg_tspan_remove_trailing(z->tspan);

	g_string_free(z->id, TRUE);
	g_free (self);
}

static void
rsvg_text_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	char *string, *tmp;
	int i, j;
	RsvgSaxHandlerText * z;
	RsvgTChunk * tchunk;

	z = (RsvgSaxHandlerText *)self;	

	string = g_try_malloc (len + 1);

	for (i = 0; i < len; i++)
		{
			
			if (ch[i] == '\n' || ch[i] == '\t')
				string[i] = ' ';
			else
				string[i] = ch[i];
		}

	if (1 /*todo replace with something about xml:space*/)
		{
			tmp = g_try_malloc (len + 1);
			j = 0;
			for (i = 0; i < len; i++)
				{
					if (j == 0)					
						tmp[j++] = string[i];
					else
						{
							if (string[i] != ' ' || string[i - 1] != ' ')
								tmp[j++] = string[i];
						}
				}
			tmp[j] = '\0';
			g_free (string);
			string = tmp;
		}
	else
		j = len;
	
	if (j == 0)
		{
			g_free (string);
			return;
		}


	if (!g_utf8_validate (string, -1, NULL))
		{
			tmp = rsvg_make_valid_utf8 (string);
			g_free (string);
			string = tmp;
		}

	tchunk = rsvg_tchunk_new_text(string);

	g_ptr_array_add (z->innerspan->contents, tchunk);

	g_free (string);
}

static void
rsvg_start_tspan (RsvgSaxHandlerText *self, RsvgPropertyBag *atts)
{
	RsvgHandle *ctx;
	RsvgState state;
	RsvgSaxHandlerText *z;
	RsvgTspan * tspan;
	RsvgTChunk * tchunk;
	const char * klazz = NULL, * id = NULL, *value;
	double font_size;
	tspan = rsvg_tspan_new();
	z = (RsvgSaxHandlerText *)self;
	ctx = z->ctx;
	font_size = rsvg_state_current_font_size(ctx);
	rsvg_state_init(&state);

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				{
					tspan->x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
					tspan->hasx = TRUE;
				}
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				{
					tspan->y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
					tspan->hasy = TRUE;
				}
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				tspan->dx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				tspan->dy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "tspan", klazz, id, atts);
		}

	tchunk = rsvg_tchunk_new_span(tspan);

	tspan->parentindex = z->innerspan->contents->len;
	tspan->parent = z->innerspan;
	tspan->state = state;
	g_ptr_array_add (z->innerspan->contents, tchunk);
	z->innerspan = tspan;
	
}

static void
rsvg_text_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 RsvgPropertyBag *atts)
{
	/*this should be the only thing starting inside of text*/ 
	if (!strcmp ((char *)name, "tspan"))
		rsvg_start_tspan ((RsvgSaxHandlerText *)self, atts);

}

static void
rsvg_text_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
	RsvgSaxHandlerText *z = (RsvgSaxHandlerText *)self;
	RsvgHandle *ctx = z->ctx;
	
	if (!strcmp ((char *)name, "tspan"))
		{
			RsvgTspan * child;
			child = z->innerspan;
			z->innerspan = child->parent;
		}
	else if (!strcmp ((char *)name, "text"))
		{
			if (ctx->handler != NULL)
				{
					ctx->handler->free (ctx->handler);
					ctx->handler = z->parent;
				}
		} 
	
}

static void 
rsvg_node_text_free (RsvgNode *self)
{
	RsvgNodeText *z = (RsvgNodeText *)self;
	rsvg_tspan_free (z->chunk);
	g_free (z);
}

void 
rsvg_text_render_text (RsvgDrawingCtx *ctx,
					   RsvgTspan  *tspan,
					   const char *text,
					   gdouble *x,
					   gdouble *y);

static gdouble
rsvg_text_width  (RsvgDrawingCtx *ctx,
				  RsvgTspan  *tspan,
				  const char *text);

static gdouble
rsvg_text_tspan_width (RsvgDrawingCtx *ctx,
					   RsvgTspan  *tspan)
{
	RsvgTspan  *currentspan = tspan;
	gdouble currentwidth = 0;
	guint currentindex = 0;
	while (1)
		{
			if (currentindex >= currentspan->contents->len)
				{
					currentindex = currentspan->parentindex;
					currentspan = currentspan->parent;
					if (currentspan == NULL)
						return currentwidth;
				}
			else
				{
					RsvgTChunk * currentchunk = 
						g_ptr_array_index(currentspan->contents, currentindex);
					if (currentchunk->string)
						currentwidth += rsvg_text_width (ctx, currentspan, currentchunk->string->str);
					else
						{
							currentspan = currentchunk->span;
							currentindex = -1;
							if (currentspan->hasx || currentspan->hasy)
								return currentwidth;
							currentwidth += currentspan->dx;
						}
				}
			currentindex++;
		}
}

static void
rsvg_tspan_draw(RsvgTspan * self, RsvgDrawingCtx *ctx, gdouble *x, gdouble *y, int dominate);

static void
rsvg_tchunk_draw(RsvgTChunk * self, RsvgDrawingCtx *ctx, RsvgTspan *span, gdouble *x, gdouble *y)
{
	if (self->string)
		rsvg_text_render_text (ctx, span, self->string->str, x, y);
	if (self->span)
		{
			rsvg_state_push(ctx);
			rsvg_tspan_draw (self->span, ctx, x, y, 0);
			rsvg_state_pop(ctx);
		}
}

static void
rsvg_tspan_draw(RsvgTspan * self, RsvgDrawingCtx *ctx, gdouble *x, gdouble *y, int dominate)
{
	unsigned int i;
	rsvg_state_reinherit_top(ctx, &self->state, dominate);
	if (self->hasx || self->hasy)
		{
			switch (rsvg_state_current(ctx)->text_anchor)
				{
				case TEXT_ANCHOR_START:
					*x = self->x;		
					break;
				case TEXT_ANCHOR_MIDDLE:
					*x = self->x - rsvg_text_tspan_width (ctx, self) / 2;
					break;
				case TEXT_ANCHOR_END:
					*x = self->x - rsvg_text_tspan_width (ctx, self);
					break;
				}
			*y = self->y;
		}

	if (rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_LTR || 
		rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_RTL)
		{
			*y += self->dx;
			*x += self->dy;
		}
	else
		{
			*x += self->dx;
			*y += self->dy;
		}
	for (i = 0; i < self->contents->len; i++) {
		rsvg_tchunk_draw (g_ptr_array_index(self->contents, i), ctx, self, x, y);
	}
}

static void 
rsvg_node_text_draw (RsvgNode * self, RsvgDrawingCtx *ctx, 
							  int dominate)
{
	gdouble x, y;
	RsvgNodeText *text = (RsvgNodeText*)self;

	rsvg_tspan_draw(text->chunk, ctx, &x, &y, dominate);
}

void
rsvg_start_text (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x, y, dx, dy, font_size;
	const char * klazz = NULL, * id = NULL, *value;
	RsvgState state;
	RsvgNodeText *text;
	RsvgSaxHandlerText *handler = g_new0 (RsvgSaxHandlerText, 1);

	handler->super.free = rsvg_text_handler_free;
	handler->super.characters = rsvg_text_handler_characters;
	handler->super.start_element = rsvg_text_handler_start;
	handler->super.end_element   = rsvg_text_handler_end;
	handler->ctx = ctx;
	font_size = rsvg_state_current_font_size(ctx);
	x = y = dx = dy = 0.;
	
	rsvg_state_init(&state);
	
	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				x = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				y = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				dx = rsvg_css_parse_normalized_length (value, ctx->dpi_x, (gdouble)ctx->width, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				dy = rsvg_css_parse_normalized_length (value, ctx->dpi_y, (gdouble)ctx->height, font_size);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				id = value;

			rsvg_parse_style_attrs (ctx, &state, "text", klazz, id, atts);
		}
	
	text = g_new (RsvgNodeText, 1);

	text->super.type = RSVG_NODE_PATH;
	text->super.free = rsvg_node_text_free;
	text->super.draw = rsvg_node_text_draw;
	rsvg_defs_set (ctx->defs, id, &text->super);
	
	text->super.parent = (RsvgNode *)ctx->currentnode;
	if (text->super.parent != NULL)
		rsvg_node_group_pack(text->super.parent, &text->super);

	handler->id = g_string_new(id);

	handler->tspan = rsvg_tspan_new();
	handler->tspan->parent = NULL;
	handler->tspan->x = x;
	handler->tspan->y = y;
	handler->tspan->hasx = TRUE;
	handler->tspan->hasy = TRUE;

	handler->tspan->dx = dx;
	handler->tspan->dy = dy;

	handler->innerspan = handler->tspan;
	handler->block = text;
	handler->tspan->state = state;

	handler->parent = ctx->handler;
	ctx->handler = &handler->super;
	text->chunk = handler->tspan;
}

typedef struct _RsvgTextLayout RsvgTextLayout;

struct _RsvgTextLayout
{
	PangoLayout * layout;
	RsvgDrawingCtx  * ctx;
	TextAnchor anchor;
	RsvgTspan    *span;
	gdouble x, y;
	gboolean orientation;
};

typedef struct _RenderCtx RenderCtx;

struct _RenderCtx
{
	GString      *path;
	gboolean      wrote;
	gdouble       offset_x;
	gdouble       offset_y;
};

typedef void (* RsvgTextRenderFunc) (PangoFont  *font,
									 PangoGlyph  glyph,
									 FT_Int32    load_flags,
									 gint        x,
									 gint        y,
									 gpointer    render_data);

#ifndef FT_GLYPH_FORMAT_OUTLINE
#define FT_GLYPH_FORMAT_OUTLINE ft_glyph_format_outline
#endif

#ifndef FT_LOAD_TARGET_MONO
#define FT_LOAD_TARGET_MONO FT_LOAD_MONOCHROME
#endif

static RenderCtx *
rsvg_render_ctx_new (void)
{
	RenderCtx *ctx;
	
	ctx = g_new0 (RenderCtx, 1);
	ctx->path = g_string_new (NULL);
	
	return ctx;
}

static void
rsvg_render_ctx_free(RenderCtx *ctx)
{
	g_string_free(ctx->path, TRUE);
	g_free(ctx);
}

static void
rsvg_text_ft2_subst_func (FcPattern *pattern,
                          gpointer   data)
{
	RsvgHandle *ctx = (RsvgHandle *)data;

	(void)ctx;

	FcPatternAddBool (pattern, FC_HINTING, 0);
	FcPatternAddBool (pattern, FC_ANTIALIAS, 0);
	FcPatternAddBool (pattern, FC_AUTOHINT, 0);	
	FcPatternAddBool (pattern, FC_SCALABLE, 1);
}

static PangoContext *
rsvg_text_get_pango_context (RsvgDrawingCtx *ctx)
{
	PangoContext    *context;
	PangoFT2FontMap *fontmap;
	
	fontmap = PANGO_FT2_FONT_MAP (pango_ft2_font_map_new ());
	
	pango_ft2_font_map_set_resolution (fontmap, ctx->dpi_x, ctx->dpi_y);
	
	pango_ft2_font_map_set_default_substitute (fontmap,
											   rsvg_text_ft2_subst_func,
											   ctx,
											   (GDestroyNotify) NULL);
	
	context = pango_ft2_font_map_create_context (fontmap);
	g_object_unref (fontmap);
	
	return context;
}

static void
rsvg_text_layout_free(RsvgTextLayout * layout)
{
	g_object_unref(G_OBJECT(layout->layout));
	g_free(layout);
}

static RsvgTextLayout *
rsvg_text_layout_new (RsvgDrawingCtx *ctx,
					  RsvgState *state,
					  const char *text)
{
	RsvgTextLayout * layout;
	PangoFontDescription *font_desc;

	if (ctx->pango_context == NULL)
		ctx->pango_context = rsvg_text_get_pango_context (ctx);
	
	if (state->lang)
		pango_context_set_language (ctx->pango_context,
									pango_language_from_string (state->lang));
	
	if (state->unicode_bidi == UNICODE_BIDI_OVERRIDE ||
		state->unicode_bidi == UNICODE_BIDI_EMBED)
		pango_context_set_base_dir (ctx->pango_context, state->text_dir);
	
	layout = g_new0 (RsvgTextLayout, 1);
	layout->layout = pango_layout_new (ctx->pango_context);
	layout->ctx = ctx;
	
	font_desc = pango_font_description_copy (pango_context_get_font_description (ctx->pango_context));
	
	if (state->font_family)
		pango_font_description_set_family_static (font_desc, state->font_family);
	
	pango_font_description_set_style (font_desc, state->font_style);
	pango_font_description_set_variant (font_desc, state->font_variant);
	pango_font_description_set_weight (font_desc, state->font_weight);
	pango_font_description_set_stretch (font_desc, state->font_stretch); 
	pango_font_description_set_size (font_desc, state->font_size * PANGO_SCALE / ctx->dpi_y * 72); 
	pango_layout_set_font_description (layout->layout, font_desc);
	pango_font_description_free (font_desc);
	
	if (text)
		pango_layout_set_text (layout->layout, text, -1);
	else
		pango_layout_set_text (layout->layout, NULL, 0);
	
	pango_layout_set_alignment (layout->layout, (state->text_dir == PANGO_DIRECTION_LTR || 
												 state->text_dir == PANGO_DIRECTION_TTB_LTR) ? 
								PANGO_ALIGN_LEFT : PANGO_ALIGN_RIGHT);
	
	layout->anchor = state->text_anchor;

	return layout;
}

static void
rsvg_text_layout_get_offsets (RsvgTextLayout *layout,
                              gint           *x,
                              gint           *y)
{
	PangoRectangle  ink;
	PangoRectangle  logical;
	
	pango_layout_get_pixel_extents (layout->layout, &ink, &logical);
	
	if (ink.width < 1 || ink.height < 1) {
		*x = *y = 0;
		return;
	}
	
	*x = MIN (ink.x, logical.x);
	*y = MIN (ink.y, logical.y);
}

static FT_Int32
rsvg_text_layout_render_flags (RsvgTextLayout *layout)
{
	gint flags = 0;

	flags |= FT_LOAD_NO_BITMAP;
	flags |= FT_LOAD_TARGET_MONO;
	flags |= FT_LOAD_NO_HINTING;

	return flags;
}

static void
rsvg_text_vector_coords (RenderCtx       *ctx,
						 const FT_Vector *vector,
						 gdouble         *x,
						 gdouble         *y)
{
	*x = ctx->offset_x + (double)vector->x / 64;
	*y = ctx->offset_y - (double)vector->y / 64;
}

static gint
moveto (FT_Vector *to,
		gpointer   data)
{
	RenderCtx * ctx;
	gchar buf[G_ASCII_DTOSTR_BUF_SIZE];
	gdouble x, y;
	
	ctx = (RenderCtx *)data;
	
	if (ctx->wrote)
		g_string_append(ctx->path, "Z ");
	else
		ctx->wrote = TRUE;

	g_string_append_c(ctx->path, 'M');
	
	rsvg_text_vector_coords(ctx, to, &x, &y);
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	g_string_append_c(ctx->path, ' ');

	return 0;
}

static gint
lineto (FT_Vector *to,
		gpointer   data)
{
	RenderCtx * ctx;
	gchar buf[G_ASCII_DTOSTR_BUF_SIZE];
	gdouble x, y;
	
	ctx = (RenderCtx *)data;
	
	if (!ctx->wrote)
		return 0;

	g_string_append_c(ctx->path, 'L');
	
	rsvg_text_vector_coords(ctx, to, &x, &y);
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	g_string_append_c(ctx->path, ' ');

	return 0;
}

static gint
conicto (FT_Vector *ftcontrol,
		 FT_Vector *to,
		 gpointer   data)
{
	RenderCtx * ctx;
	gchar buf[G_ASCII_DTOSTR_BUF_SIZE];
	gdouble x, y;
	
	ctx = (RenderCtx *)data;

	if (!ctx->wrote)
		return 0;

	g_string_append_c(ctx->path, 'Q');
	
	rsvg_text_vector_coords(ctx, ftcontrol, &x, &y);
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	
	rsvg_text_vector_coords(ctx, to, &x, &y);
	g_string_append_c(ctx->path, ' ');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	g_string_append_c(ctx->path, ' ');

	return 0;
}

static gint
cubicto (FT_Vector *ftcontrol1,
		 FT_Vector *ftcontrol2,
		 FT_Vector *to,
		 gpointer   data)
{
	RenderCtx * ctx;
	gchar buf[G_ASCII_DTOSTR_BUF_SIZE];
	gdouble x, y;
	
	ctx = (RenderCtx *)data;
	
	if (!ctx->wrote)
		return 0;

	g_string_append_c(ctx->path, 'C');
	
	rsvg_text_vector_coords(ctx, ftcontrol1, &x, &y);
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	
	rsvg_text_vector_coords(ctx, ftcontrol2, &x, &y);
	g_string_append_c(ctx->path, ' ');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	
	rsvg_text_vector_coords(ctx, to, &x, &y);
	g_string_append_c(ctx->path, ' ');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), x));
	g_string_append_c(ctx->path, ',');
	g_string_append(ctx->path, g_ascii_dtostr(buf, sizeof(buf), y));
	g_string_append_c(ctx->path, ' ');	

	return 0;
}

static gint
rsvg_text_layout_render_glyphs (RsvgTextLayout     *layout,
								PangoFont          *font,
								PangoGlyphString   *glyphs,
								RsvgTextRenderFunc  render_func,
								gint                x,
								gint                y,
								gpointer            render_data)
{
	PangoGlyphInfo *gi;
	FT_Int32        flags;
	FT_Vector       pos;
	gint            i;
	gint            x_position = 0;
	
	flags = rsvg_text_layout_render_flags (layout);

	for (i = 0, gi = glyphs->glyphs; i < glyphs->num_glyphs; i++, gi++)
		{
			if (gi->glyph)
				{
					pos.x = x + x_position + gi->geometry.x_offset;
					pos.y = y + gi->geometry.y_offset;
					
					render_func (font, gi->glyph, flags,
								 pos.x, pos.y,
								 render_data);
				}
		  
			x_position += glyphs->glyphs[i].geometry.width;
		}
	return x_position;
}

static void
rsvg_text_render_vectors (PangoFont     *font,
						  PangoGlyph     pango_glyph,
						  FT_Int32       flags,
						  gint           x,
						  gint           y,
						  gpointer       ud)
{
	static const FT_Outline_Funcs outline_funcs =
		{
			moveto,
			lineto,
			conicto,
			cubicto,
			0,
			0
		};
	
	FT_Face   face;
	FT_Glyph  glyph;
	
	RenderCtx *context = (RenderCtx *)ud;
	
	face = pango_ft2_font_get_face (font);

	FT_Load_Glyph (face, (FT_UInt) pango_glyph, flags);

	FT_Get_Glyph (face->glyph, &glyph);
	
	if (face->glyph->format == FT_GLYPH_FORMAT_OUTLINE)
		{
			FT_OutlineGlyph outline_glyph = (FT_OutlineGlyph) glyph;
			
			context->offset_x = (gdouble) x / PANGO_SCALE;
			context->offset_y = (gdouble) y / PANGO_SCALE - (int)face->size->metrics.ascender / 64;

			FT_Outline_Decompose (&outline_glyph->outline, &outline_funcs, context);			
		}
	
	FT_Done_Glyph (glyph);
}

static void
rsvg_text_layout_render_line (RsvgTextLayout     *layout,
							  PangoLayoutLine    *line,
							  RsvgTextRenderFunc  render_func,
							  gint                x,
							  gint                y,
							  gpointer            render_data)
{
	PangoRectangle  rect;
	GSList         *list;
	gint            x_off = 0;
	
	for (list = line->runs; list; list = list->next)
		{
			PangoLayoutRun *run = list->data;
			
			pango_glyph_string_extents (run->glyphs, run->item->analysis.font,
										NULL, &rect);
			x_off += rsvg_text_layout_render_glyphs (layout,
													 run->item->analysis.font, run->glyphs,
													 render_func,
													 x + x_off, y,
													 render_data);
			
		}
}

static void
rsvg_text_layout_render (RsvgTextLayout     *layout,
						 RsvgTextRenderFunc  render_func,
						 gpointer            render_data)
{
	PangoLayoutIter *iter;
	gint             offx, offy;
	gint             x, y;

	rsvg_text_layout_get_offsets (layout, &offx, &offy);

	x = offx + layout->x;
	y = offy + layout->y;

	x *= PANGO_SCALE;
	y *= PANGO_SCALE;
	
	iter = pango_layout_get_iter (layout->layout);
	
	if (iter)
		{
			PangoRectangle   rect;
			PangoLayoutLine *line;
			gint             baseline;
			
			line = pango_layout_iter_get_line (iter);
			
			pango_layout_iter_get_line_extents (iter, NULL, &rect);
			baseline = pango_layout_iter_get_baseline (iter);
			
			rsvg_text_layout_render_line (layout, line,
										  render_func,
										  x + rect.x,
										  y + baseline,
										  render_data);

			layout->x += rect.width / PANGO_SCALE + offx;
		}

	pango_layout_iter_free (iter);
}

static GString * 
rsvg_text_render_text_as_string (RsvgDrawingCtx *ctx,
								 RsvgTspan  *tspan,
								 const char *text,
								 gdouble *x,
								 gdouble *y)
{
	RsvgTextLayout *layout;
	RenderCtx      *render;
	RsvgState      *state;
	GString        *output;
	state = rsvg_state_current(ctx);

	state->fill_rule = FILL_RULE_EVENODD;	
	state->has_fill_rule = TRUE;

	layout = rsvg_text_layout_new (ctx, state, text);
	layout->span = tspan;
	layout->x = *x;
	layout->y = *y;
	layout->orientation = rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_LTR || 
		rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_RTL;
	render = rsvg_render_ctx_new ();

	rsvg_text_layout_render (layout, rsvg_text_render_vectors, 
							 (gpointer)render);

	if (render->wrote)
		g_string_append_c(render->path, 'Z');

	*x = layout->x;
	*y = layout->y;

	output = g_string_new(render->path->str);
	rsvg_render_ctx_free (render);
	rsvg_text_layout_free (layout);
	return output;
}

void
rsvg_text_render_text (RsvgDrawingCtx *ctx,
					   RsvgTspan  *tspan,
					   const char *text,
					   gdouble *x,
					   gdouble *y)
{
	GString * render;
	render = rsvg_text_render_text_as_string (ctx,tspan,text,x,y);
	rsvg_render_path (ctx, render->str);
	g_string_free(render, TRUE);
}

static gdouble
rsvg_text_layout_width  (RsvgTextLayout *layout)
{
	PangoLayoutIter *iter;
	gint             offx, offy;

	rsvg_text_layout_get_offsets (layout, &offx, &offy);
	
	iter = pango_layout_get_iter (layout->layout);
	
	if (iter)
		{
			PangoRectangle   rect;
			PangoLayoutLine *line;
			
			line = pango_layout_iter_get_line (iter);
			
			pango_layout_iter_get_line_extents (iter, NULL, &rect);

			pango_layout_iter_free (iter);
			return rect.width / PANGO_SCALE + offx;
		}

	return 0;
}

static gdouble
rsvg_text_width       (RsvgDrawingCtx *ctx,
					   RsvgTspan  *tspan,
					   const char *text)
{
	RsvgTextLayout *layout;
	RsvgState      *state;
	gdouble output;

	state = rsvg_state_current(ctx);

	layout = rsvg_text_layout_new (ctx, state, text);
	layout->span = tspan;

	output = rsvg_text_layout_width (layout);
	rsvg_text_layout_free (layout);

	return output;
}
