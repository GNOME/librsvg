/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   rsvg-styles.c: Handle SVG styles

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

#include "rsvg.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-private.h"

#include <libart_lgpl/art_rgba.h>
#include <libart_lgpl/art_affine.h>

/* Our default font */
#define DEFAULT_FONT "Times Roman"

gdouble
rsvg_viewport_percentage (gdouble width, gdouble height)
{
	return ((width * width) + (height * height)) / M_SQRT2;
}

void
rsvg_state_init (RsvgState *state)
{
	memset (state, 0, sizeof (*state));
	
	art_affine_identity (state->affine);
	
	state->opacity = 0xff;
	state->fill = rsvg_paint_server_parse (NULL, "#000");
	state->fill_opacity = 0xff;
	state->stroke_opacity = 0xff;
	state->stroke_width = 1;
	state->miter_limit = 4;
	state->cap = ART_PATH_STROKE_CAP_BUTT;
	state->join = ART_PATH_STROKE_JOIN_MITER;
	state->stop_opacity = 0xff;
	
	state->font_family  = g_strdup (DEFAULT_FONT);
	state->font_size    = 12.0;
	state->font_style   = PANGO_STYLE_NORMAL;
	state->font_variant = PANGO_VARIANT_NORMAL;
	state->font_weight  = PANGO_WEIGHT_NORMAL;
	state->font_stretch = PANGO_STRETCH_NORMAL;

	state->visible = TRUE;
}

void
rsvg_state_clone (RsvgState *dst, const RsvgState *src)
{
	gint i;
	
	*dst = *src;
	dst->font_family = g_strdup (src->font_family);
	rsvg_paint_server_ref (dst->fill);
	rsvg_paint_server_ref (dst->stroke);
	dst->save_pixbuf = NULL;
	
	if (src->dash.n_dash > 0)
		{
			dst->dash.dash = g_new (gdouble, src->dash.n_dash);
			for (i = 0; i < src->dash.n_dash; i++)
				dst->dash.dash[i] = src->dash.dash[i];
		}
}

void
rsvg_state_finalize (RsvgState *state)
{
	g_free (state->font_family);
	rsvg_paint_server_unref (state->fill);
	rsvg_paint_server_unref (state->stroke);
	
	if (state->dash.n_dash != 0)
		g_free (state->dash.dash);
}

/* Parse a CSS2 style argument, setting the SVG context attributes. */
static void
rsvg_parse_style_arg (RsvgHandle *ctx, RsvgState *state, const char *str)
{
	RsvgState * parent_state = state; /* TODO: temporary hack */
	int arg_off;
	
	arg_off = rsvg_css_param_arg_offset (str);
	if (rsvg_css_param_match (str, "opacity"))
		{
			state->opacity = rsvg_css_parse_opacity (str + arg_off);
		}
	else if (rsvg_css_param_match (str, "display"))
		{
			if (!strcmp (str + arg_off, "none"))
				state->visible = FALSE;
			else if (strcmp (str + arg_off, "inherit") != 0)
				state->visible = TRUE;
			/* else inherit */
		}
	else if (rsvg_css_param_match (str, "visibility"))
		{
			if (!strcmp (str + arg_off, "visable"))
				state->visible = TRUE;
			else if (strcmp (str + arg_off, "inherit") != 0)
				state->visible = FALSE; /* collapse or hidden */
			/* else inherit */
		}
	else if (rsvg_css_param_match (str, "fill"))
		{
			rsvg_paint_server_unref (state->fill);
			state->fill = rsvg_paint_server_parse (ctx->defs, str + arg_off);
		}
	else if (rsvg_css_param_match (str, "fill-opacity"))
		{
			state->fill_opacity = rsvg_css_parse_opacity (str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stroke"))
		{
			rsvg_paint_server_unref (state->stroke);
			state->stroke = rsvg_paint_server_parse (ctx->defs, str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stroke-width"))
		{
			state->stroke_width = rsvg_css_parse_normalized_length (str + arg_off, ctx->dpi, 
																	(gdouble)ctx->height, state->font_size);
		}
	else if (rsvg_css_param_match (str, "stroke-linecap"))
		{
			if (!strcmp (str + arg_off, "butt"))
				state->cap = ART_PATH_STROKE_CAP_BUTT;
			else if (!strcmp (str + arg_off, "round"))
				state->cap = ART_PATH_STROKE_CAP_ROUND;
			else if (!strcmp (str + arg_off, "square"))
				state->cap = ART_PATH_STROKE_CAP_SQUARE;
			else
				g_warning ("unknown line cap style %s", str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stroke-opacity"))
		{
			state->stroke_opacity = rsvg_css_parse_opacity (str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stroke-linejoin"))
		{
			if (!strcmp (str + arg_off, "miter"))
				state->join = ART_PATH_STROKE_JOIN_MITER;
			else if (!strcmp (str + arg_off, "round"))
				state->join = ART_PATH_STROKE_JOIN_ROUND;
			else if (!strcmp (str + arg_off, "bevel"))
				state->join = ART_PATH_STROKE_JOIN_BEVEL;
			else
				g_warning ("unknown line join style %s", str + arg_off);
		}
	else if (rsvg_css_param_match (str, "font-size"))
		{
			state->font_size = rsvg_css_parse_normalized_length (str + arg_off, ctx->dpi, 
																 (gdouble)ctx->height, state->font_size);
		}
	else if (rsvg_css_param_match (str, "font-family"))
		{
			char * save = g_strdup (rsvg_css_parse_font_family (str + arg_off, parent_state->font_family));
			g_free (state->font_family);
			state->font_family = save;
		}
	else if (rsvg_css_param_match (str, "font-style"))
		{
			state->font_style = rsvg_css_parse_font_style (str + arg_off, parent_state->font_style);
		}
	else if (rsvg_css_param_match (str, "font-variant"))
		{
			state->font_variant = rsvg_css_parse_font_variant (str + arg_off, parent_state->font_variant);
		}
	else if (rsvg_css_param_match (str, "font-weight"))
		{
			state->font_weight = rsvg_css_parse_font_weight (str + arg_off, parent_state->font_weight);
		}
	else if (rsvg_css_param_match (str, "font-stretch"))
		{
			state->font_stretch = rsvg_css_parse_font_stretch (str + arg_off, parent_state->font_stretch);
		}
	else if (rsvg_css_param_match (str, "text-decoration"))
		{
			if (!strcmp (str, "inherit"))
				state->font_decor = parent_state->font_decor;
			else 
				{
					if (strstr (str, "underline"))
						state->font_decor |= TEXT_UNDERLINE;
					if (strstr (str, "overline"))
						state->font_decor |= TEXT_OVERLINE;
					if (strstr (str, "strike") || strstr (str, "line-through")) /* strike though or line-through */
						state->font_decor |= TEXT_STRIKE;
				}
		}
	else if (rsvg_css_param_match (str, "stop-color"))
		{
			state->stop_color = rsvg_css_parse_color (str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stop-opacity"))
		{
			state->stop_opacity = rsvg_css_parse_opacity (str + arg_off);
		}
	else if (rsvg_css_param_match (str, "stroke-miterlimit"))
		{
			state->miter_limit = g_ascii_strtod (str + arg_off, NULL);
		}
	else if (rsvg_css_param_match (str, "stroke-dashoffset"))
		{
			state->dash.offset = rsvg_css_parse_normalized_length (str + arg_off, ctx->dpi, 
																   rsvg_viewport_percentage((gdouble)ctx->width, (gdouble)ctx->height), state->font_size);
			if (state->dash.offset < 0.)
				state->dash.offset = 0.;
		}
	else if (rsvg_css_param_match (str, "stroke-dasharray"))
		{
			if(!strcmp(str + arg_off, "none"))
				{
					if (state->dash.n_dash != 0)
						{
							/* free any cloned dash data */
							g_free (state->dash.dash);
							state->dash.n_dash = 0; 
						}
				}
			else
				{
					gchar ** dashes = g_strsplit (str + arg_off, ",", -1);
					if (NULL != dashes)
						{
							gint n_dashes, i;
							gboolean is_even = FALSE ;
							
							/* count the #dashes */
							for (n_dashes = 0; dashes[n_dashes] != NULL; n_dashes++)
								;
							
							is_even = (n_dashes % 2 == 0);
							state->dash.n_dash = (is_even ? n_dashes : n_dashes * 2);
							state->dash.dash = g_new (double, state->dash.n_dash);
							
							/* TODO: handle negative value == error case */
							
							/* the even and base case */
							for (i = 0; i < n_dashes; i++)
								state->dash.dash[i] = g_ascii_strtod (dashes[i], NULL);
							
							/* if an odd number of dashes is found, it gets repeated */
							if (!is_even)
								for (; i < state->dash.n_dash; i++)
									state->dash.dash[i] = g_ascii_strtod (dashes[i - n_dashes], NULL);
							
							g_strfreev (dashes) ;
						}
				}
		}
}

/* tell whether @str is a supported style argument 
   whenever something gets added to parse_arg, please
   remember to add it here too
*/
gboolean
rsvg_is_style_arg(const char *str)
{
	static GHashTable *styles = NULL;
	if (!styles)
		{
			styles = g_hash_table_new (g_str_hash, g_str_equal);
			
			g_hash_table_insert (styles, "display",           GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "fill",              GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "fill-opacity",      GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-family",       GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-size",         GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-stretch",      GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-style",        GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-variant",      GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "font-weight",       GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "opacity",           GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stop-color",        GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stop-opacity",      GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke",            GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-dasharray",  GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-dashoffset", GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-linecap",    GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-linejoin",   GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-miterlimit", GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-opacity",    GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "stroke-width",      GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "text-decoration",   GINT_TO_POINTER (TRUE));
			g_hash_table_insert (styles, "visibility",        GINT_TO_POINTER (TRUE));
		}
	
	/* this will default to 0 (FALSE) on a failed lookup */
	return GPOINTER_TO_INT (g_hash_table_lookup (styles, str)); 
}

/* take a pair of the form (fill="#ff00ff") and parse it as a style */
void
rsvg_parse_style_pair (RsvgHandle *ctx, RsvgState *state, 
					   const char *key, const char *val)
{
	gchar * str = g_strdup_printf ("%s:%s", key, val);
	rsvg_parse_style_arg (ctx, state, str);
	g_free (str);
}

/* Split a CSS2 style into individual style arguments, setting attributes
   in the SVG context.
   
   It's known that this is _way_ out of spec. A more complete CSS2
   implementation will happen later.
*/
void
rsvg_parse_style (RsvgHandle *ctx, RsvgState *state, const char *str)
{
	int start, end;
	char *arg;
	
	start = 0;
	while (str[start] != '\0')
		{
			for (end = start; str[end] != '\0' && str[end] != ';'; end++);
			arg = g_new (char, 1 + end - start);
			memcpy (arg, str + start, end - start);
			arg[end - start] = '\0';
			rsvg_parse_style_arg (ctx, state, arg);
			g_free (arg);
			start = end;
			if (str[start] == ';') start++;
			while (str[start] == ' ') start++;
		}
}

static void
rsvg_css_define_style (RsvgHandle *ctx, const char * style_name, const char * style_def)
{
	GString * str = g_string_new (style_def);
	char * existing = NULL;

	/* push name/style pair into HT */
	existing = (char *)g_hash_table_lookup (ctx->css_props, style_name);
	if (existing != NULL)
		g_string_append_len (str, existing, strlen (existing));
	
	/* will destroy the existing key and value for us */
	g_hash_table_insert (ctx->css_props, (gpointer)g_strdup (style_name), (gpointer)str->str);
	g_string_free (str, FALSE);
}

#ifdef HAVE_LIBCROCO

#include <libcroco.h>

typedef struct _CSSUserData
{
	RsvgHandle *ctx;
	GString    *def;
} CSSUserData;

static void
css_user_data_init (CSSUserData *user_data, RsvgHandle * ctx)
{
	user_data->ctx = ctx;
	user_data->def = NULL;
}

static void
ccss_start_selector (CRDocHandler *a_handler,
					 CRSelector *a_selector_list)
{
	CSSUserData * user_data;

	g_return_if_fail (a_handler);

	user_data = (CSSUserData *)a_handler->app_data;
	user_data->def = g_string_new (NULL);
}

static void
ccss_end_selector (CRDocHandler *a_handler,
				   CRSelector *a_selector_list)
{
	CSSUserData * user_data;
	CRSelector  * list;

	g_return_if_fail (a_handler);

	user_data = (CSSUserData *)a_handler->app_data;

	if (a_selector_list)
        {
			for (list = a_selector_list; list != NULL; list = list->next) {			
				/* iterate through the selector list, insert style into map - strdup it! */
				rsvg_css_define_style (user_data->ctx, list->simple_sel->name->str, user_data->def->str);
			}
		}

	g_string_free (user_data->def, TRUE);
}

static void
ccss_property (CRDocHandler *a_handler, GString *a_name, CRTerm *a_expr)
{
	CSSUserData * user_data;
	char * expr = NULL;

	g_return_if_fail (a_handler);

	user_data = (CSSUserData *)a_handler->app_data;

	if (a_name && a_name->str)
        {
			if (a_expr)
                {
					expr = cr_term_to_string (a_expr);
					g_string_append_len (user_data->def, expr, strlen (expr));
					g_free (expr);
                }
        }
}

static void
init_sac_handler (CRDocHandler *a_handler)
{
	a_handler->start_document        = NULL;
	a_handler->end_document          = NULL;
	a_handler->import_style          = NULL;
	a_handler->namespace_declaration = NULL;
	a_handler->comment               = NULL;
	a_handler->start_selector        = ccss_start_selector;
	a_handler->end_selector          = ccss_end_selector;
	a_handler->property              = ccss_property;
	a_handler->start_font_face       = NULL;
	a_handler->end_font_face         = NULL;
	a_handler->start_media           = NULL;
	a_handler->end_media             = NULL;
	a_handler->start_page            = NULL;
	a_handler->end_page              = NULL;
	a_handler->ignorable_at_rule     = NULL;
}

static void
rsvg_real_parse_cssbuffer (RsvgHandle *ctx, const char * buff, size_t buflen)
{
	enum CRStatus status = CR_OK;
	CRParser *parser = NULL;
	CRDocHandler * css_handler = NULL;
	CSSUserData user_data;

	css_handler = cr_doc_handler_new ();
	init_sac_handler (css_handler);

	css_user_data_init (&user_data, ctx);
	css_handler->app_data = &user_data;

	/* TODO: fix libcroco to take in const strings */
	parser = cr_parser_new_from_buf ((char *)buff, (long)buflen, CR_UTF_8, FALSE);	
	status = cr_parser_set_sac_handler (parser, css_handler);
    
	if (status != CR_OK)
        {
			cr_parser_destroy (parser);
			return;
        }        
	
	status = cr_parser_set_use_core_grammar (parser, FALSE);
	status = cr_parser_parse (parser);
	
	cr_parser_destroy (parser);       
}

#else /* !HAVE_LIBCROCO */

/* #warning Building without libcroco support. Will experience sub-optimal CSS parsing. */

static void
rsvg_real_parse_cssbuffer (RsvgHandle *ctx, const char * buff, size_t buflen)
{
	/*
	 * Extremely poor man's CSS parser. Not robust. Not compliant.
	 * See also: http://www.w3.org/TR/REC-CSS2/syndata.html
	 */

	size_t loc = 0;
	
	while (loc < buflen)
		{
			GString * style_name  = g_string_new (NULL);
			GString * style_props = g_string_new (NULL);

			/* advance to the style's name */
			while (loc < buflen && g_ascii_isspace (buff[loc]))
				loc++;
			
			while (loc < buflen && !g_ascii_isspace (buff[loc]))
				g_string_append_c (style_name, buff[loc++]);
			
			/* advance to the first { that defines the style's properties */
			while (loc < buflen && buff[loc++] != '{' )
				;
			
			while (loc < buflen && g_ascii_isspace (buff[loc]))
				loc++;
			
			while (loc < buflen && buff[loc] != '}')
				{
					/* suck in and append our property */
					while (loc < buflen && buff[loc] != ';' && buff[loc] != '}' )
						g_string_append_c (style_props, buff[loc++]);

					if (loc == buflen || buff[loc] == '}')
						break;
					else
						{
							g_string_append_c (style_props, ';');
							
							/* advance to the next property */
							loc++;
							while (loc < buflen && g_ascii_isspace (buff[loc]))
								loc++;
						}
				}

			rsvg_css_define_style (ctx, style_name->str, style_props->str);
			g_string_free (style_name, TRUE);
			g_string_free (style_props, TRUE);
			
			loc++;
			while (loc < buflen && g_ascii_isspace (buff[loc]))
				loc++;
		}
}

#endif /* HAVE_LIBCROCO */

void
rsvg_parse_cssbuffer (RsvgHandle *ctx, const char * buff, size_t buflen)
{
	/* delegate off to the builtin or libcroco implementation */
	rsvg_real_parse_cssbuffer (ctx, buff, buflen);
}

/* Parse an SVG transform string into an affine matrix. Reference: SVG
   working draft dated 1999-07-06, section 8.5. Return TRUE on
   success. */
gboolean
rsvg_parse_transform (double dst[6], const char *src)
{
	int idx;
	char keyword[32];
	double args[6];
	int n_args;
	guint key_len;
	double tmp_affine[6];
	
	art_affine_identity (dst);
	
	idx = 0;
	while (src[idx])
		{
			/* skip initial whitespace */
			while (g_ascii_isspace (src[idx]))
				idx++;
			
			/* parse keyword */
			for (key_len = 0; key_len < sizeof (keyword); key_len++)
				{
					char c;
					
					c = src[idx];
					if (g_ascii_isalpha (c) || c == '-')
						keyword[key_len] = src[idx++];
					else
						break;
				}
			if (key_len >= sizeof (keyword))
				return FALSE;
			keyword[key_len] = '\0';
			
			/* skip whitespace */
			while (g_ascii_isspace (src[idx]))
				idx++;
			
			if (src[idx] != '(')
				return FALSE;
			idx++;
			
			for (n_args = 0; ; n_args++)
				{
					char c;
					char *end_ptr;
					
					/* skip whitespace */
					while (g_ascii_isspace (src[idx]))
						idx++;
					c = src[idx];
					if (g_ascii_isdigit (c) || c == '+' || c == '-' || c == '.')
						{
							if (n_args == sizeof(args) / sizeof(args[0]))
								return FALSE; /* too many args */
							args[n_args] = g_ascii_strtod (src + idx, &end_ptr);
							idx = end_ptr - src;
							
							while (g_ascii_isspace (src[idx]))
								idx++;
							
							/* skip optional comma */
							if (src[idx] == ',')
								idx++;
						}
					else if (c == ')')
						break;
					else
						return FALSE;
				}
			idx++;
			
			/* ok, have parsed keyword and args, now modify the transform */
			if (!strcmp (keyword, "matrix"))
				{
					if (n_args != 6)
						return FALSE;
					art_affine_multiply (dst, args, dst);
				}
			else if (!strcmp (keyword, "translate"))
				{
					if (n_args == 1)
						args[1] = 0;
					else if (n_args != 2)
						return FALSE;
					art_affine_translate (tmp_affine, args[0], args[1]);
					art_affine_multiply (dst, tmp_affine, dst);
				}
			else if (!strcmp (keyword, "scale"))
				{
					if (n_args == 1)
						args[1] = args[0];
					else if (n_args != 2)
						return FALSE;
					art_affine_scale (tmp_affine, args[0], args[1]);
					art_affine_multiply (dst, tmp_affine, dst);
				}
			else if (!strcmp (keyword, "rotate"))
				{
					if (n_args != 1)
						return FALSE;
					art_affine_rotate (tmp_affine, args[0]);
					art_affine_multiply (dst, tmp_affine, dst);
				}
			else if (!strcmp (keyword, "skewX"))
				{
					if (n_args != 1)
						return FALSE;
					art_affine_shear (tmp_affine, args[0]);
					art_affine_multiply (dst, tmp_affine, dst);
				}
			else if (!strcmp (keyword, "skewY"))
				{
					if (n_args != 1)
						return FALSE;
					art_affine_shear (tmp_affine, args[0]);
					/* transpose the affine, given that we know [1] is zero */
					tmp_affine[1] = tmp_affine[2];
					tmp_affine[2] = 0;
					art_affine_multiply (dst, tmp_affine, dst);
				}
			else
				return FALSE; /* unknown keyword */
		}
	return TRUE;
}

/**
 * rsvg_parse_transform_attr: Parse transform attribute and apply to state.
 * @ctx: Rsvg context.
 * @state: State in which to apply the transform.
 * @str: String containing transform.
 *
 * Parses the transform attribute in @str and applies it to @state.
 **/
static void
rsvg_parse_transform_attr (RsvgHandle *ctx, RsvgState *state, const char *str)
{
	double affine[6];
	
	if (rsvg_parse_transform (affine, str))
		{
			art_affine_multiply (state->affine, affine, state->affine);
		}
	else
		{
			/* parse error for transform attribute. todo: report */
		}
}

static gboolean
rsvg_lookup_apply_css_style (RsvgHandle *ctx, const char * target)
{
	const char * value = (const char *)g_hash_table_lookup (ctx->css_props, target);
	
	if (value != NULL)
		{
			rsvg_parse_style (ctx, &ctx->state[ctx->n_state - 1],
							  value);
			return TRUE;
		}
	return FALSE;
}

/**
 * rsvg_parse_style_attrs: Parse style attribute.
 * @ctx: Rsvg context.
 * @tag: The SVG tag we're processing (eg: circle, ellipse), optionally %NULL
 * @klazz: The space delimited class list, optionally %NULL
 * @atts: Attributes in SAX style.
 *
 * Parses style and transform attributes and modifies state at top of
 * stack.
 **/
void
rsvg_parse_style_attrs (RsvgHandle *ctx, 
						const char * tag,
						const char * klazz,
						const char * id,
						const xmlChar **atts)
{
	int i = 0, j = 0;
	char * target = NULL;
	gboolean found = FALSE;
	GString * klazz_list = NULL;
	
	/* handle the all-encompassing "star" entry first, ignoring found-ness */
	rsvg_lookup_apply_css_style (ctx, "*");

	if (id != NULL)
		{
			target = g_strdup_printf ("#%s", id);
			rsvg_lookup_apply_css_style (ctx, target);
			g_free (target);
		}

	/* todo: see if klazz or tag should accumulate onto tag.klazz */

	if (tag != NULL && klazz != NULL)
		{
			target = g_strdup_printf ("%s.%s", tag, klazz);
			found = rsvg_lookup_apply_css_style (ctx, target);
			g_free (target);
		}
	
	if (found == FALSE)
		{
			if (tag != NULL)
				rsvg_lookup_apply_css_style (ctx, tag);
			
			if (klazz != NULL)
				{
					i = strlen (klazz);
					while (j < i)
						{
							klazz_list = g_string_new (".");
							
							while (j < i && g_ascii_isspace(klazz[j]))
								j++;
							
							while (j < i && !g_ascii_isspace(klazz[j]))
								g_string_append_c (klazz_list, klazz[j++]);
							
							rsvg_lookup_apply_css_style (ctx, klazz_list->str);
							g_string_free (klazz_list, TRUE);
						}
				}
		}
	
	if (atts != NULL)
		{
			for (i = 0; atts[i] != NULL; i += 2)
				{
					if (!strcmp ((char *)atts[i], "style"))
						rsvg_parse_style (ctx, &ctx->state[ctx->n_state - 1],
										  (char *)atts[i + 1]);
					else if (!strcmp ((char *)atts[i], "transform"))
						rsvg_parse_transform_attr (ctx, &ctx->state[ctx->n_state - 1],
												   (char *)atts[i + 1]);
					else if (rsvg_is_style_arg ((char *)atts[i]))
						rsvg_parse_style_pair (ctx, &ctx->state[ctx->n_state - 1],
											   (char *)atts[i], (char *)atts[i + 1]);
				}
		}
}

static void
rsvg_pixmap_destroy (guchar *pixels, gpointer data)
{
  g_free (pixels);
}

/**
 * rsvg_push_opacity_group: Begin a new transparency group.
 * @ctx: Context in which to push.
 *
 * Pushes a new transparency group onto the stack. The top of the stack
 * is stored in the context, while the "saved" value is in the state
 * stack.
 **/
void
rsvg_push_opacity_group (RsvgHandle *ctx)
{
	RsvgState *state;
	GdkPixbuf *pixbuf;
	art_u8 *pixels;
	int width, height, rowstride;
	
	state = &ctx->state[ctx->n_state - 1];
	pixbuf = ctx->pixbuf;
	
	state->save_pixbuf = pixbuf;
	
	if (pixbuf == NULL)
		{
			/* FIXME: What warning/GError here? */
			return;
		}

	if (!gdk_pixbuf_get_has_alpha (pixbuf))
    {
		g_warning ("push/pop transparency group on non-alpha buffer nyi");
		return;
    }
	
	width = gdk_pixbuf_get_width (pixbuf);
	height = gdk_pixbuf_get_height (pixbuf);
	rowstride = gdk_pixbuf_get_rowstride (pixbuf);
	pixels = g_new (art_u8, rowstride * height);
	memset (pixels, 0, rowstride * height);
	
	pixbuf = gdk_pixbuf_new_from_data (pixels,
									   GDK_COLORSPACE_RGB,
									   TRUE,
									   gdk_pixbuf_get_bits_per_sample (pixbuf),
									   width,
									   height,
									   rowstride,
									   rsvg_pixmap_destroy,
									   NULL);
	ctx->pixbuf = pixbuf;
}

/**
 * rsvg_pop_opacity_group: End a transparency group.
 * @ctx: Context in which to push.
 * @opacity: Opacity for blending (0..255).
 *
 * Pops a new transparency group from the stack, recompositing with the
 * next on stack.
 **/
void
rsvg_pop_opacity_group (RsvgHandle *ctx, int opacity)
{
	RsvgState *state = &ctx->state[ctx->n_state - 1];
	GdkPixbuf *tos, *nos;
	art_u8 *tos_pixels, *nos_pixels;
	int width;
	int height;
	int rowstride;
	int x, y;
	int tmp;
	
	tos = ctx->pixbuf;
	nos = state->save_pixbuf;
	
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
	
	for (y = 0; y < height; y++)
		{
			for (x = 0; x < width; x++)
				{
					art_u8 r, g, b, a;
					a = tos_pixels[4 * x + 3];
					if (a)
						{
							r = tos_pixels[4 * x];
							g = tos_pixels[4 * x + 1];
							b = tos_pixels[4 * x + 2];
							tmp = a * opacity + 0x80;
							a = (tmp + (tmp >> 8)) >> 8;
							art_rgba_run_alpha (nos_pixels + 4 * x, r, g, b, a, 1);
						}
				}
			tos_pixels += rowstride;
			nos_pixels += rowstride;
		}
	
	g_object_unref (tos);
	ctx->pixbuf = nos;
}
