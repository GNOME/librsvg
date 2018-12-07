/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-gobject.c: GObject support.

   Copyright (C) 2006 Robert Staudinger <robert.staudinger@gmail.com>

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
*/

/**
 * SECTION: rsvg-handle
 * @short_description: Loads SVG data into memory.
 *
 * This is the main entry point into the librsvg library.  An RsvgHandle is an
 * object that represents SVG data in memory.  Your program creates an
 * RsvgHandle from an SVG file, or from a memory buffer that contains SVG data,
 * or in the most general form, from a #GInputStream that will provide SVG data.
 *
 * Librsvg supports reading <link
 * xlink:href="https://www.w3.org/TR/SVG/">SVG 1.1</link> data.  It also
 * supports SVGZ files, which is just an SVG stream compressed with the GZIP
 * algorithm.
 *
 * # The "base file" and resolving references to external files
 *
 * When you load an SVG, librsvg needs to know the location of the "base file"
 * for it.  This is so that librsvg can determine the location of referenced
 * entities.  For example, say you have an SVG in <filename>/foo/bar/foo.svg</filename>
 * and that it has an image element like this:
 *
 * |[
 * <image xlink:href="resources/foo.png" .../>
 * ]|
 *
 * In this case, librsvg needs to know the location of the toplevel
 * <filename>/foo/bar/foo.svg</filename> so that it can generate the appropriate
 * reference to <filename>/foo/bar/resources/foo.png</filename>.
 *
 * ## Security and locations of referenced files
 *
 * When processing an SVG, librsvg will only load referenced files if they are
 * in the same directory as the base file, or in a subdirectory of it.  That is,
 * if the base file is <filename>/foo/bar/baz.svg</filename>, then librsvg will
 * only try to load referenced files (from SVG's "image" element, for example,
 * or from content included through XML entities) if those files are in
 * <filename>/foo/bar/<!-- -->*</filename> or in
 * <filename>/foo/bar/<!-- -->*<!-- -->/.../<!-- -->*</filename>.  This is so that malicious
 * SVG files cannot include files that are in a directory above.
 *
 * # Loading an SVG with GIO
 *
 * If you have a #GFile that stands for an SVG file, you can simply call
 * rsvg_handle_new_from_gfile_sync() to load an RsvgHandle from it.
 *
 * Alternatively, if you have a #GInputStream, you can use
 * rsvg_handle_new_from_stream_sync().
 *
 * Both of those methods allow specifying a #GCancellable, so the loading
 * process can be cancelled from another thread.
 *
 * ## Loading an SVG from memory
 *
 * If you already have SVG data in memory, you can create a memory input stream
 * with g_memory_input_stream_new_from_data() and feed that to
 * rsvg_handle_new_from_stream_sync().  This lets you specify the appropriate
 * flags, for example #RSVG_HANDLE_FLAG_UNLIMITED if your input data is very
 * large.
 *
 * Note that in this case, it is important that you specify the base_file for
 * the in-memory SVG data.  Librsvg uses the base_file to resolve links to
 * external content, like raster images.
 *
 * # Loading an SVG without GIO
 *
 * You can load an RsvgHandle from a simple filename or URI with
 * rsvg_handle_new_from_file().  Note that this is a blocking operation; there
 * is no way to cancel it if loading a remote URI takes a long time.
 *
 * Alternatively, you can create an empty RsvgHandle with rsvg_handle_new() or
 * rsvg_handle_new_with_flags().  The first function is equivalent to using
 * #RSVG_HANDLE_FLAGS_NONE on the second one.  These functions give you back an
 * empty RsvgHandle, which is ready for you to feed it SVG data.  You can do
 * this with rsvg_handle_write() and rsvg_handle_close().
 *
 * # Resolution of the rendered image (dots per inch, or DPI)
 *
 * SVG images can contain dimensions like "<literal>5 cm</literal>" or
 * "<literal>2 pt</literal>" that must be converted from physical units into
 * device units.  To do this, librsvg needs to know the actual dots per inch
 * (DPI) of your target device.  You can call rsvg_handle_set_dpi() or
 * rsvg_handle_set_dpi_x_y() on an RsvgHandle to set the DPI before rendering
 * it.
 *
 * # Rendering
 *
 * The preferred way to render an already-loaded RsvgHandle is to use
 * rsvg_handle_render_cairo().  Please see its documentation for details.
 *
 * Alternatively, you can use rsvg_handle_get_pixbuf() to directly obtain a
 * #GdkPixbuf with the rendered image.  This is simple, but it does not let you
 * control the size at which the SVG will be rendered.  It will just be rendered
 * at the size which rsvg_handle_get_dimensions() would return, which depends on
 * the dimensions that librsvg is able to compute from the SVG data.
 */

#include "config.h"
#define _GNU_SOURCE 1

#include <string.h>
#include <limits.h>
#include <stdlib.h>

#include "rsvg-load.h"
#include "rsvg-private.h"

/* Implemented in rsvg_internals/src/handle.rs */
extern RsvgHandleRust *rsvg_handle_rust_new (void);
extern void rsvg_handle_rust_free (RsvgHandleRust *raw_handle);
extern void rsvg_handle_rust_cascade (RsvgHandleRust *raw_handle);
extern double rsvg_handle_rust_get_dpi_x (RsvgHandleRust *raw_handle);
extern double rsvg_handle_rust_get_dpi_y (RsvgHandleRust *raw_handle);
extern void rsvg_handle_rust_set_dpi_x (RsvgHandleRust *raw_handle, double dpi_x);
extern void rsvg_handle_rust_set_dpi_y (RsvgHandleRust *raw_handle, double dpi_y);
extern void rsvg_handle_rust_set_base_url (RsvgHandleRust *raw_handle, const char *uri);
extern RsvgNode *rsvg_handle_rust_get_root (RsvgHandleRust *raw_handle);
extern GFile *rsvg_handle_rust_get_base_gfile (RsvgHandleRust *raw_handle);
extern RsvgNode *rsvg_handle_defs_lookup (RsvgHandle *handle, const char *name);
extern gboolean rsvg_handle_rust_node_is_root(RsvgHandleRust *raw_handle, RsvgNode *node);

/* Implemented in rust/src/node.rs */
/* Call this as node = rsvg_node_unref (node);  Then node will be NULL and you don't own it anymore! */
extern RsvgNode *rsvg_node_unref (RsvgNode *node);

/* Implemented in rsvg_internals/src/structure.rs */
G_GNUC_INTERNAL
gboolean rsvg_node_svg_get_size (RsvgNode *node, double dpi_x, double dpi_y, int *out_width, int *out_height);

/* Defined in rsvg_internals/src/drawing_ctx.rs */
extern RsvgDrawingCtx *rsvg_drawing_ctx_new (RsvgHandle *handle,
                                             cairo_t *cr,
                                             guint width,
                                             guint height,
                                             double vb_width,
                                             double vb_height,
                                             gboolean testing);
extern void rsvg_drawing_ctx_free (RsvgDrawingCtx *draw_ctx);
extern void rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx,
                                                              RsvgNode       *node);
extern gboolean rsvg_drawing_ctx_draw_node_from_stack (RsvgDrawingCtx *ctx) G_GNUC_WARN_UNUSED_RESULT;
extern void rsvg_drawing_ctx_get_geometry (RsvgDrawingCtx *ctx,
                                           RsvgRectangle *ink_rect,
                                           RsvgRectangle *logical_rect);

enum {
    PROP_0,
    PROP_FLAGS,
    PROP_DPI_X,
    PROP_DPI_Y,
    PROP_BASE_URI,
    PROP_WIDTH,
    PROP_HEIGHT,
    PROP_EM,
    PROP_EX,
    PROP_TITLE,
    PROP_DESC,
    PROP_METADATA,
    NUM_PROPS
};

G_DEFINE_TYPE_WITH_CODE (RsvgHandle, rsvg_handle, G_TYPE_OBJECT,
                         G_ADD_PRIVATE (RsvgHandle))

static void
rsvg_handle_init (RsvgHandle * self)
{
    self->priv = rsvg_handle_get_instance_private (self);

    self->priv->flags = RSVG_HANDLE_FLAGS_NONE;
    self->priv->hstate = RSVG_HANDLE_STATE_START;

    self->priv->in_loop = FALSE;

    self->priv->is_testing = FALSE;

#ifdef HAVE_PANGOFT2
    self->priv->font_config_for_testing = NULL;
    self->priv->font_map_for_testing = NULL;
#endif

    self->priv->rust_handle = rsvg_handle_rust_new();
}

static void
rsvg_handle_dispose (GObject *instance)
{
    RsvgHandle *self = (RsvgHandle *) instance;

    if (self->priv->user_data_destroy) {
        (*self->priv->user_data_destroy) (self->priv->user_data);
        self->priv->user_data_destroy = NULL;
    }

    g_clear_pointer (&self->priv->load, rsvg_load_free);

    g_clear_pointer (&self->priv->base_uri, g_free);

#ifdef HAVE_PANGOFT2
    g_clear_pointer (&self->priv->font_config_for_testing, FcConfigDestroy);
    g_clear_object (&self->priv->font_map_for_testing);
#endif

    g_clear_pointer (&self->priv->rust_handle, rsvg_handle_rust_free);

    G_OBJECT_CLASS (rsvg_handle_parent_class)->dispose (instance);
}

static void
rsvg_handle_set_property (GObject * instance, guint prop_id, GValue const *value, GParamSpec * pspec)
{
    RsvgHandle *self = RSVG_HANDLE (instance);

    switch (prop_id) {
    case PROP_FLAGS:
        self->priv->flags = g_value_get_flags (value);
        break;
    case PROP_DPI_X:
        rsvg_handle_rust_set_dpi_x (self->priv->rust_handle, g_value_get_double (value));
        break;
    case PROP_DPI_Y:
        rsvg_handle_rust_set_dpi_y (self->priv->rust_handle, g_value_get_double (value));
        break;
    case PROP_BASE_URI:
        rsvg_handle_set_base_uri (self, g_value_get_string (value));
        break;
    default:
        G_OBJECT_WARN_INVALID_PROPERTY_ID (instance, prop_id, pspec);
    }
}

static void
rsvg_handle_get_property (GObject * instance, guint prop_id, GValue * value, GParamSpec * pspec)
{
    RsvgHandle *self = RSVG_HANDLE (instance);
    RsvgDimensionData dim;

    switch (prop_id) {
    case PROP_FLAGS:
        g_value_set_flags (value, self->priv->flags);
        break;
    case PROP_DPI_X:
        g_value_set_double (value, rsvg_handle_rust_get_dpi_x (self->priv->rust_handle));
        break;
    case PROP_DPI_Y:
        g_value_set_double (value, rsvg_handle_rust_get_dpi_y (self->priv->rust_handle));
        break;
    case PROP_BASE_URI:
        g_value_set_string (value, rsvg_handle_get_base_uri (self));
        break;
    case PROP_WIDTH:
        rsvg_handle_get_dimensions (self, &dim);
        g_value_set_int (value, dim.width);
        break;
    case PROP_HEIGHT:
        rsvg_handle_get_dimensions (self, &dim);
        g_value_set_int (value, dim.height);
        break;
    case PROP_EM:
        rsvg_handle_get_dimensions (self, &dim);
        g_value_set_double (value, dim.em);
        break;
    case PROP_EX:
        rsvg_handle_get_dimensions (self, &dim);
        g_value_set_double (value, dim.ex);
        break;
    case PROP_TITLE:
        /* deprecated */
        break;
    case PROP_DESC:
        /* deprecated */
        break;
    case PROP_METADATA:
        g_value_set_string (value, rsvg_handle_get_metadata (self));
        break;
    default:
        G_OBJECT_WARN_INVALID_PROPERTY_ID (instance, prop_id, pspec);
    }
}

static void
rsvg_handle_class_init (RsvgHandleClass * klass)
{
    GObjectClass *gobject_class = G_OBJECT_CLASS (klass);

    gobject_class->dispose = rsvg_handle_dispose;
    gobject_class->set_property = rsvg_handle_set_property;
    gobject_class->get_property = rsvg_handle_get_property;

    /**
     * RsvgHandle:flags:
     *
     * Flags from #RsvgHandleFlags.
     *
     * Since: 2.36
     */
    g_object_class_install_property (gobject_class,
                                     PROP_FLAGS,
                                     g_param_spec_flags ("flags", NULL, NULL,
                                                         RSVG_TYPE_HANDLE_FLAGS,
                                                         RSVG_HANDLE_FLAGS_NONE,
                                                         G_PARAM_READWRITE | G_PARAM_CONSTRUCT_ONLY));

    /**
     * dpi-x:
     */
    g_object_class_install_property (gobject_class,
                                     PROP_DPI_X,
                                     g_param_spec_double ("dpi-x", _("Horizontal resolution"),
                                                          _("Horizontal resolution"),
                                                          0., G_MAXDOUBLE, 0.,
                                                          (GParamFlags) (G_PARAM_READWRITE |
                                                                         G_PARAM_CONSTRUCT)));

    g_object_class_install_property (gobject_class,
                                     PROP_DPI_Y,
                                     g_param_spec_double ("dpi-y", _("Vertical resolution"),
                                                          _("Vertical resolution"),
                                                          0., G_MAXDOUBLE, 0.,
                                                          (GParamFlags) (G_PARAM_READWRITE |
                                                                         G_PARAM_CONSTRUCT)));

    g_object_class_install_property (gobject_class,
                                     PROP_BASE_URI,
                                     g_param_spec_string ("base-uri", _("Base URI"),
                                                          _("Base URI"), NULL,
                                                          (GParamFlags) (G_PARAM_READWRITE |
                                                                         G_PARAM_CONSTRUCT)));

    g_object_class_install_property (gobject_class,
                                     PROP_WIDTH,
                                     g_param_spec_int ("width", _("Image width"),
                                                       _("Image width"), 0, G_MAXINT, 0,
                                                       (GParamFlags) (G_PARAM_READABLE)));

    g_object_class_install_property (gobject_class,
                                     PROP_HEIGHT,
                                     g_param_spec_int ("height", _("Image height"),
                                                       _("Image height"), 0, G_MAXINT, 0,
                                                       (GParamFlags) (G_PARAM_READABLE)));

    g_object_class_install_property (gobject_class,
                                     PROP_EM,
                                     g_param_spec_double ("em", _("em"),
                                                          _("em"), 0, G_MAXDOUBLE, 0,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    g_object_class_install_property (gobject_class,
                                     PROP_EX,
                                     g_param_spec_double ("ex", _("ex"),
                                                          _("ex"), 0, G_MAXDOUBLE, 0,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    /**
     * RsvgHandle:title:
     *
     * SVG's description
     *
     * Deprecated: 2.36
     */
    g_object_class_install_property (gobject_class,
                                     PROP_TITLE,
                                     g_param_spec_string ("title", _("Title"),
                                                          _("SVG file title"), NULL,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    /**
     * RsvgHandle:desc:
     *
     * SVG's description
     *
     * Deprecated: 2.36
     */
    g_object_class_install_property (gobject_class,
                                     PROP_DESC,
                                     g_param_spec_string ("desc", _("Description"),
                                                          _("SVG file description"), NULL,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    /**
     * RsvgHandle:metadata:
     *
     * SVG's description
     *
     * Deprecated: 2.36
     */
    g_object_class_install_property (gobject_class,
                                     PROP_METADATA,
                                     g_param_spec_string ("metadata", _("Metadata"),
                                                          _("SVG file metadata"), NULL,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    xmlInitParser ();
}

/**
 * rsvg_handle_free:
 * @handle: An #RsvgHandle
 *
 * Frees @handle.
 * Deprecated: Use g_object_unref() instead.
 **/
void
rsvg_handle_free (RsvgHandle * handle)
{
    g_object_unref (handle);
}

/**
 * rsvg_handle_new:
 *
 * Returns a new rsvg handle.  Must be freed with @g_object_unref.  This
 * handle can be used for dynamically loading an image.  You need to feed it
 * data using @rsvg_handle_write, then call @rsvg_handle_close when done.
 * Afterwords, you can render it using Cairo or get a GdkPixbuf from it. When
 * finished, free with g_object_unref(). No more than one image can be loaded
 * with one handle.
 *
 * Returns: A new #RsvgHandle
 **/
RsvgHandle *
rsvg_handle_new (void)
{
    return RSVG_HANDLE (g_object_new (RSVG_TYPE_HANDLE, NULL));
}

static gboolean
rsvg_handle_fill_with_data (RsvgHandle *handle,
                            const char *data,
                            gsize data_len,
                            GError ** error)
{
    gboolean rv;

    rsvg_return_val_if_fail (data != NULL, FALSE, error);
    rsvg_return_val_if_fail (data_len != 0, FALSE, error);

    rv = rsvg_handle_write (handle, (guchar *) data, data_len, error);

    return rsvg_handle_close (handle, rv ? error : NULL) && rv;
}

/**
 * rsvg_handle_new_from_data:
 * @data: (array length=data_len): The SVG data
 * @data_len: The length of @data, in bytes
 * @error: return location for errors
 *
 * Loads the SVG specified by @data.
 *
 * Returns: A #RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_data (const guint8 *data, gsize data_len, GError **error)
{
    RsvgHandle *handle;

    handle = rsvg_handle_new ();

    if (handle) {
        if (!rsvg_handle_fill_with_data (handle, (char *) data, data_len, error)) {
            g_object_unref (handle);
            handle = NULL;
        }
    }

    return handle;
}

/**
 * rsvg_handle_new_from_file:
 * @file_name: The file name to load. If built with gnome-vfs, can be a URI.
 * @error: return location for errors
 *
 * Loads the SVG specified by @file_name.
 *
 * Returns: A #RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_file (const gchar *file_name, GError **error)
{
    gchar *base_uri;
    RsvgHandle *handle;
    GFile *file;
    char *scheme;

    rsvg_return_val_if_fail (file_name != NULL, NULL, error);

    scheme = g_uri_parse_scheme (file_name);
    if (scheme) {
        file = g_file_new_for_uri (file_name);
        g_free (scheme);
    } else {
        file = g_file_new_for_path (file_name);
    }

    base_uri = g_file_get_uri (file);
    if (!base_uri) {
        g_set_error (error,
                     G_IO_ERROR,
                     G_IO_ERROR_FAILED,
                     _("Cannot obtain URI from '%s'"), file_name);
        g_object_unref (file);
        return NULL;
    }
    g_free (base_uri);

    handle = rsvg_handle_new_from_gfile_sync (file, 0, NULL, error);
    g_object_unref (file);

    return handle;
}

/**
 * rsvg_handle_new_with_flags:
 * @flags: flags from #RsvgHandleFlags
 *
 * Creates a new #RsvgHandle with flags @flags.
 *
 * Returns: (transfer full): a new #RsvgHandle
 *
 * Since: 2.36
 **/
RsvgHandle *
rsvg_handle_new_with_flags (RsvgHandleFlags flags)
{
    return g_object_new (RSVG_TYPE_HANDLE,
                         "flags", flags,
                         NULL);
}

/**
 * rsvg_handle_new_from_gfile_sync:
 * @file: a #GFile
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @file.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: a new #RsvgHandle on success, or %NULL with @error filled in
 *
 * Since: 2.32
 */
RsvgHandle *
rsvg_handle_new_from_gfile_sync (GFile          *file,
                                 RsvgHandleFlags flags,
                                 GCancellable   *cancellable,
                                 GError        **error)
{
    RsvgHandle *handle;
    GFileInputStream *stream;

    g_return_val_if_fail (G_IS_FILE (file), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    stream = g_file_read (file, cancellable, error);
    if (stream == NULL)
        return NULL;

    handle = rsvg_handle_new_from_stream_sync (G_INPUT_STREAM (stream), file,
                                               flags, cancellable, error);
    g_object_unref (stream);

    return handle;
}

/**
 * rsvg_handle_new_from_stream_sync:
 * @input_stream: a #GInputStream
 * @base_file: (allow-none): a #GFile, or %NULL
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (allow-none): a #GCancellable, or %NULL
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @stream.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned.
 *
 * Returns: a new #RsvgHandle on success, or %NULL with @error filled in
 *
 * Since: 2.32
 */
RsvgHandle *
rsvg_handle_new_from_stream_sync (GInputStream   *input_stream,
                                  GFile          *base_file,
                                  RsvgHandleFlags flags,
                                  GCancellable    *cancellable,
                                  GError         **error)
{
    RsvgHandle *handle;

    g_return_val_if_fail (G_IS_INPUT_STREAM (input_stream), NULL);
    g_return_val_if_fail (base_file == NULL || G_IS_FILE (base_file), NULL);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    handle = rsvg_handle_new_with_flags (flags);

    if (base_file)
        rsvg_handle_set_base_gfile (handle, base_file);

    if (!rsvg_handle_read_stream_sync (handle, input_stream, cancellable, error)) {
        g_object_unref (handle);
        return NULL;
    }

    return handle;
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

    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);
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
finish_load (RsvgHandle *handle, gboolean was_successful, GError **error)
{
    g_assert (handle->priv->load != NULL);

    if (was_successful) {
        g_assert (error == NULL || *error == NULL);

        was_successful = rsvg_load_finish_load(handle->priv->load, error);
    }

    if (was_successful) {
        handle->priv->hstate = RSVG_HANDLE_STATE_CLOSED_OK;
    } else {
        handle->priv->hstate = RSVG_HANDLE_STATE_CLOSED_ERROR;
    }

    g_clear_pointer (&handle->priv->load, rsvg_load_free);

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
    gboolean read_successfully;
    gboolean result = FALSE;

    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);
    rsvg_return_val_if_fail (handle, FALSE, error);

    priv = handle->priv;

    switch (priv->hstate) {
    case RSVG_HANDLE_STATE_START:
        g_set_error (error, RSVG_ERROR, RSVG_ERROR_FAILED, _("no data passed to parser"));
        priv->hstate = RSVG_HANDLE_STATE_CLOSED_ERROR;
        result = FALSE;
        break;

    case RSVG_HANDLE_STATE_LOADING:
        g_assert (priv->load != NULL);
        read_successfully = rsvg_load_close (priv->load, error);
        result = finish_load (handle, read_successfully, error);
        break;

    case RSVG_HANDLE_STATE_CLOSED_OK:
    case RSVG_HANDLE_STATE_CLOSED_ERROR:
        /* closing is idempotent */
        result = TRUE;
        break;

    default:
        g_assert_not_reached ();
    }

    g_assert (priv->hstate == RSVG_HANDLE_STATE_CLOSED_OK
              || priv->hstate == RSVG_HANDLE_STATE_CLOSED_ERROR);

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
    gboolean read_successfully;
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

    read_successfully = rsvg_load_read_stream_sync (priv->load, stream, cancellable, error);
    result = finish_load (handle, read_successfully, error);

    priv->load = saved_load;

    return result;
}

/* http://www.ietf.org/rfc/rfc2396.txt */

static gboolean
path_is_uri (char const *path)
{
    char const *p;

    if (path == NULL)
        return FALSE;

    if (strlen (path) < 4)
        return FALSE;

    if ((path[0] < 'a' || path[0] > 'z') &&
        (path[0] < 'A' || path[0] > 'Z')) {
        return FALSE;
    }

    for (p = &path[1];
	    (*p >= 'a' && *p <= 'z') ||
        (*p >= 'A' && *p <= 'Z') ||
        (*p >= '0' && *p <= '9') ||
         *p == '+' ||
         *p == '-' ||
         *p == '.';
        p++);

    if (strlen (p) < 3)
        return FALSE;

    return (p[0] == ':' && p[1] == '/' && p[2] == '/');
}

static gchar *
get_base_uri_from_filename (const gchar * filename)
{
    gchar *current_dir;
    gchar *absolute_filename;
    gchar *base_uri;

    if (g_path_is_absolute (filename))
        return g_filename_to_uri (filename, NULL, NULL);

    current_dir = g_get_current_dir ();
    absolute_filename = g_build_filename (current_dir, filename, NULL);
    base_uri = g_filename_to_uri (absolute_filename, NULL, NULL);
    g_free (absolute_filename);
    g_free (current_dir);

    return base_uri;
}

/**
 * rsvg_handle_set_base_uri:
 * @handle: A #RsvgHandle
 * @base_uri: The base uri
 *
 * Set the base URI for this SVG. This can only be called before rsvg_handle_write()
 * has been called.
 *
 * Since: 2.9
 */
void
rsvg_handle_set_base_uri (RsvgHandle * handle, const char *base_uri)
{
    gchar *uri;
    GFile *file;

    g_return_if_fail (RSVG_IS_HANDLE (handle));

    if (base_uri == NULL)
        return;

    if (path_is_uri (base_uri))
        uri = g_strdup (base_uri);
    else
        uri = get_base_uri_from_filename (base_uri);

    file = g_file_new_for_uri (uri ? uri : "data:");
    rsvg_handle_set_base_gfile (handle, file);
    g_object_unref (file);
    g_free (uri);
}

/**
 * rsvg_handle_set_base_gfile:
 * @handle: a #RsvgHandle
 * @base_file: a #GFile
 *
 * Set the base URI for @handle from @file.
 * Note: This function may only be called before rsvg_handle_write()
 * or rsvg_handle_read_stream_sync() has been called.
 *
 * Since: 2.32
 */
void
rsvg_handle_set_base_gfile (RsvgHandle *handle,
                            GFile      *base_file)
{
    RsvgHandlePrivate *priv;
    char *uri;
    GFile *real_base_file;

    g_return_if_fail (RSVG_IS_HANDLE (handle));
    g_return_if_fail (G_IS_FILE (base_file));

    priv = handle->priv;

    uri = g_file_get_uri (base_file);
    rsvg_handle_rust_set_base_url (priv->rust_handle, uri);
    g_free (uri);

    /* Obtain the sanitized version */

    real_base_file = rsvg_handle_rust_get_base_gfile (priv->rust_handle);
    g_free (priv->base_uri);

    if (real_base_file) {
        priv->base_uri = g_file_get_uri (real_base_file);
    } else {
        priv->base_uri = NULL;
    }
}

/**
 * rsvg_handle_get_base_uri:
 * @handle: A #RsvgHandle
 *
 * Gets the base uri for this #RsvgHandle.
 *
 * Returns: the base uri, possibly null
 * Since: 2.8
 */
const char *
rsvg_handle_get_base_uri (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);
    return handle->priv->base_uri;
}

/**
 * rsvg_handle_get_metadata:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns #NULL.
 *
 * Since: 2.9
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_metadata (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    return NULL;
}

/**
 * rsvg_handle_get_title:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns NULL.
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_title (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    return NULL;
}

/**
 * rsvg_handle_get_desc:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns NULL.
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_desc (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    return NULL;
}

guint
rsvg_handle_get_flags (RsvgHandle *handle)
{
    return (guint) handle->priv->flags;
}

RsvgHandleRust *
rsvg_handle_get_rust (RsvgHandle *handle)
{
    return handle->priv->rust_handle;
}

gboolean
rsvg_handle_keep_image_data (RsvgHandle *handle)
{
    return (handle->priv->flags & RSVG_HANDLE_FLAG_KEEP_IMAGE_DATA) != 0;
}

static RsvgDrawingCtx *
rsvg_handle_create_drawing_ctx(RsvgHandle *handle,
                               cairo_t *cr,
                               RsvgDimensionData *dimensions)
{
    return rsvg_drawing_ctx_new (handle,
                                 cr,
                                 dimensions->width, dimensions->height,
                                 dimensions->em, dimensions->ex,
                                 handle->priv->is_testing);
}

/**
 * rsvg_handle_render_cairo_sub:
 * @handle: A #RsvgHandle
 * @cr: A Cairo renderer
 * @id: (nullable): An element's id within the SVG, or %NULL to render
 *   the whole SVG. For example, if you have a layer called "layer1"
 *   that you wish to render, pass "##layer1" as the id.
 *
 * Draws a subset of a loaded SVG handle to a Cairo context.  Drawing will occur with
 * respect to the @cr's current transformation:  for example, if the @cr has a
 * rotated current transformation matrix, the whole SVG will be rotated in the
 * rendered version.
 *
 * Returns: %TRUE if drawing succeeded; %FALSE otherwise.
 * Since: 2.14
 */
gboolean
rsvg_handle_render_cairo_sub (RsvgHandle * handle, cairo_t * cr, const char *id)
{
    RsvgDimensionData dimensions;
    RsvgDrawingCtx *draw;
    RsvgNode *drawsub = NULL;
    cairo_status_t status;
    gboolean res;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);

    if (handle->priv->hstate != RSVG_HANDLE_STATE_CLOSED_OK)
        return FALSE;

    status = cairo_status (cr);

    if (status != CAIRO_STATUS_SUCCESS) {
        g_warning ("cannot render on a cairo_t with a failure status (status=%d, %s)",
                   (int) status,
                   cairo_status_to_string (status));
        return FALSE;
    }

    if (id && *id)
        drawsub = rsvg_handle_defs_lookup (handle, id);

    if (drawsub == NULL && id != NULL) {
        g_warning ("element id=\"%s\" does not exist", id);
        /* todo: there's no way to signal that @id doesn't exist */
        return FALSE;
    }

    rsvg_handle_get_dimensions (handle, &dimensions);
    if (dimensions.width == 0 || dimensions.height == 0)
        return FALSE;

    cairo_save (cr);

    draw = rsvg_handle_create_drawing_ctx (handle, cr, &dimensions);

    if (drawsub != NULL) {
        rsvg_drawing_ctx_add_node_and_ancestors_to_stack (draw, drawsub);
    }

    rsvg_handle_rust_cascade (handle->priv->rust_handle);
    res = rsvg_drawing_ctx_draw_node_from_stack (draw);

    rsvg_drawing_ctx_free (draw);

    cairo_restore (cr);

    return res;
}

/**
 * rsvg_handle_render_cairo:
 * @handle: A #RsvgHandle
 * @cr: A Cairo renderer
 *
 * Draws a loaded SVG handle to a Cairo context.  Drawing will occur with
 * respect to the @cr's current transformation:  for example, if the @cr has a
 * rotated current transformation matrix, the whole SVG will be rotated in the
 * rendered version.
 *
 * Returns: %TRUE if drawing succeeded; %FALSE otherwise.
 * Since: 2.14
 */
gboolean
rsvg_handle_render_cairo (RsvgHandle * handle, cairo_t * cr)
{
    return rsvg_handle_render_cairo_sub (handle, cr, NULL);
}

/**
 * rsvg_handle_get_dimensions:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 *
 * Get the SVG's size. Do not call from within the size_func callback, because an infinite loop will occur.
 *
 * Since: 2.14
 */
void
rsvg_handle_get_dimensions (RsvgHandle * handle, RsvgDimensionData * dimension_data)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));
    g_return_if_fail (dimension_data != NULL);

    /* This function is probably called from the cairo_render functions.
     * To prevent an infinite loop we are saving the state.
     */
    if (!handle->priv->in_loop) {
        handle->priv->in_loop = TRUE;
        rsvg_handle_get_dimensions_sub (handle, dimension_data, NULL);
        handle->priv->in_loop = FALSE;
    } else {
        /* Called within the size function, so return a standard size */
        dimension_data->em = dimension_data->width = 1;
        dimension_data->ex = dimension_data->height = 1;
    }
}

static gboolean
get_node_geometry(RsvgHandle *handle, RsvgNode *node, RsvgRectangle *ink_rect, RsvgRectangle *logical_rect)
{
    RsvgDimensionData dimensions;
    cairo_surface_t *target;
    cairo_t *cr;
    RsvgDrawingCtx *draw;
    gboolean res = FALSE;

    g_assert (node != NULL);

    rsvg_handle_get_dimensions (handle, &dimensions);
    if (dimensions.width == 0 || dimensions.height == 0)
        return res;

    target = cairo_image_surface_create (CAIRO_FORMAT_RGB24, 1, 1);
    cr = cairo_create (target);

    draw = rsvg_handle_create_drawing_ctx (handle, cr, &dimensions);
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack (draw, node);

    rsvg_handle_rust_cascade (handle->priv->rust_handle);
    /* FIXME: expose this as a RenderingError in the public API */
    res = rsvg_drawing_ctx_draw_node_from_stack (draw);
    if (res) {
        rsvg_drawing_ctx_get_geometry (draw, ink_rect, logical_rect);
    }

    rsvg_drawing_ctx_free (draw);
    cairo_destroy (cr);
    cairo_surface_destroy (target);

    return res;
}

/**
 * rsvg_handle_get_dimensions_sub:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 * @id: (nullable): An element's id within the SVG, starting with "##", for
 * example, "##layer1"; or %NULL to use the whole SVG.
 *
 * Get the size of a subelement of the SVG file. Do not call from within the
 * size_func callback, because an infinite loop will occur.
 *
 * Deprecated: Use rsvg_handle_get_geometry_sub() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_dimensions_sub (RsvgHandle * handle, RsvgDimensionData * dimension_data, const char *id)
{
    RsvgRectangle ink_r;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (dimension_data, FALSE);

    memset (&ink_r, 0, sizeof (RsvgRectangle));
    memset (dimension_data, 0, sizeof (RsvgDimensionData));

    if (!rsvg_handle_get_geometry_sub (handle, &ink_r, NULL, id)) {
        return FALSE;
    }

    dimension_data->width = ink_r.width;
    dimension_data->height = ink_r.height;
    dimension_data->em = dimension_data->width;
    dimension_data->ex = dimension_data->height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data->width, &dimension_data->height,
                                    handle->priv->user_data);
    return TRUE;
}

/**
 * rsvg_handle_get_geometry_sub:
 * @handle: A #RsvgHandle
 * @ink_rect: (out)(nullable): A place to store the SVG fragment's geometry.
 * @logical_rect: (out)(nullable): A place to store the SVG fragment's logical geometry.
 * @id: (nullable): An element's id within the SVG, starting with "##", for
 * example, "##layer1"; or %NULL to use the whole SVG.
 *
 * Get the geometry of a subelement of the SVG file.
 *
 * Note that unlike rsvg_handle_get_position_sub() and
 * rsvg_handle_get_dimensions_sub(), this function does not call the size_func.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_get_geometry_sub (RsvgHandle * handle, RsvgRectangle * ink_rect, RsvgRectangle * logical_rect, const char *id)
{
    RsvgNode *root = NULL;
    RsvgNode *node = NULL;
    gboolean has_size;
    int root_width, root_height;
    gboolean res = FALSE;
    RsvgRectangle ink_r, logical_r;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);

    memset (&ink_r, 0, sizeof (RsvgRectangle));
    memset (&logical_r, 0, sizeof (RsvgRectangle));

    g_return_val_if_fail (handle->priv->hstate == RSVG_HANDLE_STATE_CLOSED_OK, FALSE);

    root = rsvg_handle_rust_get_root (handle->priv->rust_handle);

    if (id && *id) {
        node = rsvg_handle_defs_lookup (handle, id);

        if (node && rsvg_handle_rust_node_is_root (handle->priv->rust_handle, node))
            id = NULL;
    }

    if (!node && id) {
        goto out;
    }

    has_size = rsvg_node_svg_get_size (root,
                                       rsvg_handle_rust_get_dpi_x (handle->priv->rust_handle),
                                       rsvg_handle_rust_get_dpi_y (handle->priv->rust_handle),
                                       &root_width, &root_height);

    if (id || !has_size) {
        res = get_node_geometry (handle, node ? node : root, &ink_r, &logical_r);
        if (!res) {
            goto out;
        }
    } else {
        ink_r.width = root_width;
        ink_r.height = root_height;
        ink_r.x = 0;
        ink_r.y = 0;

        logical_r.width = root_width;
        logical_r.height = root_height;
        logical_r.x = 0;
        logical_r.y = 0;
    }

    res = TRUE;

out:

    if (ink_rect != NULL) {
        *ink_rect = ink_r;
    }

    if (logical_rect != NULL) {
        *logical_rect = logical_r;
    }

    g_clear_pointer (&node, rsvg_node_unref);
    g_clear_pointer (&root, rsvg_node_unref);

    return res;
}

/**
 * rsvg_handle_get_position_sub:
 * @handle: A #RsvgHandle
 * @position_data: (out): A place to store the SVG fragment's position.
 * @id: (nullable): An element's id within the SVG, starting with "##", for
 * example, "##layer1"; or %NULL to use the whole SVG.
 *
 * Get the position of a subelement of the SVG file. Do not call from within
 * the size_func callback, because an infinite loop will occur.
 *
 * Deprecated: Use rsvg_handle_get_geometry_sub() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_position_sub (RsvgHandle * handle, RsvgPositionData * position_data, const char *id)
{
    RsvgRectangle ink_r;
    int width, height;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (position_data != NULL, FALSE);

    memset (position_data, 0, sizeof (*position_data));

    /* Short-cut when no id is given. */
    if (NULL == id || '\0' == *id)
        return TRUE;

    if (!rsvg_handle_get_geometry_sub (handle, &ink_r, NULL, id))
        return FALSE;

    position_data->x = ink_r.x;
    position_data->y = ink_r.y;

    width = ink_r.width;
    height = ink_r.height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&width, &height, handle->priv->user_data);

    return TRUE;
}

/**
 * rsvg_handle_has_sub:
 * @handle: a #RsvgHandle
 * @id: an element's id within the SVG, starting with "##", for example, "##layer1".
 *
 * Checks whether the element @id exists in the SVG document.
 *
 * Returns: %TRUE if @id exists in the SVG document
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_has_sub (RsvgHandle * handle,
                     const char *id)
{
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);

    if (G_UNLIKELY (!id || !id[0]))
      return FALSE;

    return rsvg_handle_defs_lookup (handle, id) != NULL;
}

/**
 * rsvg_handle_get_pixbuf_sub:
 * @handle: An #RsvgHandle
 * @id: (nullable): An element's id within the SVG, starting with "##", for
 * example, "##layer1"; or %NULL to use the whole SVG.
 *
 * Creates a #GdkPixbuf the same size as the entire SVG loaded into @handle, but
 * only renders the sub-element that has the specified @id (and all its
 * sub-sub-elements recursively).  If @id is #NULL, this function renders the
 * whole SVG.
 *
 * If you need to render an image which is only big enough to fit a particular
 * sub-element of the SVG, consider using rsvg_handle_render_cairo_sub(), upon a
 * surface that is just the size returned by rsvg_handle_get_dimensions_sub().
 * You will need to offset the rendering by the amount returned in
 * rsvg_handle_get_position_sub().
 *
 * Returns: (transfer full) (nullable): a pixbuf, or %NULL if an error occurs
 * during rendering.
 *
 * Since: 2.14
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf_sub (RsvgHandle * handle, const char *id)
{
    RsvgDimensionData dimensions;
    GdkPixbuf *output = NULL;
    cairo_surface_t *surface;
    cairo_t *cr;

    g_return_val_if_fail (RSVG_IS_HANDLE (handle), NULL);

    if (handle->priv->hstate != RSVG_HANDLE_STATE_CLOSED_OK)
        return NULL;

    rsvg_handle_get_dimensions (handle, &dimensions);
    if (!(dimensions.width && dimensions.height))
        return NULL;

    surface = cairo_image_surface_create (CAIRO_FORMAT_ARGB32,
                                          dimensions.width, dimensions.height);
    if (cairo_surface_status (surface) != CAIRO_STATUS_SUCCESS) {
        cairo_surface_destroy (surface);
        return NULL;
    }

    cr = cairo_create (surface);

    if (!rsvg_handle_render_cairo_sub (handle, cr, id)) {
        cairo_destroy (cr);
        cairo_surface_destroy (surface);
        return NULL;
    }

    cairo_destroy (cr);

    output = rsvg_cairo_surface_to_pixbuf (surface);
    cairo_surface_destroy (surface);

    return output;
}

/**
 * rsvg_handle_get_pixbuf:
 * @handle: An #RsvgHandle
 *
 * Returns the pixbuf loaded by @handle.  The pixbuf returned will be reffed, so
 * the caller of this function must assume that ref.  If insufficient data has
 * been read to create the pixbuf, or an error occurred in loading, then %NULL
 * will be returned.  Note that the pixbuf may not be complete until
 * @rsvg_handle_close has been called.
 *
 * Returns: (transfer full) (nullable): the pixbuf loaded by @handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle * handle)
{
    return rsvg_handle_get_pixbuf_sub (handle, NULL);
}

/**
 * rsvg_handle_set_dpi:
 * @handle: An #RsvgHandle
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to @dpi will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi (RsvgHandle * handle, double dpi)
{
    rsvg_handle_set_dpi_x_y (handle, dpi, dpi);
}

/**
 * rsvg_handle_set_dpi_x_y:
 * @handle: An #RsvgHandle
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Sets the DPI for the outgoing pixbuf. Common values are
 * 75, 90, and 300 DPI. Passing a number <= 0 to #dpi_x or @dpi_y will
 * reset the DPI to whatever the default value happens to be.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi_x_y (RsvgHandle * handle, double dpi_x, double dpi_y)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));

    rsvg_handle_rust_set_dpi_x (handle->priv->rust_handle, dpi_x);
    rsvg_handle_rust_set_dpi_y (handle->priv->rust_handle, dpi_y);
}

/**
 * rsvg_handle_set_size_callback:
 * @handle: An #RsvgHandle
 * @size_func: (nullable): A sizing function, or %NULL
 * @user_data: User data to pass to @size_func, or %NULL
 * @user_data_destroy: Destroy function for @user_data, or %NULL
 *
 * Sets the sizing function for the @handle.  This function is called right
 * after the size of the image has been loaded.  The size of the image is passed
 * in to the function, which may then modify these values to set the real size
 * of the generated pixbuf.  If the image has no associated size, then the size
 * arguments are set to -1.
 *
 * Deprecated: Set up a cairo matrix and use rsvg_handle_render_cairo() instead.
 * You can call rsvg_handle_get_dimensions() to figure out the size of your SVG,
 * and then scale it to the desired size via Cairo.  For example, the following
 * code renders an SVG at a specified size, scaled proportionally from whatever
 * original size it may have had:
 *
 * |[<!-- language="C" -->
 * void
 * render_scaled_proportionally (RsvgHandle *handle, cairo_t cr, int width, int height)
 * {
 *     RsvgDimensionData dimensions;
 *     double x_factor, y_factor;
 *     double scale_factor;
 *
 *     rsvg_handle_get_dimensions (handle, &dimensions);
 *
 *     x_factor = (double) width / dimensions.width;
 *     y_factor = (double) height / dimensions.height;
 *
 *     scale_factor = MIN (x_factor, y_factor);
 *
 *     cairo_scale (cr, scale_factor, scale_factor);
 *
 *     rsvg_handle_render_cairo (handle, cr);
 * }
 * ]|
 **/
void
rsvg_handle_set_size_callback (RsvgHandle * handle,
                               RsvgSizeFunc size_func,
                               gpointer user_data, GDestroyNotify user_data_destroy)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));

    if (handle->priv->user_data_destroy)
        (*handle->priv->user_data_destroy) (handle->priv->user_data);

    handle->priv->size_func = size_func;
    handle->priv->user_data = user_data;
    handle->priv->user_data_destroy = user_data_destroy;
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

static void
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

/**
 * _rsvg_handle_internal_set_testing:
 * @handle: a #RsvgHandle
 * @testing: Whether to enable testing mode
 *
 * Do not call this function.  This is intended for librsvg's internal
 * test suite only.
 **/
void
rsvg_handle_internal_set_testing (RsvgHandle *handle, gboolean testing)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));

    handle->priv->is_testing = testing ? TRUE : FALSE;

    rsvg_handle_update_font_map_for_testing (handle);
}
