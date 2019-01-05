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
#include <glib/gprintf.h>
#include "rsvg-private.h"

/* Defined in rsvg_internals/src/load.rs */
typedef struct RsvgLoad RsvgLoad;

/* Defined in rsvg_internals/src/handle.rs */
typedef struct RsvgHandleRust RsvgHandleRust;

/* Implemented in rsvg_internals/src/xml.rs */
typedef struct RsvgXmlState RsvgXmlState;

/* Implemented in rsvg_internals/src/xml.rs */
extern void rsvg_xml_state_error(RsvgXmlState *xml, const char *msg);

G_GNUC_INTERNAL
RsvgHandleRust *rsvg_handle_get_rust (RsvgHandle *handle);

/* Implemented in rsvg_internals/src/handle.rs */
extern RsvgHandleRust *rsvg_handle_rust_new (void);
extern void rsvg_handle_rust_free (RsvgHandleRust *raw_handle);
extern double rsvg_handle_rust_get_dpi_x (RsvgHandleRust *raw_handle);
extern double rsvg_handle_rust_get_dpi_y (RsvgHandleRust *raw_handle);
extern void rsvg_handle_rust_set_dpi_x (RsvgHandleRust *raw_handle, double dpi_x);
extern void rsvg_handle_rust_set_dpi_y (RsvgHandleRust *raw_handle, double dpi_y);
extern void rsvg_handle_rust_set_base_url (RsvgHandleRust *raw_handle, const char *uri);
extern GFile *rsvg_handle_rust_get_base_gfile (RsvgHandleRust *raw_handle);
extern guint rsvg_handle_rust_get_flags (RsvgHandleRust *raw_handle);
extern void rsvg_handle_rust_set_flags (RsvgHandleRust *raw_handle, guint flags);
extern guint rsvg_handle_rust_set_testing (RsvgHandleRust *raw_handle, gboolean testing);
extern gboolean rsvg_handle_rust_is_at_start_for_setting_base_file (RsvgHandle *handle);
extern gboolean rsvg_handle_rust_read_stream_sync (RsvgHandle *handle,
                                                   GInputStream *stream,
                                                   GCancellable *cancellable,
                                                   GError **error);
extern void rsvg_handle_rust_write (RsvgHandle *handle, const guchar *buf, gsize count);
extern gboolean rsvg_handle_rust_close (RsvgHandle *handle, GError **error);
extern gboolean rsvg_handle_rust_get_geometry_sub (RsvgHandle *handle,
                                                   RsvgRectangle *out_ink_rect,
                                                   RsvgRectangle *out_logical_rect,
                                                   const char *id);
extern gboolean rsvg_handle_rust_has_sub (RsvgHandle *handle, const char *id);
extern gboolean rsvg_handle_rust_render_cairo_sub (RsvgHandle *handle,
                                                   cairo_t *cr,
                                                   const char *id);
extern GdkPixbuf *rsvg_handle_rust_get_pixbuf_sub (RsvgHandle *handle, const char *id);
extern void rsvg_handle_rust_get_dimensions (RsvgHandle *handle,
                                             RsvgDimensionData *dimension_data);
extern gboolean rsvg_handle_rust_get_dimensions_sub (RsvgHandle *handle,
                                                     RsvgDimensionData *dimension_data,
                                                     const char *id);
extern gboolean rsvg_handle_rust_get_position_sub (RsvgHandle *handle,
                                                   RsvgPositionData *dimension_data,
                                                   const char *id);

typedef struct {
    RsvgSizeFunc func;
    gpointer data;
    GDestroyNotify data_destroy;
} RsvgSizeClosure;

static RsvgSizeClosure *
rsvg_size_closure_new (RsvgSizeFunc func, gpointer data, GDestroyNotify data_destroy)
{
    RsvgSizeClosure *closure;

    closure = g_new0(RsvgSizeClosure, 1);
    closure->func = func;
    closure->data = data;
    closure->data_destroy = data_destroy;

    return closure;
}

G_GNUC_INTERNAL
void rsvg_size_closure_free (RsvgSizeClosure *closure);

void
rsvg_size_closure_free (RsvgSizeClosure *closure)
{
    if (closure && closure->data && closure->data_destroy) {
        (*closure->data_destroy) (closure->data);
    }

    g_free (closure);
}

G_GNUC_INTERNAL
void rsvg_size_closure_call (RsvgSizeClosure *closure, int *width, int *height);

void
rsvg_size_closure_call (RsvgSizeClosure *closure, int *width, int *height)
{
    if (closure && closure->func) {
        (*closure->func) (width, height, closure->data);
    }
}

extern void rsvg_handle_rust_set_size_closure (RsvgHandleRust *raw_handle, RsvgSizeClosure *closure);

struct RsvgHandlePrivate {
    gchar *base_uri; // Keep this here; since rsvg_handle_get_base_uri() returns a const char *

    RsvgHandleRust *rust_handle;
};

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
    self->priv->rust_handle = rsvg_handle_rust_new();
}

static void
rsvg_handle_dispose (GObject *instance)
{
    RsvgHandle *self = (RsvgHandle *) instance;

    g_clear_pointer (&self->priv->base_uri, g_free);
    g_clear_pointer (&self->priv->rust_handle, rsvg_handle_rust_free);

    G_OBJECT_CLASS (rsvg_handle_parent_class)->dispose (instance);
}

static void
rsvg_handle_set_property (GObject * instance, guint prop_id, GValue const *value, GParamSpec * pspec)
{
    RsvgHandle *self = RSVG_HANDLE (instance);

    switch (prop_id) {
    case PROP_FLAGS:
        rsvg_handle_rust_set_flags (self->priv->rust_handle, g_value_get_flags (value));
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
        g_value_set_flags (value, rsvg_handle_rust_get_flags (self->priv->rust_handle));
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
 * handle can be used to load an image.
 *
 * The preferred way of loading SVG data into the returned #RsvgHandle is with
 * rsvg_handle_read_stream_sync().
 *
 * The deprecated way of loading SVG data is with rsvg_handle_write() and
 * rsvg_handle_close().
 *
 * After loading the #RsvgHandl with data, you can render it using Cairo or get
 * a GdkPixbuf from it. When finished, free with g_object_unref(). No more than
 * one image can be loaded with one handle.
 *
 * Returns: A new #RsvgHandle
 **/
RsvgHandle *
rsvg_handle_new (void)
{
    return RSVG_HANDLE (g_object_new (RSVG_TYPE_HANDLE, NULL));
}

/**
 * rsvg_handle_new_from_data:
 * @data: (array length=data_len): The SVG data
 * @data_len: The length of @data, in bytes
 * @error: return location for errors
 *
 * Loads the SVG specified by @data.  Note that this function creates an
 * #RsvgHandle without a base file, and without any special flags.  If you
 * need these, use rsvg_handle_new_from_stream_sync() instead by creating
 * a #GMemoryInputStream from your data.
 *
 * Returns: A #RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_data (const guint8 *data, gsize data_len, GError **error)
{
    GInputStream *stream;
    RsvgHandle *handle;

    g_return_val_if_fail (data != NULL, NULL);
    g_return_val_if_fail (data_len <= G_MAXSSIZE, NULL);
    g_return_val_if_fail (error == NULL || *error == NULL, NULL);

    stream = g_memory_input_stream_new_from_data (data, data_len, NULL);
    handle = rsvg_handle_new_from_stream_sync (stream, NULL, RSVG_HANDLE_FLAGS_NONE, NULL, error);
    g_object_unref (stream);

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
 * Loads the next @count bytes of the image.
 *
 * Returns: This function always returns %TRUE, and does not set the @error.
 *
 * Deprecated: 2.46.  Use rsvg_handle_read_stream_sync() or the constructor
 * functions rsvg_handle_new_from_gfile_sync() or rsvg_handle_new_from_stream_sync().
 *
 * Notes: This function will accumlate data from the @buf in memory until
 * rsvg_handle_close() gets called.  To avoid a big temporary buffer, use the
 * funtions listed before, which take a #GFile or a #GInputStream.
 **/
gboolean
rsvg_handle_write (RsvgHandle *handle, const guchar *buf, gsize count, GError **error)
{
    rsvg_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE, error);
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);
    g_return_val_if_fail ((buf != NULL && count != 0) || (count == 0), FALSE);

    rsvg_handle_rust_write (handle, buf, count);
    return TRUE;
}

/**
 * rsvg_handle_close:
 * @handle: a #RsvgHandle
 * @error: (allow-none): a location to store a #GError, or %NULL
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return %TRUE if the loader closed successfully and the SVG data was parsed
 * correctly.  Note that @handle isn't freed until @g_object_unref is called.
 *
 * Returns: %TRUE on success, or %FALSE on error.
 *
 * Deprecated: 2.46.  Use rsvg_handle_read_stream_sync() or the constructor
 * functions rsvg_handle_new_from_gfile_sync() or rsvg_handle_new_from_stream_sync().
 **/
gboolean
rsvg_handle_close (RsvgHandle *handle, GError **error)
{
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);
    rsvg_return_val_if_fail (handle, FALSE, error);

    return rsvg_handle_rust_close(handle, error);
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
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (G_IS_INPUT_STREAM (stream), FALSE);
    g_return_val_if_fail (cancellable == NULL || G_IS_CANCELLABLE (cancellable), FALSE);
    g_return_val_if_fail (error == NULL || *error == NULL, FALSE);

    return rsvg_handle_rust_read_stream_sync (handle,
                                              stream,
                                              cancellable,
                                              error);
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

    if (!rsvg_handle_rust_is_at_start_for_setting_base_file (handle)) {
        return;
    }

    if (base_uri == NULL) {
        return;
    }

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

    if (!rsvg_handle_rust_is_at_start_for_setting_base_file (handle)) {
        return;
    }

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

RsvgHandleRust *
rsvg_handle_get_rust (RsvgHandle *handle)
{
    return handle->priv->rust_handle;
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
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (cr != NULL, FALSE);

    return rsvg_handle_rust_render_cairo_sub (handle, cr, id);
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

    rsvg_handle_rust_get_dimensions (handle, dimension_data);
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
 * Deprecated: 2.46.  Use rsvg_handle_get_geometry_sub() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_dimensions_sub (RsvgHandle * handle, RsvgDimensionData * dimension_data, const char *id)
{
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (dimension_data, FALSE);

    return rsvg_handle_rust_get_dimensions_sub (handle, dimension_data, id);
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
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);

    return rsvg_handle_rust_get_geometry_sub(handle, ink_rect, logical_rect, id);
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
 * Deprecated: 2.46.  Use rsvg_handle_get_geometry_sub() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_position_sub (RsvgHandle * handle, RsvgPositionData * position_data, const char *id)
{
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);
    g_return_val_if_fail (position_data != NULL, FALSE);

    return rsvg_handle_rust_get_position_sub (handle, position_data, id);
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
rsvg_handle_has_sub (RsvgHandle *handle,
                     const char *id)
{
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), FALSE);

    return rsvg_handle_rust_has_sub (handle, id);
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
    g_return_val_if_fail (RSVG_IS_HANDLE (handle), NULL);

    return rsvg_handle_rust_get_pixbuf_sub (handle, id);
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
                               gpointer user_data,
                               GDestroyNotify user_data_destroy)
{
    g_return_if_fail (RSVG_IS_HANDLE (handle));

    rsvg_handle_rust_set_size_closure (handle->priv->rust_handle,
                                       rsvg_size_closure_new (size_func, user_data, user_data_destroy));
}

/**
 * rsvg_handle_internal_set_testing:
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

    rsvg_handle_rust_set_testing (handle->priv->rust_handle, testing);
}

/* This one is defined in the C code, because the prototype has varargs
 * and we can't handle those from Rust :(
 */
G_GNUC_INTERNAL void rsvg_sax_error_cb (void *data, const char *msg, ...);

void
rsvg_sax_error_cb (void *data, const char *msg, ...)
{
    RsvgXmlState *xml = data;
    va_list args;
    char *buf;

    va_start (args, msg);
    g_vasprintf (&buf, msg, args);
    va_end (args);

    rsvg_xml_state_error (xml, buf);

    g_free (buf);
}
