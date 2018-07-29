/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

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
#define _GNU_SOURCE 1

#include "rsvg-private.h"
#include "rsvg-css.h"
#include "rsvg-styles.h"
#include "rsvg-io.h"
#include "rsvg-load.h"

#include <gio/gio.h>

#include <math.h>
#include <string.h>
#include <stdarg.h>
#include <limits.h>
#include <stdlib.h>

#ifdef G_OS_WIN32
static char *
rsvg_realpath_utf8 (const char *filename, const char *unused)
{
    wchar_t *wfilename;
    wchar_t *wfull;
    char *full;

    wfilename = g_utf8_to_utf16 (filename, -1, NULL, NULL, NULL);
    if (!wfilename)
        return NULL;

    wfull = _wfullpath (NULL, wfilename, 0);
    g_free (wfilename);
    if (!wfull)
        return NULL;

    full = g_utf16_to_utf8 (wfull, -1, NULL, NULL, NULL);
    free (wfull);

    if (!full)
        return NULL;

    return full;
}

#define realpath(a,b) rsvg_realpath_utf8 (a, b)
#endif

/*
 * This is configurable at runtime
 */
#define RSVG_DEFAULT_DPI_X 90.0
#define RSVG_DEFAULT_DPI_Y 90.0

G_GNUC_INTERNAL
double rsvg_internal_dpi_x = RSVG_DEFAULT_DPI_X;
G_GNUC_INTERNAL
double rsvg_internal_dpi_y = RSVG_DEFAULT_DPI_Y;

/**
 * rsvg_error_quark:
 *
 * The error domain for RSVG
 *
 * Returns: The error domain
 */
GQuark
rsvg_error_quark (void)
{
    /* don't use from_static_string(), since librsvg might be used in a module
       that's ultimately unloaded */
    return g_quark_from_string ("rsvg-error-quark");
}

/**
 * rsvg_set_default_dpi:
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
void
rsvg_set_default_dpi (double dpi)
{
    rsvg_set_default_dpi_x_y (dpi, dpi);
}

/**
 * rsvg_set_default_dpi_x_y:
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi_x_y() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
void
rsvg_set_default_dpi_x_y (double dpi_x, double dpi_y)
{
    if (dpi_x <= 0.)
        rsvg_internal_dpi_x = RSVG_DEFAULT_DPI_X;
    else
        rsvg_internal_dpi_x = dpi_x;

    if (dpi_y <= 0.)
        rsvg_internal_dpi_y = RSVG_DEFAULT_DPI_Y;
    else
        rsvg_internal_dpi_y = dpi_y;
}

/**
 * rsvg_handle_write:
 * @handle: an #RsvgHandle
 * @buf: (array length=count) (element-type guchar): pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Loads the next @count bytes of the image.  This will return %TRUE if the data
 * was loaded successful, and %FALSE if an error occurred.  In the latter case,
 * the loader will be closed, and will not accept further writes. If %FALSE is
 * returned, @error will be set to an error from the #RsvgError domain. Errors
 * from #GIOErrorEnum are also possible.
 *
 * Returns: %TRUE on success, or %FALSE on error
 **/
gboolean
rsvg_handle_write (RsvgHandle *handle, const guchar *buf, gsize count, GError **error)
{
    RsvgHandlePrivate *priv;

    rsvg_return_val_if_fail (handle, FALSE, error);
    priv = handle->priv;

    rsvg_return_val_if_fail (priv->hstate == RSVG_HANDLE_STATE_START
                             || priv->hstate == RSVG_HANDLE_STATE_LOADING,
                             FALSE,
                             error);

    if (priv->hstate == RSVG_HANDLE_STATE_START) {
        priv->hstate = RSVG_HANDLE_STATE_LOADING;
        priv->load = rsvg_load_new (handle, (priv->flags & RSVG_HANDLE_FLAG_UNLIMITED) != 0);
    }

    g_assert (priv->hstate == RSVG_HANDLE_STATE_LOADING);

    return rsvg_load_write (priv->load, buf, count, error);
}

static gboolean
finish_load (RsvgHandle *handle, gboolean was_successful)
{
    RsvgNode *treebase = rsvg_load_destroy (handle->priv->load);
    handle->priv->load = NULL;

    if (was_successful) {
        handle->priv->hstate = RSVG_HANDLE_STATE_CLOSED_OK;
        handle->priv->treebase = treebase;
    } else {
        handle->priv->hstate = RSVG_HANDLE_STATE_CLOSED_ERROR;
        treebase = rsvg_node_unref (treebase);
    }

    return was_successful;
}

/**
 * rsvg_handle_close:
 * @handle: a #RsvgHandle
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return %TRUE if the loader closed successfully.  Note that @handle isn't
 * freed until @g_object_unref is called.
 *
 * Returns: %TRUE on success, or %FALSE on error
 **/
gboolean
rsvg_handle_close (RsvgHandle *handle, GError **error)
{
    RsvgHandlePrivate *priv;
    gboolean result;

    rsvg_return_val_if_fail (handle, FALSE, error);
    priv = handle->priv;

    if (priv->hstate == RSVG_HANDLE_STATE_CLOSED_OK
        || priv->hstate == RSVG_HANDLE_STATE_CLOSED_ERROR) {
        /* closing is idempotent */
        return TRUE;
    }

    result = finish_load (handle, rsvg_load_close (priv->load, error));

    return result;
}

/**
 * rsvg_handle_read_stream_sync:
 * @handle: a #RsvgHandle
 * @stream: a #GInputStream
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Reads @stream and writes the data from it to @handle.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: %TRUE if reading @stream succeeded, or %FALSE otherwise
 *   with @error filled in
 *
 * Since: 2.32
 */
gboolean
rsvg_handle_read_stream_sync (RsvgHandle   *handle,
                              GInputStream *stream,
                              GCancellable *cancellable,
                              GError      **error)
{
    RsvgHandlePrivate *priv;
    gboolean result;
    RsvgLoad *saved_load;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), FALSE);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), FALSE);
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);

    priv = handle->priv;

    g_return_val_if_fail (priv->hstate == RSVG_HANDLE_STATE_START, FALSE);

    priv->hstate = RSVG_HANDLE_STATE_LOADING;

    saved_load = priv->load;

    priv->load = rsvg_load_new (handle, (priv->flags & RSVG_HANDLE_FLAG_UNLIMITED) != 0);
    result = finish_load (handle, rsvg_load_read_stream_sync (priv->load, stream, cancellable, error));

    priv->load = saved_load;

    return result;
}

/**
 * rsvg_init:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to initialize librsvg.
 **/
void
rsvg_init (void)
{
}

/**
 * rsvg_term:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to de-initialize librsvg.
 **/
void
rsvg_term (void)
{
}

/**
 * rsvg_cleanup:
 *
 * This function should not be called from normal programs.
 * See xmlCleanupParser() for more information.
 *
 * Since: 2.36
 **/
void
rsvg_cleanup (void)
{
    xmlCleanupParser ();
}

cairo_surface_t *
rsvg_cairo_surface_new_from_href (RsvgHandle *handle,
                                  const char *href,
                                  GError **error)
{
    char *data;
    gsize data_len;
    char *mime_type = NULL;
    GdkPixbufLoader *loader = NULL;
    GdkPixbuf *pixbuf = NULL;
    cairo_surface_t *surface = NULL;

    data = _rsvg_handle_acquire_data (handle, href, &mime_type, &data_len, error);
    if (data == NULL)
        return NULL;

    if (mime_type) {
        loader = gdk_pixbuf_loader_new_with_mime_type (mime_type, error);
    } else {
        loader = gdk_pixbuf_loader_new ();
    }

    if (loader == NULL)
        goto out;

    if (!gdk_pixbuf_loader_write (loader, (guchar *) data, data_len, error)) {
        gdk_pixbuf_loader_close (loader, NULL);
        goto out;
    }

    if (!gdk_pixbuf_loader_close (loader, error))
        goto out;

    pixbuf = gdk_pixbuf_loader_get_pixbuf (loader);

    if (!pixbuf) {
        g_set_error (error,
                     GDK_PIXBUF_ERROR,
                     GDK_PIXBUF_ERROR_FAILED,
                      _("Failed to load image '%s': reason not known, probably a corrupt image file"),
                      href);
        goto out;
    }

    surface = rsvg_cairo_surface_from_pixbuf (pixbuf);

    if (mime_type == NULL) {
        /* Try to get the information from the loader */
        GdkPixbufFormat *format;
        char **mime_types;

        if ((format = gdk_pixbuf_loader_get_format (loader)) != NULL) {
            mime_types = gdk_pixbuf_format_get_mime_types (format);

            if (mime_types != NULL)
                mime_type = g_strdup (mime_types[0]);
            g_strfreev (mime_types);
        }
    }

    if ((handle->priv->flags & RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA) != 0 &&
        mime_type != NULL &&
        cairo_surface_set_mime_data (surface, mime_type, (guchar *) data,
                                     data_len, g_free, data) == CAIRO_STATUS_SUCCESS) {
        data = NULL; /* transferred to the surface */
    }

  out:
    if (loader)
        g_object_unref (loader);
    g_free (mime_type);
    g_free (data);

    return surface;
}

void
rsvg_return_if_fail_warning (const char *pretty_function, const char *expression, GError ** error)
{
    g_set_error (error, RSVG_ERROR, 0, _("%s: assertion `%s' failed"), pretty_function, expression);
}

#ifdef HAVE_PANGOFT2

static void
create_font_config_for_testing (RsvgHandle *handle)
{
    const char *font_paths[] = {
        "resources/Roboto-Regular.ttf",
        "resources/Roboto-Italic.ttf",
        "resources/Roboto-Bold.ttf",
        "resources/Roboto-BoldItalic.ttf",
    };

    int i;

    if (handle->priv->font_config_for_testing != NULL)
        return;

    handle->priv->font_config_for_testing = FcConfigCreate ();

    for (i = 0; i < G_N_ELEMENTS(font_paths); i++) {
        char *font_path = g_test_build_filename (G_TEST_DIST, font_paths[i], NULL);

        if (!FcConfigAppFontAddFile (handle->priv->font_config_for_testing, (const FcChar8 *) font_path)) {
            g_error ("Could not load font file \"%s\" for tests; aborting", font_path);
        }

        g_free (font_path);
    }
}

#endif

void
rsvg_handle_update_font_map_for_testing (RsvgHandle *handle)
{
#ifdef HAVE_PANGOFT2
    if (handle->priv->is_testing) {
        create_font_config_for_testing (handle);

        if (handle->priv->font_map_for_testing == NULL) {
            handle->priv->font_map_for_testing = pango_cairo_font_map_new_for_font_type (CAIRO_FONT_TYPE_FT);
            pango_fc_font_map_set_config (PANGO_FC_FONT_MAP (handle->priv->font_map_for_testing),
                                          handle->priv->font_config_for_testing);

            pango_cairo_font_map_set_default (PANGO_CAIRO_FONT_MAP (handle->priv->font_map_for_testing));
        }
    }
#endif
}

gboolean
rsvg_allow_load (GFile       *base_gfile,
                 const char  *uri,
                 GError     **error)
{
    GFile *base;
    char *path, *dir;
    char *scheme = NULL, *cpath = NULL, *cdir = NULL;

    g_assert (error == NULL || *error == NULL);

    scheme = g_uri_parse_scheme (uri);

    /* Not a valid URI */
    if (scheme == NULL)
        goto deny;

    /* Allow loads of data: from any location */
    if (g_str_equal (scheme, "data"))
        goto allow;

    /* No base to compare to? */
    if (base_gfile == NULL)
        goto deny;

    /* Deny loads from differing URI schemes */
    if (!g_file_has_uri_scheme (base_gfile, scheme))
        goto deny;

    /* resource: is allowed to load anything from other resources */
    if (g_str_equal (scheme, "resource"))
        goto allow;

    /* Non-file: isn't allowed to load anything */
    if (!g_str_equal (scheme, "file"))
        goto deny;

    base = g_file_get_parent (base_gfile);
    if (base == NULL)
        goto deny;

    dir = g_file_get_path (base);
    g_object_unref (base);

    cdir = realpath (dir, NULL);
    g_free (dir);
    if (cdir == NULL)
        goto deny;

    path = g_filename_from_uri (uri, NULL, NULL);
    if (path == NULL)
        goto deny;

    cpath = realpath (path, NULL);
    g_free (path);

    if (cpath == NULL)
        goto deny;

    /* Now check that @cpath is below @cdir */
    if (!g_str_has_prefix (cpath, cdir) ||
        cpath[strlen (cdir)] != G_DIR_SEPARATOR)
        goto deny;

    /* Allow load! */

 allow:
    g_free (scheme);
    free (cpath);
    free (cdir);
    return TRUE;

 deny:
    g_free (scheme);
    free (cpath);
    free (cdir);

    g_set_error (error, G_IO_ERROR, G_IO_ERROR_PERMISSION_DENIED,
                 "File may not link to URI \"%s\"", uri);
    return FALSE;
}

char *
rsvg_handle_resolve_uri (RsvgHandle *handle,
                         const char *uri)
{
    RsvgHandlePrivate *priv = handle->priv;
    char *scheme, *resolved_uri;
    GFile *base, *resolved;

    if (uri == NULL)
        return NULL;

    scheme = g_uri_parse_scheme (uri);
    if (scheme != NULL ||
        priv->base_gfile == NULL ||
        (base = g_file_get_parent (priv->base_gfile)) == NULL) {
        g_free (scheme);
        return g_strdup (uri);
    }

    resolved = g_file_resolve_relative_path (base, uri);
    resolved_uri = g_file_get_uri (resolved);

    g_free (scheme);
    g_object_unref (base);
    g_object_unref (resolved);

    return resolved_uri;
}

char *
_rsvg_handle_acquire_data (RsvgHandle *handle,
                           const char *url,
                           char **content_type,
                           gsize *len,
                           GError **error)
{
    RsvgHandlePrivate *priv = handle->priv;
    char *uri;
    char *data;

    uri = rsvg_handle_resolve_uri (handle, url);

    if (rsvg_allow_load (priv->base_gfile, uri, error)) {
        data = _rsvg_io_acquire_data (uri,
                                      rsvg_handle_get_base_uri (handle),
                                      content_type,
                                      len,
                                      handle->priv->cancellable,
                                      error);
    } else {
        data = NULL;
    }

    g_free (uri);
    return data;
}

GInputStream *
_rsvg_handle_acquire_stream (RsvgHandle *handle,
                             const char *url,
                             char **content_type,
                             GError **error)
{
    RsvgHandlePrivate *priv = handle->priv;
    char *uri;
    GInputStream *stream;

    uri = rsvg_handle_resolve_uri (handle, url);

    if (rsvg_allow_load (priv->base_gfile, uri, error)) {
        stream = _rsvg_io_acquire_stream (uri,
                                          rsvg_handle_get_base_uri (handle),
                                          content_type,
                                          handle->priv->cancellable,
                                          error);
    } else {
        stream = NULL;
    }

    g_free (uri);
    return stream;
}
