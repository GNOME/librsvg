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
#include "rsvg-text.h"
#include "rsvg-css.h"

#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_render_mask.h>

#if ENABLE_TEXT_DECOR

#include "rsvg-shapes.h"

static void 
rsvg_draw_hline (RsvgHandle *ctx, double x, double w, double y)
{
	char buf [G_ASCII_DTOSTR_BUF_SIZE];

	/* ("M %f %f L %f %f", x, y, x+w, y) */
	GString * d = g_string_new ("M ");   

	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));
	g_string_append (d, " L ");	
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), x+w));
	g_string_append_c (d, ' ');
	g_string_append (d, g_ascii_dtostr (buf, sizeof (buf), y));   

	rsvg_render_path (ctx, d->str);

	g_string_free (d, TRUE);
}

#endif /* ENABLE_TEXT_DECOR */

static char *
make_valid_utf8 (const char *str)
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
	ArtRender *render;
	GdkPixbuf *pixbuf;
	gboolean has_alpha;
	int opacity;
	PangoLayout *layout;
	PangoFontDescription *font;
	PangoLayoutLine *line;
	PangoRectangle ink_rect, line_ink_rect;
	FT_Bitmap bitmap;
	
	state = &ctx->state[ctx->n_state - 1];
	if (state->fill == NULL && state->font_size <= 0)
		return;

	/* not quite up to spec, but good enough */
	if (!state->visible)
		return;

	pixbuf = ctx->pixbuf;
	if (pixbuf == NULL)
    {
		/* FIXME: What warning/GError here? */
		return;
    }
	
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
			tmp = make_valid_utf8 (string);
			g_free (string);
			string = tmp;
		}
	
	if (ctx->pango_context == NULL)
		ctx->pango_context = pango_ft2_get_context ((guint)ctx->dpi, (guint)ctx->dpi);
	
	has_alpha = gdk_pixbuf_get_has_alpha (pixbuf);
	
	render = art_render_new (0, 0,
							 gdk_pixbuf_get_width (pixbuf),
							 gdk_pixbuf_get_height (pixbuf),
							 gdk_pixbuf_get_pixels (pixbuf),
							 gdk_pixbuf_get_rowstride (pixbuf),
							 gdk_pixbuf_get_n_channels (pixbuf) -
							 (has_alpha ? 1 : 0),
							 gdk_pixbuf_get_bits_per_sample (pixbuf),
							 has_alpha ? ART_ALPHA_SEPARATE : ART_ALPHA_NONE,
							 NULL);
	
	layout = pango_layout_new (ctx->pango_context);
	pango_layout_set_text (layout, string, end - beg);
	font = pango_font_description_copy (pango_context_get_font_description (ctx->pango_context));

	/* we need to resize the font by our X or Y scale (ideally could stretch in both directions...)
	   which, though? Y for now */
	pango_font_description_set_size (font, state->font_size * PANGO_SCALE * state->affine[3]);
	
	if (state->font_family)
		pango_font_description_set_family_static (font, state->font_family);
	
	pango_font_description_set_style (font, state->font_style);
	pango_font_description_set_variant (font, state->font_variant);
	pango_font_description_set_weight (font, state->font_weight);
	pango_font_description_set_stretch (font, state->font_stretch);
  
	pango_layout_set_font_description (layout, font);
	pango_font_description_free (font);
	
	pango_layout_get_pixel_extents (layout, &ink_rect, NULL);
	
	line = pango_layout_get_line (layout, 0);
	if (line == NULL)
		line_ink_rect = ink_rect; /* nothing to draw anyway */
	else
		pango_layout_line_get_pixel_extents (line, &line_ink_rect, NULL);
	
	bitmap.rows = ink_rect.height;
	bitmap.width = ink_rect.width;
	bitmap.pitch = (bitmap.width + 3) & ~3;
	bitmap.buffer = g_malloc0 (bitmap.rows * bitmap.pitch);
	bitmap.num_grays = 0x100;
	bitmap.pixel_mode = ft_pixel_mode_grays;
	
	pango_ft2_render_layout (&bitmap, layout, -ink_rect.x, -ink_rect.y);
	
	g_object_unref (layout);
	
	rsvg_render_paint_server (render, state->fill, NULL); /* todo: paint server ctx */
	opacity = state->fill_opacity * state->opacity;
	opacity = opacity + (opacity >> 7) + (opacity >> 14);
	
	art_render_mask_solid (render, opacity);
	art_render_mask (render,
					 state->affine[4] + line_ink_rect.x + state->text_offset,
					 state->affine[5] + line_ink_rect.y,
					 state->affine[4] + line_ink_rect.x + bitmap.width + state->text_offset,
					 state->affine[5] + line_ink_rect.y + bitmap.rows,
					 bitmap.buffer, bitmap.pitch);
	art_render_invoke (render);
	
	g_free (bitmap.buffer);
	g_free (string);
	
#if ENABLE_TEXT_DECOR
	if (state->font_decor & TEXT_OVERLINE)
		rsvg_draw_hline (ctx, 0, line_ink_rect.width, line_ink_rect.y - line_ink_rect.height);
	if (state->font_decor & TEXT_UNDERLINE)
		rsvg_draw_hline (ctx, 0, line_ink_rect.width, line_ink_rect.y);
	if (state->font_decor & TEXT_STRIKE)
		rsvg_draw_hline (ctx, 0, ink_rect.width, line_ink_rect.y - (line_ink_rect.height/2));
#endif
	
	state->text_offset += line_ink_rect.width;
}

void
rsvg_start_tspan (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double affine[6] ;
	double x, y, dx, dy;
	RsvgState *state;
	const char * klazz = NULL, * id = NULL;
	x = y = dx = dy = 0.;
	
  state = &ctx->state[ctx->n_state - 1];
  
  if (atts != NULL)
	  {
		  for (i = 0; atts[i] != NULL; i += 2)
			  {
				  if (!strcmp ((char *)atts[i], "x"))
					  x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
				  else if (!strcmp ((char *)atts[i], "y"))
					  y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
				  else if (!strcmp ((char *)atts[i], "dx"))
					  dx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
				  else if (!strcmp ((char *)atts[i], "dy"))
					  dy = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
				  else if (!strcmp ((char *)atts[i], "class"))
					  klazz = (const char *)atts[i + 1];
				  else if (!strcmp ((char *)atts[i], "id"))
					  id = (const char *)atts[i + 1];
			  }
	  }
  
  /* todo: transform() is illegal here */
  x += dx ;
  y += dy ;
  
  if (x > 0 && y > 0)
	  {
		  art_affine_translate (affine, x, y);
		  art_affine_multiply (state->affine, affine, state->affine);
	  }
  rsvg_parse_style_attrs (ctx, "tspan", klazz, id, atts);
}

static void
rsvg_text_handler_start (RsvgSaxHandler *self, const xmlChar *name,
						 const xmlChar **atts)
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
rsvg_start_text (RsvgHandle *ctx, const xmlChar **atts)
{
	int i;
	double affine[6] ;
	double x, y, dx, dy;
	const char * klazz = NULL, * id = NULL;
	RsvgState *state;
	
	RsvgSaxHandlerText *handler = g_new0 (RsvgSaxHandlerText, 1);
	handler->super.free = rsvg_text_handler_free;
	handler->super.characters = rsvg_text_handler_characters;
	handler->super.start_element = rsvg_text_handler_start;
	handler->super.end_element   = rsvg_text_handler_end;
	handler->ctx = ctx;
	
	x = y = dx = dy = 0.;
	
	state = &ctx->state[ctx->n_state - 1];
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "x"))
						x = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
					else if (!strcmp ((char *)atts[i], "y"))
						y = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
					else if (!strcmp ((char *)atts[i], "dx"))
						dx = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->width, state->font_size);
					else if (!strcmp ((char *)atts[i], "dy"))
						dy = rsvg_css_parse_normalized_length ((char *)atts[i + 1], ctx->dpi, (gdouble)ctx->height, state->font_size);
					else if (!strcmp ((char *)atts[i], "class"))
						klazz = (const char *)atts[i + 1];
					else if (!strcmp ((char *)atts[i], "id"))
						id = (const char *)atts[i + 1];
				}
		}

	x += dx ;
	y += dy ;
	
	art_affine_translate (affine, x, y);
	art_affine_multiply (state->affine, affine, state->affine);
	
	rsvg_parse_style_attrs (ctx, "text", klazz, id, atts);
	
	handler->parent = ctx->handler;
	ctx->handler = &handler->super;
}
