/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.

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

#include "config.h"
#include "rsvg.h"

#include <math.h>
#include <string.h>

#include <libart_lgpl/art_affine.h>
#include <libart_lgpl/art_vpath_bpath.h>
#include <libart_lgpl/art_svp_vpath_stroke.h>
#include <libart_lgpl/art_svp_vpath.h>
#include <libart_lgpl/art_svp_intersect.h>
#include <libart_lgpl/art_render_mask.h>
#include <libart_lgpl/art_render_svp.h>
#include <libart_lgpl/art_rgba.h>

#include <libxml/SAX.h>
#include <libxml/xmlmemory.h>

#include <pango/pangoft2.h>

#include "rsvg-bpath-util.h"
#include "rsvg-path.h"
#include "rsvg-css.h"
#include "rsvg-paint-server.h"

#define noVERBOSE

typedef struct _RsvgState RsvgState;
typedef struct _RsvgSaxHandler RsvgSaxHandler;


struct _RsvgState {
  double affine[6];

  gint opacity; /* 0..255 */

  RsvgPaintServer *fill;
  gint fill_opacity; /* 0..255 */

  RsvgPaintServer *stroke;
  gint stroke_opacity; /* 0..255 */
  double stroke_width;

  ArtPathStrokeCapType cap;
  ArtPathStrokeJoinType join;

  double font_size;
  char *font_family;

  guint32 stop_color; /* rgb */
  gint stop_opacity; /* 0..255 */

  gboolean in_defs;

  GdkPixbuf *save_pixbuf;
};

struct _RsvgSaxHandler {
  void (*free) (RsvgSaxHandler *self);
  void (*start_element) (RsvgSaxHandler *self, const xmlChar *name, const xmlChar **atts);
  void (*end_element) (RsvgSaxHandler *self, const xmlChar *name);
  void (*characters) (RsvgSaxHandler *self, const xmlChar *ch, int len);
};

struct RsvgHandle {
  RsvgSizeFunc size_func;
  gpointer user_data;
  GDestroyNotify user_data_destroy;
  GdkPixbuf *pixbuf;

  /* stack; there is a state for each element */
  RsvgState *state;
  int n_state;
  int n_state_max;

  RsvgDefs *defs;

  RsvgSaxHandler *handler; /* should this be a handler stack? */
  int handler_nest;

  GHashTable *entities; /* g_malloc'd string -> xmlEntityPtr */

  PangoContext *pango_context;
  xmlParserCtxtPtr ctxt;
  GError **error;
};

static void
rsvg_state_init (RsvgState *state)
{
  memset (state, 0, sizeof (*state));

  art_affine_identity (state->affine);

  state->opacity = 0xff;
  state->fill = rsvg_paint_server_parse (NULL, "#000");
  state->fill_opacity = 0xff;
  state->stroke_opacity = 0xff;
  state->stroke_width = 1;
  state->cap = ART_PATH_STROKE_CAP_BUTT;
  state->join = ART_PATH_STROKE_JOIN_MITER;
  state->stop_opacity = 0xff;
}

static void
rsvg_state_clone (RsvgState *dst, const RsvgState *src)
{
  *dst = *src;
  dst->font_family = g_strdup (src->font_family);
  rsvg_paint_server_ref (dst->fill);
  rsvg_paint_server_ref (dst->stroke);
  dst->save_pixbuf = NULL;
}

static void
rsvg_state_finalize (RsvgState *state)
{
  g_free (state->font_family);
  rsvg_paint_server_unref (state->fill);
  rsvg_paint_server_unref (state->stroke);
}

static void
rsvg_ctx_free_helper (gpointer key, gpointer value, gpointer user_data)
{
  xmlEntityPtr entval = (xmlEntityPtr)value;

  /* key == entval->name, so it's implicitly freed below */

  g_free ((xmlChar *)entval->name);
  g_free ((xmlChar *)entval->ExternalID);
  g_free ((xmlChar *)entval->SystemID);
  xmlFree (entval->content);
  xmlFree (entval->orig);
  g_free (entval);
}

static void
rsvg_pixmap_destroy (guchar *pixels, gpointer data)
{
  g_free (pixels);
}

static void
rsvg_start_svg (RsvgHandle *ctx, const xmlChar **atts)
{
  int i;
  int width = -1, height = -1;
  int rowstride;
  art_u8 *pixels;
  gint fixed;
  RsvgState *state;
  gboolean has_alpha = 1;
  gint new_width, new_height;
  gdouble x_zoom;
  gdouble y_zoom;

  if (atts != NULL)
    {
      for (i = 0; atts[i] != NULL; i += 2)
	{
	  if (!strcmp ((char *)atts[i], "width"))
	    width = rsvg_css_parse_length ((char *)atts[i + 1], &fixed);
	  else if (!strcmp ((char *)atts[i], "height"))
	    height = rsvg_css_parse_length ((char *)atts[i + 1], &fixed);
	}
#ifdef VERBOSE
      fprintf (stdout, "rsvg_start_svg: width = %d, height = %d\n",
	       width, height);
#endif

      new_width = width;
      new_height = height;
      if (ctx->size_func)
	(* ctx->size_func) (&new_width, &new_height, ctx->user_data);

      x_zoom = (width < 0 || new_width < 0) ? 1 : (double) new_width / width;
      y_zoom = (height < 0 || new_height < 0) ? 1 : (double) new_height / height;

      /* Scale size of target pixbuf */
      state = &ctx->state[ctx->n_state - 1];
      art_affine_scale (state->affine, x_zoom, y_zoom);

      if (new_width < 0 || new_height < 0)
        {
          g_warning ("rsvg_start_svg: no width and height attributes in SVG, nor supplied by size_func");
          if (new_width < 0) new_width = 500;
          if (new_height < 0) new_height = 500;
        }

      if (new_width >= INT_MAX / 4)
        {
          /* FIXME: What warning, GError here? */
	  return;
        }
      rowstride = (new_width * (has_alpha ? 4 : 3) + 3) & ~3;
      if (rowstride > INT_MAX / new_height)
        {
          /* FIXME: What warning, GError here? */
	  return;
        }

      /* FIXME: Add GError here if size is too big. */

      pixels = g_try_malloc (rowstride * new_height);
      if (pixels == NULL)
        {
          /* FIXME: What warning, GError here? */
	  return;
        }
      memset (pixels, has_alpha ? 0 : 255, rowstride * new_height);
      ctx->pixbuf = gdk_pixbuf_new_from_data (pixels,
					      GDK_COLORSPACE_RGB,
					      has_alpha, 8,
					      new_width, new_height,
					      rowstride,
					      rsvg_pixmap_destroy,
					      NULL);
    }
}

/* Parse a CSS2 style argument, setting the SVG context attributes. */
static void
rsvg_parse_style_arg (RsvgHandle *ctx, RsvgState *state, const char *str)
{
  int arg_off;

  arg_off = rsvg_css_param_arg_offset (str);
  if (rsvg_css_param_match (str, "opacity"))
    {
      state->opacity = rsvg_css_parse_opacity (str + arg_off);
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
      int fixed;
      state->stroke_width = rsvg_css_parse_length (str + arg_off, &fixed);
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
      state->font_size = rsvg_css_parse_fontsize (str + arg_off);
    }
  else if (rsvg_css_param_match (str, "font-family"))
    {
      g_free (state->font_family);
      state->font_family = g_strdup (str + arg_off);
    }
  else if (rsvg_css_param_match (str, "stop-color"))
    {
      state->stop_color = rsvg_css_parse_color (str + arg_off);
    }
  else if (rsvg_css_param_match (str, "stop-opacity"))
    {
      state->stop_opacity = rsvg_css_parse_opacity (str + arg_off);
    }
}

/* Split a CSS2 style into individual style arguments, setting attributes
   in the SVG context.

   It's known that this is _way_ out of spec. A more complete CSS2
   implementation will happen later.
*/
static void
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

/* Parse an SVG transform string into an affine matrix. Reference: SVG
   working draft dated 1999-07-06, section 8.5. Return TRUE on
   success. */
static gboolean
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
	      args[n_args] = strtod (src + idx, &end_ptr);
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

/**
 * rsvg_parse_style_attrs: Parse style attribute.
 * @ctx: Rsvg context.
 * @atts: Attributes in SAX style.
 *
 * Parses style and transform attributes and modifies state at top of
 * stack.
 **/
static void
rsvg_parse_style_attrs (RsvgHandle *ctx, const xmlChar **atts)
{
  int i;

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
	}
    }
}

/**
 * rsvg_push_opacity_group: Begin a new transparency group.
 * @ctx: Context in which to push.
 *
 * Pushes a new transparency group onto the stack. The top of the stack
 * is stored in the context, while the "saved" value is in the state
 * stack.
 **/
static void
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
static void
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

static void
rsvg_start_g (RsvgHandle *ctx, const xmlChar **atts)
{
  RsvgState *state = &ctx->state[ctx->n_state - 1];

  rsvg_parse_style_attrs (ctx, atts);

  if (state->opacity != 0xff)
    rsvg_push_opacity_group (ctx);
}

static void
rsvg_end_g (RsvgHandle *ctx)
{
  RsvgState *state = &ctx->state[ctx->n_state - 1];

  if (state->opacity != 0xff)
    rsvg_pop_opacity_group (ctx, state->opacity);
}

/**
 * rsvg_close_vpath: Close a vector path.
 * @src: Source vector path.
 *
 * Closes any open subpaths in the vector path.
 *
 * Return value: Closed vector path, allocated with g_new.
 **/
static ArtVpath *
rsvg_close_vpath (const ArtVpath *src)
{
  ArtVpath *result;
  int n_result, n_result_max;
  int src_ix;
  double beg_x, beg_y;
  gboolean open;

  n_result = 0;
  n_result_max = 16;
  result = g_new (ArtVpath, n_result_max);

  beg_x = 0;
  beg_y = 0;
  open = FALSE;

  for (src_ix = 0; src[src_ix].code != ART_END; src_ix++)
    {
      if (n_result == n_result_max)
	result = g_renew (ArtVpath, result, n_result_max <<= 1);
      result[n_result].code = src[src_ix].code == ART_MOVETO_OPEN ?
	ART_MOVETO : src[src_ix].code;
      result[n_result].x = src[src_ix].x;
      result[n_result].y = src[src_ix].y;
      n_result++;
      if (src[src_ix].code == ART_MOVETO_OPEN)
	{
	  beg_x = src[src_ix].x;
	  beg_y = src[src_ix].y;
	  open = TRUE;
	}
      else if (src[src_ix + 1].code != ART_LINETO)
	{
	  if (open && (beg_x != src[src_ix].x || beg_y != src[src_ix].y))
	    {
	      if (n_result == n_result_max)
		result = g_renew (ArtVpath, result, n_result_max <<= 1);
	      result[n_result].code = ART_LINETO;
	      result[n_result].x = beg_x;
	      result[n_result].y = beg_y;
	      n_result++;
	    }
	  open = FALSE;
	}
    }
  if (n_result == n_result_max)
    result = g_renew (ArtVpath, result, n_result_max <<= 1);
  result[n_result].code = ART_END;
  result[n_result].x = 0.0;
  result[n_result].y = 0.0;
  return result;
}

/**
 * rsvg_render_svp: Render an SVP.
 * @ctx: Context in which to render.
 * @svp: SVP to render.
 * @ps: Paint server for rendering.
 * @opacity: Opacity as 0..0xff.
 *
 * Renders the SVP over the pixbuf in @ctx.
 **/
static void
rsvg_render_svp (RsvgHandle *ctx, const ArtSVP *svp,
		 RsvgPaintServer *ps, int opacity)
{
  GdkPixbuf *pixbuf;
  ArtRender *render;
  gboolean has_alpha;

  pixbuf = ctx->pixbuf;
  if (pixbuf == NULL)
    {
      /* FIXME: What warning/GError here? */
      return;
    }

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

  art_render_svp (render, svp);
  art_render_mask_solid (render, (opacity << 8) + opacity + (opacity >> 7));
  rsvg_render_paint_server (render, ps, NULL); /* todo: paint server ctx */
  art_render_invoke (render);
}

static void
rsvg_render_bpath (RsvgHandle *ctx, const ArtBpath *bpath)
{
  RsvgState *state;
  ArtBpath *affine_bpath;
  ArtVpath *vpath;
  ArtSVP *svp;
  GdkPixbuf *pixbuf;
  gboolean need_tmpbuf;
  int opacity;
  int tmp;

  pixbuf = ctx->pixbuf;
  if (pixbuf == NULL)
    {
      /* FIXME: What warning/GError here? */
      return;
    }

  state = &ctx->state[ctx->n_state - 1];
  affine_bpath = art_bpath_affine_transform (bpath,
					     state->affine);

  vpath = art_bez_path_to_vec (affine_bpath, 0.25);
  art_free (affine_bpath);

  need_tmpbuf = (state->fill != NULL) && (state->stroke != NULL) &&
    state->opacity != 0xff;

  if (need_tmpbuf)
    rsvg_push_opacity_group (ctx);

  if (state->fill != NULL)
    {
      ArtVpath *closed_vpath;
      ArtSVP *svp2;
      ArtSvpWriter *swr;

      closed_vpath = rsvg_close_vpath (vpath);
      svp = art_svp_from_vpath (closed_vpath);
      g_free (closed_vpath);
      
      swr = art_svp_writer_rewind_new (ART_WIND_RULE_NONZERO);
      art_svp_intersector (svp, swr);

      svp2 = art_svp_writer_rewind_reap (swr);
      art_svp_free (svp);

      opacity = state->fill_opacity;
      if (!need_tmpbuf && state->opacity != 0xff)
	{
	  tmp = opacity * state->opacity + 0x80;
	  opacity = (tmp + (tmp >> 8)) >> 8;
	}
      rsvg_render_svp (ctx, svp2, state->fill, opacity);
      art_svp_free (svp2);
    }

  if (state->stroke != NULL)
    {
      /* todo: libart doesn't yet implement anamorphic scaling of strokes */
      double stroke_width = state->stroke_width *
	art_affine_expansion (state->affine);

      if (stroke_width < 0.25)
	stroke_width = 0.25;

      svp = art_svp_vpath_stroke (vpath, state->join, state->cap,
				  stroke_width, 4, 0.25);
      opacity = state->stroke_opacity;
      if (!need_tmpbuf && state->opacity != 0xff)
	{
	  tmp = opacity * state->opacity + 0x80;
	  opacity = (tmp + (tmp >> 8)) >> 8;
	}
      rsvg_render_svp (ctx, svp, state->stroke, opacity);
      art_svp_free (svp);
    }

  if (need_tmpbuf)
    rsvg_pop_opacity_group (ctx, state->opacity);

  art_free (vpath);
}

static void
rsvg_start_path (RsvgHandle *ctx, const xmlChar **atts)
{
  int i;
  char *d = NULL;

  rsvg_parse_style_attrs (ctx, atts);
  if (atts != NULL)
    {
      for (i = 0; atts[i] != NULL; i += 2)
	{
	  if (!strcmp ((char *)atts[i], "d"))
	    d = (char *)atts[i + 1];
	}
    }
  if (d != NULL)
    {
      RsvgBpathDef *bpath_def;

      bpath_def = rsvg_parse_path (d);
      rsvg_bpath_def_art_finish (bpath_def);

      rsvg_render_bpath (ctx, bpath_def->bpath);

      rsvg_bpath_def_free (bpath_def);
    }
}

/* begin text - this should likely get split into its own .c file */

typedef struct _RsvgSaxHandlerText RsvgSaxHandlerText;

struct _RsvgSaxHandlerText {
  RsvgSaxHandler super;
  RsvgHandle *ctx;
  double xpos;
  double ypos;
};

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
  char *string;
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
    {
      return;
    }

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

  string = g_malloc (end - beg + 1);
  memcpy (string, ch + beg, end - beg);
  string[end - beg] = 0;

#ifdef VERBOSE
  fprintf (stderr, "text characters(%s, %d)\n", string, len);
#endif

  if (ctx->pango_context == NULL)
    ctx->pango_context = pango_ft2_get_context (72, 72); /* FIXME: dpi? */

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
  pango_layout_set_text (layout, string, -1);
  font = pango_font_description_copy (pango_context_get_font_description (ctx->pango_context));
  if (state->font_family)
    pango_font_description_set_family_static (font, state->font_family);
  pango_font_description_set_size (font, state->font_size * PANGO_SCALE);
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
#ifdef VERBOSE
  fprintf (stderr, "opacity = %d\n", opacity);
#endif
  art_render_mask_solid (render, opacity);
  art_render_mask (render,
		   state->affine[4] + line_ink_rect.x,
		   state->affine[5] + line_ink_rect.y,
		   state->affine[4] + line_ink_rect.x + bitmap.width,
		   state->affine[5] + line_ink_rect.y + bitmap.rows,
		   bitmap.buffer, bitmap.pitch);
  art_render_invoke (render);
  g_free (bitmap.buffer);

  g_free (string);
}

static void
rsvg_start_text (RsvgHandle *ctx, const xmlChar **atts)
{
  RsvgSaxHandlerText *handler = g_new0 (RsvgSaxHandlerText, 1);

  handler->super.free = rsvg_text_handler_free;
  handler->super.characters = rsvg_text_handler_characters;
  handler->ctx = ctx;

  /* todo: parse "x" and "y" attributes */
  handler->xpos = 0;
  handler->ypos = 0;

  rsvg_parse_style_attrs (ctx, atts);
  ctx->handler = &handler->super;
#ifdef VERBOSE
  fprintf (stderr, "begin text!\n");
#endif
}

/* end text */

static void
rsvg_start_defs (RsvgHandle *ctx, const xmlChar **atts)
{
  RsvgState *state = &ctx->state[ctx->n_state - 1];

  state->in_defs = TRUE;
}

typedef struct _RsvgSaxHandlerGstops RsvgSaxHandlerGstops;

struct _RsvgSaxHandlerGstops {
  RsvgSaxHandler super;
  RsvgHandle *ctx;
  RsvgGradientStops *stops;
};

static void
rsvg_gradient_stop_handler_free (RsvgSaxHandler *self)
{
  g_free (self);
}

static void
rsvg_gradient_stop_handler_start (RsvgSaxHandler *self, const xmlChar *name,
				  const xmlChar **atts)
{
  RsvgSaxHandlerGstops *z = (RsvgSaxHandlerGstops *)self;
  RsvgGradientStops *stops = z->stops;
  int i;
  double offset = 0;
  gboolean got_offset = FALSE;
  gint fixed;
  RsvgState state;
  int n_stop;

  if (strcmp ((char *)name, "stop"))
    {
      g_warning ("unexpected <%s> element in gradient\n", name);
      return;
    }

  rsvg_state_init (&state);

  if (atts != NULL)
    {
      for (i = 0; atts[i] != NULL; i += 2)
	{
	  if (!strcmp ((char *)atts[i], "offset"))
	    {
	      offset = rsvg_css_parse_length ((char *)atts[i + 1], &fixed);
	      got_offset = TRUE;
	    }
	  else if (!strcmp ((char *)atts[i], "style"))
	    rsvg_parse_style (z->ctx, &state, (char *)atts[i + 1]);
	}
    }

  rsvg_state_finalize (&state);

  if (!got_offset)
    {
      g_warning ("gradient stop must specify offset\n");
      return;
    }

  n_stop = stops->n_stop++;
  if (n_stop == 0)
    stops->stop = g_new (RsvgGradientStop, 1);
  else if (!(n_stop & (n_stop - 1)))
    /* double the allocation if size is a power of two */
    stops->stop = g_renew (RsvgGradientStop, stops->stop, n_stop << 1);
  stops->stop[n_stop].offset = offset;
  stops->stop[n_stop].rgba = (state.stop_color << 8) | state.stop_opacity;
}

static void
rsvg_gradient_stop_handler_end (RsvgSaxHandler *self, const xmlChar *name)
{
}

static RsvgSaxHandler *
rsvg_gradient_stop_handler_new (RsvgHandle *ctx, RsvgGradientStops **p_stops)
{
  RsvgSaxHandlerGstops *gstops = g_new0 (RsvgSaxHandlerGstops, 1);
  RsvgGradientStops *stops = g_new (RsvgGradientStops, 1);

  gstops->super.free = rsvg_gradient_stop_handler_free;
  gstops->super.start_element = rsvg_gradient_stop_handler_start;
  gstops->super.end_element = rsvg_gradient_stop_handler_end;
  gstops->ctx = ctx;
  gstops->stops = stops;

  stops->n_stop = 0;
  stops->stop = NULL;

  *p_stops = stops;
  return &gstops->super;
}

static void
rsvg_linear_gradient_free (RsvgDefVal *self)
{
  RsvgLinearGradient *z = (RsvgLinearGradient *)self;

  g_free (z->stops->stop);
  g_free (z->stops);
  g_free (self);
}

static void
rsvg_start_linear_gradient (RsvgHandle *ctx, const xmlChar **atts)
{
  RsvgState *state = &ctx->state[ctx->n_state - 1];
  RsvgLinearGradient *grad;
  int i;
  char *id = NULL;
  double x1 = 0, y1 = 0, x2 = 100, y2 = 0;
  ArtGradientSpread spread = ART_GRADIENT_PAD;

  /* todo: only handles numeric coordinates in gradientUnits = userSpace */
  if (atts != NULL)
    {
      for (i = 0; atts[i] != NULL; i += 2)
	{
	  if (!strcmp ((char *)atts[i], "id"))
	    id = (char *)atts[i + 1];
	  else if (!strcmp ((char *)atts[i], "x1"))
	    x1 = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "y1"))
	    y1 = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "x2"))
	    x2 = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "y2"))
	    y2 = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "spreadMethod"))
	    {
	      if (!strcmp ((char *)atts[i + 1], "pad"))
		spread = ART_GRADIENT_PAD;
	      else if (!strcmp ((char *)atts[i + 1], "reflect"))
		spread = ART_GRADIENT_REFLECT;
	      else if (!strcmp ((char *)atts[i + 1], "repeat"))
		spread = ART_GRADIENT_REPEAT;
	    }
	}
    }

  grad = g_new (RsvgLinearGradient, 1);
  grad->super.type = RSVG_DEF_LINGRAD;
  grad->super.free = rsvg_linear_gradient_free;

  ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops);

  rsvg_defs_set (ctx->defs, id, &grad->super);

  for (i = 0; i < 6; i++)
    grad->affine[i] = state->affine[i];
  grad->x1 = x1;
  grad->y1 = y1;
  grad->x2 = x2;
  grad->y2 = y2;
  grad->spread = spread;
}

static void
rsvg_radial_gradient_free (RsvgDefVal *self)
{
  RsvgRadialGradient *z = (RsvgRadialGradient *)self;

  g_free (z->stops->stop);
  g_free (z->stops);
  g_free (self);
}

static void
rsvg_start_radial_gradient (RsvgHandle *ctx, const xmlChar **atts)
{
  RsvgState *state = &ctx->state[ctx->n_state - 1];
  RsvgRadialGradient *grad;
  int i;
  char *id = NULL;
  double cx = 50, cy = 50, r = 50, fx = 50, fy = 50;

  /* todo: only handles numeric coordinates in gradientUnits = userSpace */
  if (atts != NULL)
    {
      for (i = 0; atts[i] != NULL; i += 2)
	{
	  if (!strcmp ((char *)atts[i], "id"))
	    id = (char *)atts[i + 1];
	  else if (!strcmp ((char *)atts[i], "cx"))
	    cx = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "cy"))
	    cy = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "r"))
	    r = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "fx"))
	    fx = atof ((char *)atts[i + 1]);
	  else if (!strcmp ((char *)atts[i], "fy"))
	    fy = atof ((char *)atts[i + 1]);
	}
    }

  grad = g_new (RsvgRadialGradient, 1);
  grad->super.type = RSVG_DEF_RADGRAD;
  grad->super.free = rsvg_radial_gradient_free;

  ctx->handler = rsvg_gradient_stop_handler_new (ctx, &grad->stops);

  rsvg_defs_set (ctx->defs, id, &grad->super);

  for (i = 0; i < 6; i++)
    grad->affine[i] = state->affine[i];
  grad->cx = cx;
  grad->cy = cy;
  grad->r = r;
  grad->fx = fx;
  grad->fy = fy;
}

static void
rsvg_start_element (void *data, const xmlChar *name, const xmlChar **atts)
{
  RsvgHandle *ctx = (RsvgHandle *)data;
#ifdef VERBOSE
  int i;
#endif

#ifdef VERBOSE
  fprintf (stdout, "SAX.startElement(%s", (char *) name);
  if (atts != NULL) {
    for (i = 0;(atts[i] != NULL);i++) {
      fprintf (stdout, ", %s='", atts[i++]);
      fprintf (stdout, "%s'", atts[i]);
    }
  }
  fprintf (stdout, ")\n");
#endif

  if (ctx->handler)
    {
      ctx->handler_nest++;
      if (ctx->handler->start_element != NULL)
	ctx->handler->start_element (ctx->handler, name, atts);
    }
  else
    {
      /* push the state stack */
      if (ctx->n_state == ctx->n_state_max)
	ctx->state = g_renew (RsvgState, ctx->state, ctx->n_state_max <<= 1);
      if (ctx->n_state)
	rsvg_state_clone (&ctx->state[ctx->n_state],
			  &ctx->state[ctx->n_state - 1]);
      else
	rsvg_state_init (ctx->state);
      ctx->n_state++;

      if (!strcmp ((char *)name, "svg"))
	rsvg_start_svg (ctx, atts);
      else if (!strcmp ((char *)name, "g"))
	rsvg_start_g (ctx, atts);
      else if (!strcmp ((char *)name, "path"))
	rsvg_start_path (ctx, atts);
      else if (!strcmp ((char *)name, "text"))
	rsvg_start_text (ctx, atts);
      else if (!strcmp ((char *)name, "defs"))
	rsvg_start_defs (ctx, atts);
      else if (!strcmp ((char *)name, "linearGradient"))
	rsvg_start_linear_gradient (ctx, atts);
      else if (!strcmp ((char *)name, "radialGradient"))
	rsvg_start_radial_gradient (ctx, atts);
    }
}

static void
rsvg_end_element (void *data, const xmlChar *name)
{
  RsvgHandle *ctx = (RsvgHandle *)data;

  if (ctx->handler_nest > 0)
    {
      if (ctx->handler->end_element != NULL)
	ctx->handler->end_element (ctx->handler, name);
      ctx->handler_nest--;
    }
  else
    {
      if (ctx->handler != NULL)
	{
	  ctx->handler->free (ctx->handler);
	  ctx->handler = NULL;
	}

      if (!strcmp ((char *)name, "g"))
	rsvg_end_g (ctx);

      /* pop the state stack */
      ctx->n_state--;
      rsvg_state_finalize (&ctx->state[ctx->n_state]);

#ifdef VERBOSE
      fprintf (stdout, "SAX.endElement(%s)\n", (char *) name);
#endif
    }
}

static void
rsvg_characters (void *data, const xmlChar *ch, int len)
{
  RsvgHandle *ctx = (RsvgHandle *)data;

  if (ctx->handler && ctx->handler->characters != NULL)
    ctx->handler->characters (ctx->handler, ch, len);
}

static xmlEntityPtr
rsvg_get_entity (void *data, const xmlChar *name)
{
  RsvgHandle *ctx = (RsvgHandle *)data;

  return (xmlEntityPtr)g_hash_table_lookup (ctx->entities, name);
}

static void
rsvg_entity_decl (void *data, const xmlChar *name, int type,
		  const xmlChar *publicId, const xmlChar *systemId, xmlChar *content)
{
  RsvgHandle *ctx = (RsvgHandle *)data;
  GHashTable *entities = ctx->entities;
  xmlEntityPtr entity;
  char *dupname;

  entity = g_new0 (xmlEntity, 1);
  entity->type = type;
  entity->length = strlen (name);
  dupname = g_strdup (name);
  entity->name = dupname;
  entity->ExternalID = g_strdup (publicId);
  entity->SystemID = g_strdup (systemId);
  if (content)
    {
      entity->content = xmlMemStrdup (content);
      entity->length = strlen (content);
    }
  g_hash_table_insert (entities, dupname, entity);
}

static xmlSAXHandler rsvgSAXHandlerStruct = {
    NULL, /* internalSubset */
    NULL, /* isStandalone */
    NULL, /* hasInternalSubset */
    NULL, /* hasExternalSubset */
    NULL, /* resolveEntity */
    rsvg_get_entity, /* getEntity */
    rsvg_entity_decl, /* entityDecl */
    NULL, /* notationDecl */
    NULL, /* attributeDecl */
    NULL, /* elementDecl */
    NULL, /* unparsedEntityDecl */
    NULL, /* setDocumentLocator */
    NULL, /* startDocument */
    NULL, /* endDocument */
    rsvg_start_element, /* startElement */
    rsvg_end_element, /* endElement */
    NULL, /* reference */
    rsvg_characters, /* characters */
    NULL, /* ignorableWhitespace */
    NULL, /* processingInstruction */
    NULL, /* comment */
    NULL, /* xmlParserWarning */
    NULL, /* xmlParserError */
    NULL, /* xmlParserError */
    NULL, /* getParameterEntity */
};

static xmlSAXHandlerPtr rsvgSAXHandler = &rsvgSAXHandlerStruct;

GQuark
rsvg_error_quark (void)
{
  static GQuark q = 0;
  if (q == 0)
    q = g_quark_from_static_string ("rsvg-error-quark");

  return q;
}

/**
 * rsvg_handle_new:
 * @void:
 *
 * Returns a new rsvg handle.  Must be freed with @rsvg_handle_free.  This
 * handle can be used for dynamically loading an image.  You need to feed it
 * data using @rsvg_handle_write, then call @rsvg_handle_close when done.  No
 * more than one image can be loaded with one handle.
 *
 * Return value: A new #RsvgHandle
 **/
RsvgHandle *
rsvg_handle_new (void)
{
  RsvgHandle *handle;

  handle = g_new0 (RsvgHandle, 1);
  handle->n_state = 0;
  handle->n_state_max = 16;
  handle->state = g_new (RsvgState, handle->n_state_max);
  handle->defs = rsvg_defs_new ();
  handle->handler_nest = 0;
  handle->entities = g_hash_table_new (g_str_hash, g_str_equal);

  handle->ctxt = NULL;

  return handle;
}

/**
 * rsvg_handle_set_size_callback:
 * @handle: An #RsvgHandle
 * @size_func: A sizing function, or %NULL
 * @user_data: User data to pass to @size_func, or %NULL
 * @user_data_destroy: Destroy function for @user_data, or %NULL
 *
 * Sets the sizing function for the @handle.  This function is called right
 * after the size of the image has been loaded.  The size of the image is passed
 * in to the function, which may then modify these values to set the real size
 * of the generated pixbuf.  If the image has no associated size, then the size
 * arguments are set to -1.
 **/
void
rsvg_handle_set_size_callback (RsvgHandle     *handle,
			       RsvgSizeFunc    size_func,
			       gpointer        user_data,
			       GDestroyNotify  user_data_destroy)
{
  g_return_if_fail (handle != NULL);

  if (handle->user_data_destroy)
    (* handle->user_data_destroy) (handle->user_data);

  handle->size_func = size_func;
  handle->user_data = user_data;
  handle->user_data_destroy = user_data_destroy;
}

/**
 * rsvg_handle_write:
 * @handle: An #RsvgHandle
 * @buf: Pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: return location for errors
 *
 * Loads the next @count bytes of the image.  This will return #TRUE if the data
 * was loaded successful, and #FALSE if an error occurred.  In the latter case,
 * the loader will be closed, and will not accept further writes. If FALSE is
 * returned, @error will be set to an error from the #RSVG_ERROR domain.
 *
 * Return value: #TRUE if the write was successful, or #FALSE if there was an
 * error.
 **/
gboolean
rsvg_handle_write (RsvgHandle    *handle,
		   const guchar  *buf,
		   gsize          count,
		   GError       **error)
{
  GError *real_error;
  g_return_val_if_fail (handle != NULL, FALSE);

  handle->error = &real_error;
  if (handle->ctxt == NULL)
    {
      handle->ctxt = xmlCreatePushParserCtxt (rsvgSAXHandler,
					      handle,
					      buf, count,
					      "filename XXX");
      handle->ctxt->replaceEntities = TRUE;
    }
  else
    {
      xmlParseChunk (handle->ctxt, buf, count, 0);
    }
  handle->error = NULL;
  /* FIXME: Error handling not implemented. */
  /*  if (*real_error != NULL)
    {
      g_propagate_error (error, real_error);
      return FALSE;
      }*/
  return TRUE;
}

/**
 * rsvg_handle_close:
 * @handle: An #RsvgHandle
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return #TRUE if the loader closed successfully.  Note that @handle isn't
 * freed until @rsvg_handle_free is called.
 *
 * Return value: #TRUE if the loader closed successfully, or #FALSE if there was
 * an error.
 **/
gboolean
rsvg_handle_close (RsvgHandle  *handle,
		   GError     **error)
{
  gchar chars[1];
  GError *real_error;

  handle->error = &real_error;
  xmlParseChunk (handle->ctxt, chars, 0, 1);
  xmlFreeParserCtxt (handle->ctxt);
  /* FIXME: Error handling not implemented. */
  /*
  if (real_error != NULL)
    {
      g_propagate_error (error, real_error);
      return FALSE;
      }*/
  return TRUE;
}

/**
 * rsvg_handle_get_pixbuf:
 * @handle: An #RsvgHandle
 *
 * Returns the pixbuf loaded by #handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Return value: the pixbuf loaded by #handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle *handle)
{
  g_return_val_if_fail (handle != NULL, NULL);

  if (handle->pixbuf)
    return g_object_ref (handle->pixbuf);

  return NULL;
}

/**
 * rsvg_handle_free:
 * @handle: An #RsvgHandle
 *
 * Frees #handle.
 **/
void
rsvg_handle_free (RsvgHandle *handle)
{
  int i;

  if (handle->pango_context != NULL)
    g_object_unref (handle->pango_context);
  rsvg_defs_free (handle->defs);

  for (i = 0; i < handle->n_state; i++)
    rsvg_state_finalize (&handle->state[i]);
  g_free (handle->state);

  g_hash_table_foreach (handle->entities, rsvg_ctx_free_helper, NULL);
  g_hash_table_destroy (handle->entities);

  if (handle->user_data_destroy)
    (* handle->user_data_destroy) (handle->user_data);
  if (handle->pixbuf)
    g_object_unref (handle->pixbuf);
  g_free (handle);
}

struct RsvgSizeCallbackData
{
  gdouble x_zoom;
  gdouble y_zoom;
  gint width;
  gint height;
  gboolean zoom_set;
};

static void
rsvg_size_callback (gint     *width,
		    gint     *height,
		    gpointer  data)
{
  struct RsvgSizeCallbackData *real_data = (struct RsvgSizeCallbackData *)data;

  if (real_data->zoom_set)
    {
      (* width) = real_data->x_zoom * (* width);
      (* height) = real_data->y_zoom * (* height);
    }
  else
    {
      if (real_data->width != -1)
	*width = real_data->width;
      if (real_data->height != -1)
	*height = real_data->height;
    }
}

/**
 * rsvg_pixbuf_from_file:
 * @file_name: A file name
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  The caller must
 * assume the reference to the reurned pixbuf. If an error occurred, @error is
 * set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file (const gchar *file_name,
		       GError     **error)
{
  return rsvg_pixbuf_from_file_at_size (file_name, -1, -1, error);
}


/**
 * rsvg_pixbuf_from_file_at_zoom:
 * @file_name: A file name
 * @x_zoom: The horizontal zoom factor
 * @y_zoom: The vertical zoom factor
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated by the file by a factor of @x_zoom and @y_zoom.  The
 * caller must assume the reference to the reurned pixbuf. If an error
 * occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_zoom (const gchar *file_name,
			       double       x_zoom,
			       double       y_zoom,
			       GError     **error)
{
  FILE *f;
  char chars[10];
  gint result;
  GdkPixbuf *retval;
  RsvgHandle *handle;
  struct RsvgSizeCallbackData data;

  g_return_val_if_fail (file_name != NULL, NULL);
  g_return_val_if_fail (x_zoom > 0.0 && y_zoom > 0.0, NULL);

  f = fopen (file_name, "r");
  if (!f)
    {
      /* FIXME: Set up error. */
      return NULL;
    }

  handle = rsvg_handle_new ();
  data.zoom_set = TRUE;
  data.x_zoom = x_zoom;
  data.y_zoom = y_zoom;

  rsvg_handle_set_size_callback (handle, rsvg_size_callback, &data, NULL);
  while ((result = fread (chars, 1, 3, f)) > 0)
    rsvg_handle_write (handle, chars, result, error);
  rsvg_handle_close (handle, error);
  
  retval = rsvg_handle_get_pixbuf (handle);

  fclose (f);
  rsvg_handle_free (handle);
  return retval;
}

/**
 * rsvg_pixbuf_from_file_at_size:
 * @file_name: A file name
 * @width: The new width, or -1
 * @height: The new height, or -1
 * @error: return location for errors
 * 
 * Loads a new #GdkPixbuf from @file_name and returns it.  This pixbuf is scaled
 * from the size indicated to the new size indicated by @width and @height.  If
 * either of these are -1, then the default size of the image being loaded is
 * used.  The caller must assume the reference to the reurned pixbuf. If an
 * error occurred, @error is set and %NULL is returned.
 * 
 * Return value: A newly allocated #GdkPixbuf, or %NULL
 **/
GdkPixbuf *
rsvg_pixbuf_from_file_at_size (const gchar *file_name,
			       gint         width,
			       gint         height,
			       GError     **error)
{
  FILE *f;
  char chars[10];
  gint result;
  GdkPixbuf *retval;
  RsvgHandle *handle;
  struct RsvgSizeCallbackData data;

  f = fopen (file_name, "r");
  if (!f)
    {
      /* FIXME: Set up error. */
      return NULL;
    }
  handle = rsvg_handle_new ();
  data.zoom_set = FALSE;
  data.width = width;
  data.height = height;

  rsvg_handle_set_size_callback (handle, rsvg_size_callback, &data, NULL);
  while ((result = fread (chars, 1, 3, f)) > 0)
    rsvg_handle_write (handle, chars, result, error);

  rsvg_handle_close (handle, error);
  
  retval = rsvg_handle_get_pixbuf (handle);

  fclose (f);
  rsvg_handle_free (handle);
  return retval;
}

