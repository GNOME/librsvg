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

typedef struct _RsvgNodeText RsvgNodeText;

struct _RsvgNodeText {
	RsvgNode super;
	RsvgLength x, y, dx, dy;
};

typedef struct _RsvgNodeTref RsvgNodeTref;

struct _RsvgNodeTref {
	RsvgNode super;
	RsvgNode * link;
};
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

static GString *
_rsvg_text_chomp (GString * in, gboolean *lastwasspace)
{
	GString * out;
	guint i;
	out = g_string_new(in->str);

	for (i = 0; i < out->len;)
		{
			if (out->str[i] == '\n')
				g_string_erase(out, i, 1);
			else
				i++;
		}

	for (i = 0; i < out->len; i++)
		if (out->str[i] == '\t')
			out->str[i] = ' ';

	for (i = 0; i < out->len;)
		{
			if (out->str[i] == ' ' && *lastwasspace)
					g_string_erase(out, i, 1);
			else 
				{
					if (out->str[i] == ' ')
						*lastwasspace = TRUE;
					else
						*lastwasspace = FALSE;
					i++;
				}
		}
	
	return out;
}


static void
_rsvg_node_text_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	RsvgNodeText * text = (RsvgNodeText *)self;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				text->x = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				text->y = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				text->dx = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				text->dy = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "text", klazz, id, atts);
		}	
}

void
rsvg_text_render_text (RsvgDrawingCtx *ctx,
					   const char *text,
					   gdouble *x,
					   gdouble *y);


static void
_rsvg_node_text_type_tspan(RsvgNodeText * self, RsvgDrawingCtx *ctx, 
						   gdouble * x, gdouble * y, gboolean * lastwasspace);

static void
_rsvg_node_text_type_tref(RsvgNodeTref * self, RsvgDrawingCtx *ctx, 
						  gdouble * x, gdouble * y, gboolean * lastwasspace);

static void
_rsvg_node_text_type_children(RsvgNode * self, RsvgDrawingCtx *ctx, 
							  gdouble * x, gdouble * y, gboolean * lastwasspace)
{
	guint i;
	for (i = 0; i < self->children->len; i++)
		{
			RsvgNode * node = g_ptr_array_index(self->children, i);
			if (node->type == RSVG_NODE_CHARS)
				{
					RsvgNodeChars * chars = (RsvgNodeChars *)node;
					GString * str = _rsvg_text_chomp(chars->contents, lastwasspace);
					rsvg_text_render_text (ctx, str->str, x, y);				
					g_string_free(str, TRUE);
				}
			else if (node->type == RSVG_NODE_TSPAN)
				{
					RsvgNodeText * tspan = (RsvgNodeText *)node;
					rsvg_state_push(ctx);
					_rsvg_node_text_type_tspan(tspan, ctx, x, y, lastwasspace);
					rsvg_state_pop(ctx);
				}
			else if (node->type == RSVG_NODE_TREF)
				{
					RsvgNodeTref * tref = (RsvgNodeTref *)node;
					_rsvg_node_text_type_tref(tref, ctx, x, y, lastwasspace);
				}
		}		
}

static int
_rsvg_node_text_length_tref(RsvgNodeTref * self, RsvgDrawingCtx *ctx, 
							gdouble * x, gboolean * lastwasspace);

static int
_rsvg_node_text_length_tspan(RsvgNodeText * self, RsvgDrawingCtx *ctx, 
							 gdouble * x, gboolean * lastwasspace);

static gdouble 
rsvg_text_length_text_as_string (RsvgDrawingCtx *ctx,
								 const char *text);

static int
_rsvg_node_text_length_children(RsvgNode * self, RsvgDrawingCtx *ctx, 
								gdouble * x, gboolean * lastwasspace)
{
	guint i;
	int out = FALSE;
	for (i = 0; i < self->children->len; i++)
		{
			RsvgNode * node = g_ptr_array_index(self->children, i);
			if (node->type == RSVG_NODE_CHARS)
				{
					RsvgNodeChars * chars = (RsvgNodeChars *)node;
					GString * str = _rsvg_text_chomp(chars->contents, lastwasspace);
					*x += rsvg_text_length_text_as_string(ctx, str->str);
					g_string_free(str, TRUE);
				}
			else if (node->type == RSVG_NODE_TSPAN)
				{
					RsvgNodeText * tspan = (RsvgNodeText *)node;
					out = _rsvg_node_text_length_tspan(tspan, ctx, x, lastwasspace);
				}
			else if (node->type == RSVG_NODE_TREF)
				{
					RsvgNodeTref * tref = (RsvgNodeTref *)node;
					out = _rsvg_node_text_length_tref(tref, ctx, x, lastwasspace);
				}
			if (out)
				break;
		}
	return out;
}


static void
_rsvg_node_text_draw(RsvgNode * self, RsvgDrawingCtx *ctx, 
					 int dominate)
{
	double x, y;
	gboolean lastwasspace = TRUE;
	RsvgNodeText * text = (RsvgNodeText *)self;
	rsvg_state_reinherit_top(ctx, self->state, dominate);

	x = _rsvg_css_normalize_length(&text->x, ctx, 'h');
	y = _rsvg_css_normalize_length(&text->y, ctx, 'v');
	x += _rsvg_css_normalize_length(&text->dx, ctx, 'h');
	y += _rsvg_css_normalize_length(&text->dy, ctx, 'v');

	if (rsvg_state_current(ctx)->text_anchor != TEXT_ANCHOR_START)
		{
			double length = 0;
			_rsvg_node_text_length_children(self, ctx, &length, &lastwasspace);
			if (rsvg_state_current(ctx)->text_anchor == TEXT_ANCHOR_END)
				x -= length;
			if (rsvg_state_current(ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
				x -= length / 2;
		}

	lastwasspace = TRUE;
	_rsvg_node_text_type_children(self, ctx, &x, &y, &lastwasspace);
}

RsvgNode *
rsvg_new_text (void)
{
	RsvgNodeText *text;
	text = g_new (RsvgNodeText, 1);
	_rsvg_node_init(&text->super);
	text->super.type = RSVG_NODE_PATH;
	text->super.draw = _rsvg_node_text_draw;
	text->super.set_atts = _rsvg_node_text_set_atts;
	text->x = text->y = text->dx = text->dy = _rsvg_css_parse_length("0");
	return &text->super;
}

static void
_rsvg_node_text_type_tspan(RsvgNodeText * self, RsvgDrawingCtx *ctx, 
						   gdouble * x, gdouble * y, 
						   gboolean * lastwasspace)
{
	rsvg_state_reinherit_top(ctx, self->super.state, 0);

	if (self->x.factor != 'n')
		{
			*x = _rsvg_css_normalize_length(&self->x, ctx, 'h');
			if (rsvg_state_current(ctx)->text_anchor != TEXT_ANCHOR_START)
				{
					double length = 0;
					gboolean lws = *lastwasspace;
					_rsvg_node_text_length_children(&self->super, ctx, &length, &lws);
					if (rsvg_state_current(ctx)->text_anchor == TEXT_ANCHOR_END)
						*x -= length;
					if (rsvg_state_current(ctx)->text_anchor == TEXT_ANCHOR_MIDDLE)
						*x -= length / 2;
				}
		}
	if (self->y.factor != 'n')
		*y = _rsvg_css_normalize_length(&self->y, ctx, 'v');
	*x += _rsvg_css_normalize_length(&self->dx, ctx, 'h');
	*y += _rsvg_css_normalize_length(&self->dy, ctx, 'v');
	_rsvg_node_text_type_children(&self->super, ctx, x, y, lastwasspace);
}

static int
_rsvg_node_text_length_tspan(RsvgNodeText * self, RsvgDrawingCtx *ctx, gdouble * x, gboolean * lastwasspace)
{
	if (self->x.factor != 'n' || self->y.factor != 'n')
		return TRUE;
	return _rsvg_node_text_length_children(&self->super, ctx, x, lastwasspace);
}

static void
_rsvg_node_tspan_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char * klazz = NULL, * id = NULL, *value;
	RsvgNodeText * text = (RsvgNodeText *)self;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "x")))
				text->x = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "y")))
				text->y = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "dx")))
				text->dx = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "dy")))
				text->dy = _rsvg_css_parse_length (value);
			if ((value = rsvg_property_bag_lookup (atts, "class")))
				klazz = value;
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				{
					id = value;
					rsvg_defs_register_name (ctx->defs, value, self);
				}

			rsvg_parse_style_attrs (ctx, self->state, "tspan", klazz, id, atts);
		}	
}

/* generic function that doesn't draw anything at all */
static void 
_rsvg_node_draw_nothing (RsvgNode * self, RsvgDrawingCtx *ctx, 
						 int dominate)
{
}

RsvgNode *
rsvg_new_tspan (void)
{
	RsvgNodeText *text;
	text = g_new (RsvgNodeText, 1);
	_rsvg_node_init(&text->super);
	text->super.type = RSVG_NODE_TSPAN;
	text->super.draw = _rsvg_node_draw_nothing;
	text->super.set_atts = _rsvg_node_tspan_set_atts;
	text->x.factor = text->y.factor = 'n';
	text->dx = text->dy = _rsvg_css_parse_length("0");
	return &text->super;
}

static void
_rsvg_node_text_type_tref(RsvgNodeTref * self, RsvgDrawingCtx *ctx, 
						  gdouble * x, gdouble * y, gboolean * lastwasspace)
{
	if (self->link)
		_rsvg_node_text_type_children(self->link, ctx, x, y, lastwasspace);
}

static int
_rsvg_node_text_length_tref(RsvgNodeTref * self, RsvgDrawingCtx *ctx, gdouble * x, 
							gboolean * lastwasspace)
{
	if (self->link)
		return _rsvg_node_text_length_children(self->link, ctx, x, lastwasspace);
	return FALSE;
}

static void
_rsvg_node_tref_set_atts (RsvgNode * self, RsvgHandle *ctx, RsvgPropertyBag *atts)
{
	const char *value;
	RsvgNodeTref * text = (RsvgNodeTref *)self;

	if (rsvg_property_bag_size (atts))
		{
			if ((value = rsvg_property_bag_lookup (atts, "xlink:href")))
				rsvg_defs_add_resolver (ctx->defs, &text->link, value);
			if ((value = rsvg_property_bag_lookup (atts, "id")))
				rsvg_defs_register_name (ctx->defs, value, self);
		}	
}

RsvgNode *
rsvg_new_tref (void)
{
	RsvgNodeTref *text;
	text = g_new (RsvgNodeTref, 1);
	_rsvg_node_init(&text->super);
	text->super.type = RSVG_NODE_TREF;
	text->super.draw = _rsvg_node_draw_nothing;
	text->super.set_atts = _rsvg_node_tref_set_atts;
	text->link = NULL;
	return &text->super;
}

typedef struct _RsvgTextLayout RsvgTextLayout;

struct _RsvgTextLayout
{
	PangoLayout * layout;
	RsvgDrawingCtx  * ctx;
	TextAnchor anchor;
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
	pango_font_description_set_size (font_desc, _rsvg_css_normalize_length(&state->font_size, ctx, 'v') * PANGO_SCALE / ctx->dpi_y * 72); 
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
					   const char *text,
					   gdouble *x,
					   gdouble *y)
{
	GString * render;
	render = rsvg_text_render_text_as_string (ctx,text,x,y);
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
rsvg_text_length_text_as_string (RsvgDrawingCtx *ctx,
								 const char *text)
{
	RsvgTextLayout *layout;
	gdouble x;

	layout = rsvg_text_layout_new (ctx, rsvg_state_current(ctx), text);
	layout->x = layout->y = 0;
	layout->orientation = rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_LTR || 
		rsvg_state_current(ctx)->text_dir == PANGO_DIRECTION_TTB_RTL;

	x = rsvg_text_layout_width(layout);

	rsvg_text_layout_free (layout);
	return x;
}
