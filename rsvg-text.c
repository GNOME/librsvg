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
#include "rsvg-shapes.h"
#include "rsvg-filter.h"
#include "rsvg-text.h"
#include "rsvg-css.h"
#include "rsvg-mask.h"

#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_render_mask.h>

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

struct _RsvgTspan {
	gdouble x, y;
	gboolean hasx, hasy;
	gdouble dx, dy;
	RsvgTspan * parent;
};

static void
rsvg_tspan_free(RsvgTspan * self)
{
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
	return self;
}

static gdouble
rsvg_tspan_x(RsvgTspan * self)
{
	RsvgTspan * i;
	for (i = self; i->hasx == FALSE; i = i->parent)
		;
	return i->x;
}

static gdouble
rsvg_tspan_y(RsvgTspan * self)
{
	RsvgTspan * i;
	for (i = self; i->hasy == FALSE; i = i->parent)
		;
	return i->y;
}

static void
rsvg_tspan_set_x(RsvgTspan * self, gdouble setto)
{
	RsvgTspan * i;
	for (i = self; i->hasx == FALSE; i = i->parent)
		;
	i->x = setto;
}

/*
static void
rsvg_tspan_set_y(RsvgTspan * self, gdouble setto)
{
	RsvgTspan * i;
	for (i = self; i->hasy == FALSE; i = i->parent)
		;
	i->y = setto;
}
*/

static gdouble
rsvg_tspan_dx(RsvgTspan * self)
{
	RsvgTspan * i;
	gdouble total;

	total = 0;
	for (i = self; i->hasx == FALSE; i = i->parent)
		total += i->dx;
	total += i->dx;
	return total;
}

static gdouble
rsvg_tspan_dy(RsvgTspan * self)
{
	RsvgTspan * i;
	gdouble total;

	total = 0;
	for (i = self; i->hasy == FALSE; i = i->parent)
		total += i->dy;
	total += i->dy;
	return total;
}

typedef struct _RsvgSaxHandlerText {
	RsvgSaxHandler super;
	RsvgSaxHandler *parent;
	RsvgHandle *ctx;
	GString * id;
	RsvgTspan * tspan;
	RsvgTspan * innerspan;
	RsvgDefsDrawableText * block;
} RsvgSaxHandlerText;

static void
rsvg_text_handler_free (RsvgSaxHandler *self)
{
	RsvgSaxHandlerText * z;
	z = (RsvgSaxHandlerText *)self;

	rsvg_tspan_free(z->tspan);
	g_string_free(z->id, TRUE);
	g_free (self);
}

void 
rsvg_text_render_text (RsvgSaxHandlerText *ctx,
					   RsvgState  *state,
					   const char *text,
					   const char *id);

static void
rsvg_text_handler_characters (RsvgSaxHandler *self, const xmlChar *ch, int len)
{
	char *string, *tmp;
	int beg, end;
	
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

	g_free (string);
}

static void
rsvg_start_tspan (RsvgSaxHandlerText *self, RsvgPropertyBag *atts)
{
	RsvgState state;
	RsvgHandle *ctx;
	RsvgSaxHandlerText *z;
	RsvgTspan * tspan;
	const char * klazz = NULL, * id = NULL, *value;
	double font_size;
	tspan = rsvg_tspan_new();
	z = (RsvgSaxHandlerText *)self;
	ctx = z->ctx;
	rsvg_state_init(&state);
	font_size = rsvg_state_current_font_size(ctx);

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
	
	tspan->parent = z->innerspan;
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
			rsvg_tspan_free(child);
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
rsvg_defs_drawable_text_free (RsvgDefVal *self)
{
	RsvgDefsDrawableText *z = (RsvgDefsDrawableText *)self;
	unsigned int i;
	rsvg_state_finalize (&z->super.state);
	for (i = 0; i < z->chunks->len; i++) {
		rsvg_state_finalize (g_ptr_array_index(z->styles, i));
		g_free (g_ptr_array_index(z->styles, i));
		g_free (g_ptr_array_index(z->chunks, i));
	}
	g_ptr_array_free(z->styles, 1);
	g_ptr_array_free(z->chunks, 1);
	g_free (z);
}

static void 
rsvg_defs_drawable_text_draw (RsvgDefsDrawable * self, DrawingCtx *ctx, 
							  int dominate)
{
	RsvgDefsDrawableText *text = (RsvgDefsDrawableText*)self;
	unsigned int i;

	rsvg_state_reinherit_top(ctx, &self->state, dominate);
	for (i = 0; i < text->chunks->len; i++) {
		rsvg_state_push(ctx);
		rsvg_state_reinherit_top(ctx, g_ptr_array_index(text->styles, i), 1);
		rsvg_render_path (ctx, g_ptr_array_index(text->chunks, i));
		rsvg_state_pop(ctx);
	}
	
}

static ArtSVP *
rsvg_defs_drawable_text_draw_as_svp (RsvgDefsDrawable * self, DrawingCtx *ctx, 
									 int dominate)
{
	RsvgDefsDrawableText *text = (RsvgDefsDrawableText*)self;
	ArtSVP * output = NULL;
	unsigned int i;

	rsvg_state_reinherit_top(ctx,  &self->state, dominate);

	for (i = 0; i < text->chunks->len; i++) {
		rsvg_state_push(ctx);
		rsvg_state_reinherit_top(ctx, g_ptr_array_index(text->styles, i), 0);
		output = rsvg_clip_path_merge(output, 
									  rsvg_render_path_as_svp (ctx, g_ptr_array_index(text->chunks, i)),
									  'u');
		rsvg_state_pop(ctx);
	}
	return output;
}

void
rsvg_start_text (RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	double x, y, dx, dy, font_size;
	const char * klazz = NULL, * id = NULL, *value;
	RsvgState state;
	RsvgDefsDrawableText *text;
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
	
	text = g_new (RsvgDefsDrawableText, 1);
	text->chunks = g_ptr_array_new();
	text->styles = g_ptr_array_new();
	text->super.state = state;
	text->super.super.type = RSVG_DEF_PATH;
	text->super.super.free = rsvg_defs_drawable_text_free;
	text->super.draw = rsvg_defs_drawable_text_draw;
	text->super.draw_as_svp = rsvg_defs_drawable_text_draw_as_svp;
	rsvg_defs_set (ctx->defs, id, &text->super.super);
	
	text->super.parent = (RsvgDefsDrawable *)ctx->current_defs_group;
	if (text->super.parent != NULL)
		rsvg_defs_drawable_group_pack((RsvgDefsDrawableGroup *)text->super.parent, 
									  &text->super);


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

	handler->parent = ctx->handler;
	ctx->handler = &handler->super;
}

typedef struct _RsvgTextLayout RsvgTextLayout;

struct _RsvgTextLayout
{
	PangoLayout * layout;
	RsvgHandle  * ctx;
	RsvgSaxHandlerText * th;
	TextAnchor anchor;
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
rsvg_text_get_pango_context (RsvgHandle *ctx)
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
rsvg_text_layout_new (RsvgHandle *ctx,
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

static void
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
			rsvg_text_layout_render_glyphs (layout,
											run->item->analysis.font, run->glyphs,
											render_func,
											x + x_off, y,
											render_data);
			
			x_off += rect.width;
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
	gdouble          xshift;
	gint anchoroffset;

	xshift = 0;

	rsvg_text_layout_get_offsets (layout, &offx, &offy);

	offx += rsvg_tspan_dx(layout->th->innerspan);
	offy += rsvg_tspan_dy(layout->th->innerspan);
   

	x = offx + rsvg_tspan_x(layout->th->innerspan);
	y = offy + rsvg_tspan_y(layout->th->innerspan);

	x *= PANGO_SCALE;
	y *= PANGO_SCALE;
	
	iter = pango_layout_get_iter (layout->layout);
	
	do
		{
			PangoRectangle   rect;
			PangoLayoutLine *line;
			gint             baseline;
			
			line = pango_layout_iter_get_line (iter);
			
			pango_layout_iter_get_line_extents (iter, NULL, &rect);
			baseline = pango_layout_iter_get_baseline (iter);
			
			if (layout->anchor == TEXT_ANCHOR_START)
				anchoroffset = 0;
			else if (layout->anchor == TEXT_ANCHOR_MIDDLE)
				anchoroffset = rect.width / 2;
			else
				anchoroffset = rect.width;
			
			rsvg_text_layout_render_line (layout, line,
										  render_func,
										  x + rect.x - anchoroffset,
										  y + baseline,
										  render_data);
			xshift += rect.width;
		}
	while (pango_layout_iter_next_line (iter));
	
	pango_layout_iter_free (iter);

	rsvg_tspan_set_x(layout->th->innerspan, (x + xshift) / PANGO_SCALE);

}

static void
rsvg_text_add_chunk(RsvgDefsDrawableText *text, const char *d, RsvgHandle *ctx)
{
	RsvgState * toinsert = g_new(RsvgState, 1);
	char *cd = g_strdup(d);

	/*
	rsvg_state_clone(toinsert, rsvg_state_current(ctx));
	*/
	g_ptr_array_add(text->chunks, cd);
	g_ptr_array_add(text->styles, toinsert);

}

void 
rsvg_text_render_text (RsvgSaxHandlerText *self,
					   RsvgState  *state,
					   const char *text,
					   const char *id)
{
	RsvgTextLayout *layout;
	RenderCtx      *render;
	RsvgHandle     *ctx = self->ctx;
	RsvgDefsDrawableText *block;

	state->fill_rule = FILL_RULE_EVENODD;	
	state->has_fill_rule = TRUE;

	layout = rsvg_text_layout_new (ctx, state, text);
	layout->th = self;
	render = rsvg_render_ctx_new ();

	rsvg_text_layout_render (layout, rsvg_text_render_vectors, 
							 (gpointer)render);

	if (render->wrote)
		g_string_append_c(render->path, 'Z');

#ifdef RSVG_TEXT_DEBUG
	fprintf(stderr, "%s\n", render->path->str);
#endif

	block = self->block;
	
	rsvg_text_add_chunk (block, render->path->str, ctx);
	rsvg_render_ctx_free (render);
	rsvg_text_layout_free (layout);
}
