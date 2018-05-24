/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
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
#include "config.h"

#include <errno.h>
#include <string.h>
#include <math.h>

#include "rsvg-attributes.h"
#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"

#include <libcroco/libcroco.h>

typedef gboolean (*InheritanceFunction) (gboolean dst_has_prop, gboolean src_has_prop);

/* Defined in rsvg_internals/src/state.rs */
extern gboolean rsvg_state_parse_style_pair(RsvgState *state, RsvgAttribute attr, const char *value, gboolean important, gboolean accept_shorthands) G_GNUC_WARN_UNUSED_RESULT;

typedef struct _StyleValueData {
    gchar *value;
    gboolean important;
} StyleValueData;

static StyleValueData *
style_value_data_new (const gchar *value, gboolean important)
{
    StyleValueData *ret;

    ret = g_new0 (StyleValueData, 1);
    ret->value = g_strdup (value);
    ret->important = important;

    return ret;
}

static void
style_value_data_free (StyleValueData *value)
{
    if (!value)
        return;
    g_free (value->value);
    g_free (value);
}

static void
rsvg_css_define_style (RsvgHandle *handle,
                       const gchar *selector,
                       const gchar *style_name,
                       const gchar *style_value,
                       gboolean important)
{
    GHashTable *styles;
    gboolean need_insert = FALSE;

    /* push name/style pair into HT */
    styles = g_hash_table_lookup (handle->priv->css_props, selector);
    if (styles == NULL) {
        styles = g_hash_table_new_full (g_str_hash, g_str_equal,
                                        g_free, (GDestroyNotify) style_value_data_free);
        g_hash_table_insert (handle->priv->css_props, (gpointer) g_strdup (selector), styles);
        need_insert = TRUE;
    } else {
        StyleValueData *current_value;
        current_value = g_hash_table_lookup (styles, style_name);
        if (current_value == NULL || !current_value->important)
            need_insert = TRUE;
    }
    if (need_insert) {
        g_hash_table_insert (styles,
                             (gpointer) g_strdup (style_name),
                             (gpointer) style_value_data_new (style_value, important));
    }
}

typedef struct _CSSUserData {
    RsvgHandle *handle;
    CRSelector *selector;
} CSSUserData;

static void
css_user_data_init (CSSUserData *user_data, RsvgHandle *handle)
{
    user_data->handle = handle;
    user_data->selector = NULL;
}

static void
ccss_start_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;
    cr_selector_ref (a_selector_list);
    user_data->selector = a_selector_list;
}

static void
ccss_end_selector (CRDocHandler * a_handler, CRSelector * a_selector_list)
{
    CSSUserData *user_data;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    cr_selector_unref (user_data->selector);
    user_data->selector = NULL;
}

static void
ccss_property (CRDocHandler * a_handler, CRString * a_name, CRTerm * a_expr, gboolean a_important)
{
    CSSUserData *user_data;
    gchar *name = NULL;
    size_t len = 0;

    g_return_if_fail (a_handler);

    user_data = (CSSUserData *) a_handler->app_data;

    if (a_name && a_expr && user_data->selector) {
        CRSelector *cur;
        for (cur = user_data->selector; cur; cur = cur->next) {
            if (cur->simple_sel) {
                gchar *selector = (gchar *) cr_simple_sel_to_string (cur->simple_sel);
                if (selector) {
                    gchar *style_name, *style_value;
                    name = (gchar *) cr_string_peek_raw_str (a_name);
                    len = cr_string_peek_raw_str_len (a_name);
                    style_name = g_strndup (name, len);
                    style_value = (gchar *)cr_term_to_string (a_expr);
                    rsvg_css_define_style (user_data->handle,
                                           selector,
                                           style_name,
                                           style_value,
                                           a_important);
                    g_free (selector);
                    g_free (style_name);
                    g_free (style_value);
                }
            }
        }
    }
}

static void
ccss_error (CRDocHandler * a_handler)
{
    /* yup, like i care about CSS parsing errors ;-)
       ignore, chug along */
    g_message (_("CSS parsing error\n"));
}

static void
ccss_unrecoverable_error (CRDocHandler * a_handler)
{
    /* yup, like i care about CSS parsing errors ;-)
       ignore, chug along */
    g_message (_("CSS unrecoverable error\n"));
}

static void
 ccss_import_style (CRDocHandler * a_this,
                    GList * a_media_list,
                    CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location);

static void
init_sac_handler (CRDocHandler * a_handler)
{
    a_handler->start_document = NULL;
    a_handler->end_document = NULL;
    a_handler->import_style = ccss_import_style;
    a_handler->namespace_declaration = NULL;
    a_handler->comment = NULL;
    a_handler->start_selector = ccss_start_selector;
    a_handler->end_selector = ccss_end_selector;
    a_handler->property = ccss_property;
    a_handler->start_font_face = NULL;
    a_handler->end_font_face = NULL;
    a_handler->start_media = NULL;
    a_handler->end_media = NULL;
    a_handler->start_page = NULL;
    a_handler->end_page = NULL;
    a_handler->ignorable_at_rule = NULL;
    a_handler->error = ccss_error;
    a_handler->unrecoverable_error = ccss_unrecoverable_error;
}

void
rsvg_parse_cssbuffer (RsvgHandle *handle, const char *buff, size_t buflen)
{
    CRParser *parser = NULL;
    CRDocHandler *css_handler = NULL;
    CSSUserData user_data;

    if (buff == NULL || buflen == 0)
        return;

    css_handler = cr_doc_handler_new ();
    init_sac_handler (css_handler);

    css_user_data_init (&user_data, handle);
    css_handler->app_data = &user_data;

    /* TODO: fix libcroco to take in const strings */
    parser = cr_parser_new_from_buf ((guchar *) buff, (gulong) buflen, CR_UTF_8, FALSE);
    if (parser == NULL) {
        cr_doc_handler_unref (css_handler);
        return;
    }

    cr_parser_set_sac_handler (parser, css_handler);
    cr_doc_handler_unref (css_handler);

    cr_parser_set_use_core_grammar (parser, FALSE);
    cr_parser_parse (parser);

    /* FIXME: we aren't reporting errors in the CSS; we have no way to know if
     * we should print the "buff" for diagnostics.
     */

    cr_parser_destroy (parser);
}

static void
ccss_import_style (CRDocHandler * a_this,
                   GList * a_media_list,
                   CRString * a_uri, CRString * a_uri_default_ns, CRParsingLocation * a_location)
{
    CSSUserData *user_data = (CSSUserData *) a_this->app_data;
    char *stylesheet_data;
    gsize stylesheet_data_len;
    char *mime_type = NULL;

    if (a_uri == NULL)
        return;

    stylesheet_data = _rsvg_handle_acquire_data (user_data->handle,
                                                 cr_string_peek_raw_str (a_uri),
                                                 &mime_type,
                                                 &stylesheet_data_len,
                                                 NULL);
    if (stylesheet_data == NULL || 
        mime_type == NULL || 
        strcmp (mime_type, "text/css") != 0) {
        g_free (stylesheet_data);
        g_free (mime_type);
        return;
    }

    rsvg_parse_cssbuffer (user_data->handle, stylesheet_data, stylesheet_data_len);
    g_free (stylesheet_data);
    g_free (mime_type);
}

static void
apply_style (const gchar *key, StyleValueData *value, gpointer user_data)
{
    RsvgState *state = user_data;
    RsvgAttribute attr;

    if (rsvg_attribute_from_name (key, &attr)) {
        /* FIXME: this is ignoring errors */
        gboolean success = rsvg_state_parse_style_pair (state,
                                                        attr,
                                                        value->value,
                                                        value->important,
                                                        TRUE);
    }
}

gboolean
rsvg_lookup_apply_css_style (RsvgHandle *handle, const char *target, RsvgState * state)
{
    GHashTable *styles;

    styles = g_hash_table_lookup (handle->priv->css_props, target);

    if (styles != NULL) {
        g_hash_table_foreach (styles, (GHFunc) apply_style, state);
        return TRUE;
    }
    return FALSE;
}

/* This is defined like this so that we can export the Rust function... just for
 * the benefit of rsvg-convert.c
 */
RsvgCssColorSpec
rsvg_css_parse_color_ (const char *str)
{
    return rsvg_css_parse_color (str);
}
