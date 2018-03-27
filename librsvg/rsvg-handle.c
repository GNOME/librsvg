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
#include <string.h>

#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "rsvg-cairo-render.h"
#include "rsvg-structure.h"

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

extern double rsvg_internal_dpi_x;
extern double rsvg_internal_dpi_y;

G_DEFINE_TYPE (RsvgHandle, rsvg_handle, G_TYPE_OBJECT)

static void
rsvg_handle_init (RsvgHandle * self)
{
    self->priv = G_TYPE_INSTANCE_GET_PRIVATE (self, RSVG_TYPE_HANDLE, RsvgHandlePrivate);

    self->priv->flags = RSVG_HANDLE_FLAGS_NONE;
    self->priv->hstate = RSVG_HANDLE_STATE_START;
    self->priv->all_nodes = g_ptr_array_new ();
    self->priv->defs = rsvg_defs_new (self);
    self->priv->dpi_x = rsvg_internal_dpi_x;
    self->priv->dpi_y = rsvg_internal_dpi_y;

    self->priv->css_props = g_hash_table_new_full (g_str_hash,
                                                   g_str_equal,
                                                   g_free,
                                                   (GDestroyNotify) g_hash_table_destroy);

    self->priv->treebase = NULL;

    self->priv->cancellable = NULL;

    self->priv->is_disposed = FALSE;
    self->priv->in_loop = FALSE;

    self->priv->is_testing = FALSE;
}

static void
free_nodes (RsvgHandle *self)
{
    int i;

    g_assert (self->priv->all_nodes != NULL);

    for (i = 0; i < self->priv->all_nodes->len; i++) {
        RsvgNode *node;

        node = g_ptr_array_index (self->priv->all_nodes, i);
        node = rsvg_node_unref (node);
    }

    g_ptr_array_free (self->priv->all_nodes, TRUE);
    self->priv->all_nodes = NULL;
}

static void
rsvg_handle_dispose (GObject *instance)
{
    RsvgHandle *self = (RsvgHandle *) instance;

    if (self->priv->is_disposed)
      goto chain;

    self->priv->is_disposed = TRUE;

    free_nodes (self);

    rsvg_defs_free (self->priv->defs);
    self->priv->defs = NULL;

    g_hash_table_destroy (self->priv->css_props);

    self->priv->treebase = rsvg_node_unref (self->priv->treebase);

    if (self->priv->user_data_destroy)
        (*self->priv->user_data_destroy) (self->priv->user_data);

    if (self->priv->title)
        g_string_free (self->priv->title, TRUE);
    if (self->priv->desc)
        g_string_free (self->priv->desc, TRUE);
    if (self->priv->base_uri)
        g_free (self->priv->base_uri);

    if (self->priv->base_gfile) {
        g_object_unref (self->priv->base_gfile);
        self->priv->base_gfile = NULL;
    }
    if (self->priv->load) {
        rsvg_load_destroy (self->priv->load);
        self->priv->load = NULL;
    }

    g_clear_object (&self->priv->cancellable);

  chain:
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
        rsvg_handle_set_dpi_x_y (self, g_value_get_double (value), self->priv->dpi_y);
        break;
    case PROP_DPI_Y:
        rsvg_handle_set_dpi_x_y (self, self->priv->dpi_x, g_value_get_double (value));
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
        g_value_set_double (value, self->priv->dpi_x);
        break;
    case PROP_DPI_Y:
        g_value_set_double (value, self->priv->dpi_y);
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
        g_value_set_string (value, rsvg_handle_get_title (self));
        break;
    case PROP_DESC:
        g_value_set_string (value, rsvg_handle_get_desc (self));
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
                                                          _("Horizontal resolution"), 0.,
                                                          G_MAXDOUBLE, rsvg_internal_dpi_x,
                                                          (GParamFlags) (G_PARAM_READWRITE |
                                                                         G_PARAM_CONSTRUCT)));

    g_object_class_install_property (gobject_class,
                                     PROP_DPI_Y,
                                     g_param_spec_double ("dpi-y", _("Vertical resolution"),
                                                          _("Vertical resolution"), 0., G_MAXDOUBLE,
                                                          rsvg_internal_dpi_y,
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

    g_type_class_add_private (klass, sizeof (RsvgHandlePrivate));

    xmlInitParser ();

    rsvg_SAX_handler_struct_init ();
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

    g_return_if_fail (handle != NULL);

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

    g_return_if_fail (RSVG_IS_HANDLE (handle));
    g_return_if_fail (G_IS_FILE (base_file));

    priv = handle->priv;

    g_object_ref (base_file);
    if (priv->base_gfile)
        g_object_unref (priv->base_gfile);
    priv->base_gfile = base_file;

    g_free (priv->base_uri);
    priv->base_uri = g_file_get_uri (base_file);
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
 * Returns the SVG's title in UTF-8 or %NULL. You must make a copy
 * of this title if you wish to use it after @handle has been freed.
 *
 * Returns: (nullable): The SVG's title
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_title (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->title)
        return handle->priv->title->str;
    else
        return NULL;
}

/**
 * rsvg_handle_get_desc:
 * @handle: An #RsvgHandle
 *
 * Returns the SVG's description in UTF-8 or %NULL. You must make a copy
 * of this description if you wish to use it after @handle has been freed.
 *
 * Returns: (nullable): The SVG's description
 *
 * Since: 2.4
 *
 * Deprecated: 2.36
 */
const char *
rsvg_handle_get_desc (RsvgHandle * handle)
{
    g_return_val_if_fail (handle, NULL);

    if (handle->priv->desc)
        return handle->priv->desc->str;
    else
        return NULL;
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
 * Since: 2.22
 */
gboolean
rsvg_handle_get_dimensions_sub (RsvgHandle * handle, RsvgDimensionData * dimension_data, const char *id)
{
    cairo_t *cr;
    cairo_surface_t *target;
    RsvgDrawingCtx *draw;
    RsvgNode *sself = NULL;
    RsvgBbox bbox;
    RsvgLength root_width, root_height;
    RsvgViewBox root_vbox;

    gboolean handle_subelement = TRUE;

    g_return_val_if_fail (handle, FALSE);
    g_return_val_if_fail (dimension_data, FALSE);

    memset (dimension_data, 0, sizeof (RsvgDimensionData));

    if (id && *id) {
        sself = rsvg_defs_lookup (handle->priv->defs, id);

        if (rsvg_node_is_same (sself, handle->priv->treebase))
            id = NULL;
    } else {
        sself = handle->priv->treebase;
    }

    if (!sself && id)
        return FALSE;

    if (!handle->priv->treebase)
        return FALSE;

    g_assert (rsvg_node_get_type (handle->priv->treebase) == RSVG_NODE_TYPE_SVG);

    bbox.rect.x = bbox.rect.y = 0;
    bbox.rect.width = bbox.rect.height = 1;

    rsvg_node_svg_get_size (handle->priv->treebase, &root_width, &root_height);
    root_vbox = rsvg_node_svg_get_view_box (handle->priv->treebase);

    if (!id) {
        if ((root_width.unit == LENGTH_UNIT_PERCENT || root_height.unit == LENGTH_UNIT_PERCENT) && !root_vbox.active)
            handle_subelement = TRUE;
        else
            handle_subelement = FALSE;
    }

    if (handle_subelement == TRUE) {
        target = cairo_image_surface_create (CAIRO_FORMAT_RGB24,
                                             1, 1);
        cr = cairo_create  (target);

        draw = rsvg_cairo_new_drawing_ctx (cr, handle);

        if (!draw) {
            cairo_destroy (cr);
            cairo_surface_destroy (target);

            return FALSE;
        }

        g_assert (sself != NULL);
        rsvg_drawing_ctx_add_node_and_ancestors_to_stack (draw, sself);

        rsvg_drawing_ctx_draw_node_from_stack (draw, handle->priv->treebase, 0);
        bbox = RSVG_CAIRO_RENDER (draw->render)->bbox;

        rsvg_drawing_ctx_free (draw);
        cairo_destroy (cr);
        cairo_surface_destroy (target);

        dimension_data->width = bbox.rect.width;
        dimension_data->height = bbox.rect.height;
    } else {
        bbox.rect.width = root_vbox.rect.width;
        bbox.rect.height = root_vbox.rect.height;

        dimension_data->width = (int) (rsvg_length_hand_normalize (&root_width, handle->priv->dpi_x,
                                                                   bbox.rect.width, 12) + 0.5);
        dimension_data->height = (int) (rsvg_length_hand_normalize (&root_height, handle->priv->dpi_y,
                                                                    bbox.rect.height, 12) + 0.5);
    }

    dimension_data->em = dimension_data->width;
    dimension_data->ex = dimension_data->height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data->width, &dimension_data->height,
                                    handle->priv->user_data);

    return TRUE;
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
 * Since: 2.22
 */
gboolean
rsvg_handle_get_position_sub (RsvgHandle * handle, RsvgPositionData * position_data, const char *id)
{
    RsvgDrawingCtx		*draw;
    RsvgNode			*node;
    RsvgBbox			 bbox;
    RsvgDimensionData    dimension_data;
    cairo_surface_t		*target = NULL;
    cairo_t				*cr = NULL;
    gboolean			 ret = FALSE;

    g_return_val_if_fail (handle, FALSE);
    g_return_val_if_fail (position_data, FALSE);

    if (!handle->priv->treebase)
        return FALSE;

    /* Short-cut when no id is given. */
    if (NULL == id || '\0' == *id) {
        position_data->x = 0;
        position_data->y = 0;
        return TRUE;
    }

    memset (position_data, 0, sizeof (*position_data));
    memset (&dimension_data, 0, sizeof (dimension_data));

    node = rsvg_defs_lookup (handle->priv->defs, id);
    if (!node) {
        return FALSE;
    } else if (rsvg_node_is_same (node, handle->priv->treebase)) {
        /* Root node. */
        position_data->x = 0;
        position_data->y = 0;
        return TRUE;
    }

    target = cairo_image_surface_create (CAIRO_FORMAT_RGB24, 1, 1);
    cr = cairo_create  (target);
    draw = rsvg_cairo_new_drawing_ctx (cr, handle);
    if (!draw)
        goto bail;

    g_assert (node != NULL);
    rsvg_drawing_ctx_add_node_and_ancestors_to_stack (draw, node);

    rsvg_drawing_ctx_draw_node_from_stack (draw, handle->priv->treebase, 0);
    bbox = RSVG_CAIRO_RENDER (draw->render)->bbox;

    rsvg_drawing_ctx_free (draw);

    position_data->x = bbox.rect.x;
    position_data->y = bbox.rect.y;
    dimension_data.width = bbox.rect.width;
    dimension_data.height = bbox.rect.height;

    dimension_data.em = dimension_data.width;
    dimension_data.ex = dimension_data.height;

    if (handle->priv->size_func)
        (*handle->priv->size_func) (&dimension_data.width, &dimension_data.height,
                                    handle->priv->user_data);

    ret = TRUE;

bail:
    if (cr)
        cairo_destroy (cr);
    if (target)
        cairo_surface_destroy (target);

    return ret;
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
    g_return_val_if_fail (handle, FALSE);

    if (G_UNLIKELY (!id || !id[0]))
      return FALSE;

    return rsvg_defs_lookup (handle->priv->defs, id) != NULL;
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

    g_return_val_if_fail (handle != NULL, NULL);

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
    g_return_if_fail (handle != NULL);

    if (dpi_x <= 0.)
        handle->priv->dpi_x = rsvg_internal_dpi_x;
    else
        handle->priv->dpi_x = dpi_x;

    if (dpi_y <= 0.)
        handle->priv->dpi_y = rsvg_internal_dpi_y;
    else
        handle->priv->dpi_y = dpi_y;
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
    g_return_if_fail (handle != NULL);

    if (handle->priv->user_data_destroy)
        (*handle->priv->user_data_destroy) (handle->priv->user_data);

    handle->priv->size_func = size_func;
    handle->priv->user_data = user_data;
    handle->priv->user_data_destroy = user_data_destroy;
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
}
