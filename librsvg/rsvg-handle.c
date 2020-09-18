/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-gobject.c: GObject support.

   Copyright (C) 2006 Robert Staudinger <robert.staudinger@gmail.com>

   This library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public
   License as published by the Free Software Foundation; either
   version 2.1 of the License, or (at your option) any later version.

   This library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with this library; if not, write to the Free Software
   Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
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
 * Librsvg can load SVG images and render them to Cairo surfaces,
 * using a mixture of SVG's [static mode] and [secure static mode].
 * Librsvg does not do animation nor scripting, and can load
 * references to external data only in some situations; see below.
 *
 * Librsvg supports reading <ulink
 * url="https://www.w3.org/TR/SVG11/">SVG 1.1</ulink> data, and is
 * gradually adding support for features in <ulink
 * url="https://www.w3.org/TR/SVG2/">SVG 2</ulink>.  Librsvg also supports
 * SVGZ files, which are just an SVG stream compressed with the GZIP algorithm.
 *
 * # The "base file" and resolving references to external files
 *
 * When you load an SVG, librsvg needs to know the location of the "base file"
 * for it.  This is so that librsvg can determine the location of referenced
 * entities.  For example, say you have an SVG in <filename>/foo/bar/foo.svg</filename>
 * and that it has an image element like this:
 *
 * |[
 * <image href="resources/foo.png" .../>
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
 * only try to load referenced files (from SVG's
 * <literal>&lt;image&gt;</literal> element, for example, or from content
 * included through XML entities) if those files are in <filename>/foo/bar/<!--
 * -->*</filename> or in <filename>/foo/bar/<!-- -->*<!-- -->/.../<!--
 * -->*</filename>.  This is so that malicious SVG files cannot include files
 * that are in a directory above.
 *
 * The full set of rules for deciding which URLs may be loaded is as follows;
 * they are applied in order.  A referenced URL will not be loaded as soon as
 * one of these rules fails:
 *
 * <orderedlist>
 *   <listitem>
 *     All <literal>data:</literal> URLs may be loaded.  These are sometimes used
 *     to include raster image data, encoded as base-64, directly in an SVG file.
 *   </listitem>
 *
 *   <listitem>
 *     All other URL schemes in references require a base URL.  For
 *     example, this means that if you load an SVG with
 *     rsvg_handle_new_from_data() without calling rsvg_handle_set_base_uri(),
 *     then any referenced files will not be allowed (e.g. raster images to be
 *     loaded from other files will not work).
 *   </listitem>
 *
 *   <listitem>
 *     If referenced URLs are absolute, rather than relative, then they must
 *     have the same scheme as the base URL.  For example, if the base URL has a
 *     "<literal>file</literal>" scheme, then all URL references inside the SVG must
 *     also have the "<literal>file</literal>" scheme, or be relative references which
 *     will be resolved against the base URL.
 *   </listitem>
 *
 *   <listitem>
 *     If referenced URLs have a "<literal>resource</literal>" scheme, that is,
 *     if they are included into your binary program with GLib's resource
 *     mechanism, they are allowed to be loaded (provided that the base URL is
 *     also a "<literal>resource</literal>", per the previous rule).
 *   </listitem>
 *
 *   <listitem>
 *     Otherwise, non-<literal>file</literal> schemes are not allowed.  For
 *     example, librsvg will not load <literal>http</literal> resources, to keep
 *     malicious SVG data from "phoning home".
 *   </listitem>
 *
 *   <listitem>
 *     A relative URL must resolve to the same directory as the base URL, or to
 *     one of its subdirectories.  Librsvg will canonicalize filenames, by
 *     removing ".." path components and resolving symbolic links, to decide whether
 *     files meet these conditions.
 *   </listitem>
 * </orderedlist>
 *
 * # Loading an SVG with GIO
 *
 * This is the easiest and most resource-efficient way of loading SVG data into
 * an #RsvgHandle.
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
 * If you already have SVG data in a byte buffer in memory, you can create a
 * memory input stream with g_memory_input_stream_new_from_data() and feed that
 * to rsvg_handle_new_from_stream_sync().
 *
 * Note that in this case, it is important that you specify the base_file for
 * the in-memory SVG data.  Librsvg uses the base_file to resolve links to
 * external content, like raster images.
 *
 * # Loading an SVG without GIO
 *
 * You can load an RsvgHandle from a simple filename or URI with
 * rsvg_handle_new_from_file().  Note that this is a blocking operation; there
 * is no way to cancel it if loading a remote URI takes a long time.  Also, note that
 * this method does not let you specify #RsvgHandleFlags.
 *
 * Otherwise, loading an SVG without GIO is not recommended, since librsvg will
 * need to buffer your entire data internally before actually being able to
 * parse it.  The deprecated way of doing this is by creating a handle with
 * rsvg_handle_new() or rsvg_handle_new_with_flags(), and then using
 * rsvg_handle_write() and rsvg_handle_close() to feed the handle with SVG data.
 * Still, please try to use the GIO stream functions instead.
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
 *
 * # API ordering
 *
 * Due to the way the librsvg API evolved over time, an #RsvgHandle object is available
 * for use as soon as it is constructed.  However, not all of its methods can be
 * called at any time.  For example, an #RsvgHandle just constructed with rsvg_handle_new()
 * is not loaded yet, and it does not make sense to call rsvg_handle_get_dimensions() on it
 * just at that point.
 *
 * The documentation for the available methods in #RsvgHandle may mention that a particular
 * method is only callable on a "fully loaded handle".  This means either:
 *
 * <itemizedlist>
 *   <listitem>
 *     The handle was loaded with rsvg_handle_write() and rsvg_handle_close(), and
 *     those functions returned no errors.
 *   </listitem>
 *   <listitem>
 *     The handle was loaded with rsvg_handle_read_stream_sync() and that function
 *     returned no errors.
 *   </listitem>
 * </itemizedlist>
 *
 * Before librsvg 2.46, the library did not fully verify that a handle was in a
 * fully loaded state for the methods that require it.  To preserve
 * compatibility with old code which inadvertently called the API without
 * checking for errors, or which called some methods outside of the expected
 * order, librsvg will just emit a g_critical() message in those cases.
 *
 * New methods introduced in librsvg 2.46 and later will check for the correct
 * ordering, and panic if they are called out of order.  This will abort
 * the program as if it had a failed assertion.
 */

/***** Begin documentation for RsvgHandle properties *****/

/**
 * RsvgHandle:flags:
 *
 * Flags from #RsvgHandleFlags.
 *
 * Since: 2.36
 */

/**
 * RsvgHandle::dpi-x:
 *
 * Horizontal resolution in dots per inch.
 */

/**
 * RsvgHandle::dpi-y:
 *
 * Horizontal resolution in dots per inch.
 */

/**
 * RsvgHandle::base-uri:
 *
 * Base URI, to be used to resolve relative references for resources.  See the section
 */

/**
 * RsvgHandle:width:
 *
 * Width, in pixels, of the rendered SVG after calling the size callback
 * as specified by rsvg_handle_set_size_callback().
 *
 * Deprecated: 2.46.  For historical reasons, this property is of integer type,
 * which cannot give the exact size of SVG images that are not pixel-aligned.
 * Moreover, reading each of the size properties causes the size of the SVG to
 * be recomputed, so reading both the <literal>width</literal> and
 * <literal>height</literal> properties will cause two such computations.
 * Please use rsvg_handle_get_intrinsic_dimensions() instead.
 */

/**
 * RsvgHandle:height:
 *
 * Height, in pixels, of the rendered SVG after calling the size callback
 * as specified by rsvg_handle_set_size_callback().
 *
 * Deprecated: 2.46.  For historical reasons, this property is of integer type,
 * which cannot give the exact size of SVG images that are not pixel-aligned.
 * Moreover, reading each of the size properties causes the size of the SVG to
 * be recomputed, so reading both the <literal>width</literal> and
 * <literal>height</literal> properties will cause two such computations.
 * Please use rsvg_handle_get_intrinsic_dimensions() instead.
 */

/**
 * RsvgHandle:em:
 *
 * Exact width, in pixels, of the rendered SVG before calling the size callback
 * as specified by rsvg_handle_set_size_callback().
 *
 * Deprecated: 2.46.  Reading each of the size properties causes the size of the
 * SVG to be recomputed, so reading both the <literal>em</literal> and
 * <literal>ex</literal> properties will cause two such computations.  Please
 * use rsvg_handle_get_intrinsic_dimensions() instead.
 */

/**
 * RsvgHandle:ex:
 *
 * Exact height, in pixels, of the rendered SVG before calling the size callback
 * as specified by rsvg_handle_set_size_callback().
 *
 * Deprecated: 2.46.  Reading each of the size properties causes the size of the
 * SVG to be recomputed, so reading both the <literal>em</literal> and
 * <literal>ex</literal> properties will cause two such computations.  Please
 * use rsvg_handle_get_intrinsic_dimensions() instead.
 */

/**
 * RsvgHandle:title:
 *
 * SVG's title.
 *
 * Deprecated: 2.36.  Reading this property always returns #NULL.
 */

/**
 * RsvgHandle:desc:
 *
 * SVG's description.
 *
 * Deprecated: 2.36.  Reading this property always returns #NULL.
 */

/**
 * RsvgHandle:metadata:
 *
 * SVG's metadata
 *
 * Deprecated: 2.36.  Reading this property always returns #NULL.
 */

/***** End documentation for RsvgHandle properties *****/

#include "config.h"
#define _GNU_SOURCE 1

#include <string.h>
#include <limits.h>
#include <stdlib.h>
#include <glib/gprintf.h>
#include <glib/gi18n-lib.h>

#include "rsvg.h"

/* Implemented in rsvg_internals/src/handle.rs */
extern void rsvg_rust_handle_set_dpi (RsvgHandle *handle, double dpi);
extern void rsvg_rust_handle_set_dpi_x_y (RsvgHandle *handle, double dpi_x, double dpi_y);
extern void rsvg_rust_handle_set_base_url (RsvgHandle *handle, const char *uri);
extern void rsvg_rust_handle_set_base_gfile (RsvgHandle *handle, GFile *file);
extern const char *rsvg_rust_handle_get_base_url (RsvgHandle *handle);
extern guint rsvg_rust_handle_set_testing (RsvgHandle *handle, gboolean testing);
extern gboolean rsvg_rust_handle_read_stream_sync (RsvgHandle *handle,
                                                   GInputStream *stream,
                                                   GCancellable *cancellable,
                                                   GError **error);
extern gboolean rsvg_rust_handle_write (RsvgHandle *handle, const guchar *buf, gsize count, GError **error);
extern gboolean rsvg_rust_handle_close (RsvgHandle *handle, GError **error);
extern gboolean rsvg_rust_handle_has_sub (RsvgHandle *handle, const char *id);
extern gboolean rsvg_rust_handle_render_cairo (RsvgHandle *handle, cairo_t *cr);
extern gboolean rsvg_rust_handle_render_cairo_sub (RsvgHandle *handle,
                                                   cairo_t *cr,
                                                   const char *id);
extern GdkPixbuf *rsvg_rust_handle_get_pixbuf (RsvgHandle *handle);
extern GdkPixbuf *rsvg_rust_handle_get_pixbuf_sub (RsvgHandle *handle, const char *id);
extern void rsvg_rust_handle_get_dimensions (RsvgHandle *handle,
                                             RsvgDimensionData *dimension_data);
extern gboolean rsvg_rust_handle_get_dimensions_sub (RsvgHandle *handle,
                                                     RsvgDimensionData *dimension_data,
                                                     const char *id);
extern gboolean rsvg_rust_handle_get_position_sub (RsvgHandle *handle,
                                                   RsvgPositionData *dimension_data,
                                                   const char *id);
extern void rsvg_rust_handle_set_size_callback (RsvgHandle *handle,
                                                RsvgSizeFunc size_func,
                                                gpointer user_data,
                                                GDestroyNotify destroy_notify);
extern RsvgHandle *rsvg_rust_handle_new (void);
extern RsvgHandle *rsvg_rust_handle_new_with_flags (RsvgHandleFlags flags);
extern RsvgHandle *rsvg_rust_handle_new_from_file (const char *filename,
                                                   GError **error);
extern RsvgHandle *rsvg_rust_handle_new_from_gfile_sync (GFile *file,
                                                         RsvgHandleFlags flags,
                                                         GCancellable *cancellable,
                                                         GError **error);
extern RsvgHandle *rsvg_rust_handle_new_from_stream_sync (GInputStream *input_stream,
                                                          GFile *base_file,
                                                          RsvgHandleFlags flags,
                                                          GCancellable *cancellable,
                                                          GError **error);
extern RsvgHandle *rsvg_rust_handle_new_from_data (const guint8 *data,
                                                   gsize data_len,
                                                   GError **error);
extern void rsvg_rust_handle_free (RsvgHandle *handle);
extern gboolean rsvg_rust_handle_set_stylesheet (RsvgHandle  *handle,
                                                 const char  *css,
                                                 gsize        css_len,
                                                 GError     **error);

extern void rsvg_rust_handle_get_intrinsic_dimensions (RsvgHandle *handle,
                                                       gboolean   *out_has_width,
                                                       RsvgLength *out_width,
                                                       gboolean   *out_has_height,
                                                       RsvgLength *out_height,
                                                       gboolean   *out_has_viewbox,
                                                       RsvgRectangle *out_viewbox);
extern gboolean rsvg_rust_handle_render_document (RsvgHandle           *handle,
                                                  cairo_t              *cr,
                                                  const RsvgRectangle  *viewport,
                                                  GError              **error);
extern gboolean rsvg_rust_handle_get_geometry_for_layer (RsvgHandle     *handle,
                                                         const char     *id,
                                                         const RsvgRectangle *viewport,
                                                         RsvgRectangle  *out_ink_rect,
                                                         RsvgRectangle  *out_logical_rect,
                                                         GError        **error);
extern gboolean rsvg_rust_handle_render_layer (RsvgHandle           *handle,
                                               cairo_t              *cr,
                                               const char           *id,
                                               const RsvgRectangle  *viewport,
                                               GError              **error);
extern gboolean rsvg_rust_handle_get_geometry_for_element (RsvgHandle     *handle,
                                                           const char     *id,
                                                           RsvgRectangle  *out_ink_rect,
                                                           RsvgRectangle  *out_logical_rect,
                                                           GError        **error);
extern gboolean rsvg_rust_handle_render_element (RsvgHandle           *handle,
                                                 cairo_t              *cr,
                                                 const char           *id,
                                                 const RsvgRectangle  *element_viewport,
                                                 GError              **error);
extern const char *rsvg_rust_handle_get_desc (RsvgHandle *handle);
extern const char *rsvg_rust_handle_get_metadata (RsvgHandle *handle);
extern const char *rsvg_rust_handle_get_title (RsvgHandle *handle);


/* Implemented in librsvg/c_api.rs */
extern GType rsvg_rust_error_get_type (void);
extern GType rsvg_rust_handle_get_type (void);
extern GType rsvg_rust_handle_flags_get_type (void);

GType
rsvg_handle_get_type (void)
{
    return rsvg_rust_handle_get_type ();
}

/**
 * rsvg_handle_free:
 * @handle: An #RsvgHandle
 *
 * Frees @handle.
 * Deprecated: Use g_object_unref() instead.
 **/
void
rsvg_handle_free (RsvgHandle *handle)
{
    rsvg_rust_handle_free (handle);
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
 * rsvg_handle_close(); note that these require buffering the entire file
 * internally, and for this reason it is better to use the stream functions:
 * rsvg_handle_new_from_stream_sync(), rsvg_handle_read_stream_sync(), or
 * rsvg_handle_new_from_gfile_sync().
 *
 * After loading the #RsvgHandle with data, you can render it using Cairo or get
 * a GdkPixbuf from it. When finished, free the handle with g_object_unref(). No
 * more than one image can be loaded with one handle.
 *
 * Note that this function creates an #RsvgHandle with no flags set.  If you
 * require any of #RsvgHandleFlags to be set, use any of
 * rsvg_handle_new_with_flags(), rsvg_handle_new_from_stream_sync(), or
 * rsvg_handle_new_from_gfile_sync().
 *
 * Returns: A new #RsvgHandle with no flags set.
 **/
RsvgHandle *
rsvg_handle_new (void)
{
    return rsvg_rust_handle_new();
}

/**
 * rsvg_handle_new_from_data:
 * @data: (array length=data_len): The SVG data
 * @data_len: The length of @data, in bytes
 * @error: (optional): return location for errors
 *
 * Loads the SVG specified by @data.  Note that this function creates an
 * #RsvgHandle without a base URL, and without any #RsvgHandleFlags.  If you
 * need these, use rsvg_handle_new_from_stream_sync() instead by creating
 * a #GMemoryInputStream from your data.
 *
 * Returns: A #RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_data (const guint8 *data, gsize data_len, GError **error)
{
    return rsvg_rust_handle_new_from_data (data, data_len, error);
}

/**
 * rsvg_handle_new_from_file:
 * @filename: The file name to load, or a URI.
 * @error: (optional): return location for errors
 *
 * Loads the SVG specified by @file_name.  Note that this function, like
 * rsvg_handle_new(), does not specify any loading flags for the resulting
 * handle.  If you require the use of #RsvgHandleFlags, use
 * rsvg_handle_new_from_gfile_sync().
 *
 * Returns: A #RsvgHandle or %NULL if an error occurs.
 * Since: 2.14
 */
RsvgHandle *
rsvg_handle_new_from_file (const gchar *filename, GError **error)
{
    return rsvg_rust_handle_new_from_file (filename, error);
}

/**
 * rsvg_handle_new_with_flags:
 * @flags: flags from #RsvgHandleFlags
 *
 * Creates a new #RsvgHandle with flags @flags.  After calling this function,
 * you can feed the resulting handle with SVG data by using
 * rsvg_handle_read_stream_sync().
 *
 * Returns: (transfer full): a new #RsvgHandle
 *
 * Since: 2.36
 **/
RsvgHandle *
rsvg_handle_new_with_flags (RsvgHandleFlags flags)
{
    return rsvg_rust_handle_new_with_flags (flags);
}

/**
 * rsvg_handle_new_from_gfile_sync:
 * @file: a #GFile
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (nullable): a #GCancellable, or %NULL
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @file.
 *
 * This function sets the "base file" of the handle to be @file itself, so SVG
 * elements like <literal>&lt;image&gt;</literal> which reference external
 * resources will be resolved relative to the location of @file.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned in @error.
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
    return rsvg_rust_handle_new_from_gfile_sync (file, flags, cancellable, error);
}

/**
 * rsvg_handle_new_from_stream_sync:
 * @input_stream: a #GInputStream
 * @base_file: (nullable): a #GFile, or %NULL
 * @flags: flags from #RsvgHandleFlags
 * @cancellable: (nullable): a #GCancellable, or %NULL
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Creates a new #RsvgHandle for @stream.
 *
 * This function sets the "base file" of the handle to be @base_file if
 * provided.  SVG elements like <literal>&lt;image&gt;</literal> which reference
 * external resources will be resolved relative to the location of @base_file.
 *
 * If @cancellable is not %NULL, then the operation can be cancelled by
 * triggering the cancellable object from another thread. If the
 * operation was cancelled, the error %G_IO_ERROR_CANCELLED will be
 * returned in @error.
 *
 * Returns: a new #RsvgHandle on success, or %NULL with @error filled in
 *
 * Since: 2.32
 */
RsvgHandle *
rsvg_handle_new_from_stream_sync (GInputStream    *input_stream,
                                  GFile           *base_file,
                                  RsvgHandleFlags  flags,
                                  GCancellable    *cancellable,
                                  GError         **error)
{
    return rsvg_rust_handle_new_from_stream_sync (input_stream,
                                                  base_file,
                                                  flags,
                                                  cancellable,
                                                  error);
}

/**
 * rsvg_handle_write:
 * @handle: an #RsvgHandle
 * @buf: (array length=count) (element-type guchar): pointer to svg data
 * @count: length of the @buf buffer in bytes
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Loads the next @count bytes of the image.
 *
 * Before calling this function for the first time, you may need to call
 * rsvg_handle_set_base_uri() or rsvg_handle_set_base_gfile() to set the "base
 * file" for resolving references to external resources.  SVG elements like
 * <literal>&lt;image&gt;</literal> which reference external resources will be
 * resolved relative to the location you specify with those functions.
 *
 * Returns: %TRUE on success, or %FALSE on error.
 *
 * Deprecated: 2.46.  Use rsvg_handle_read_stream_sync() or the constructor
 * functions rsvg_handle_new_from_gfile_sync() or
 * rsvg_handle_new_from_stream_sync().  This function is deprecated because it
 * will accumulate data from the @buf in memory until rsvg_handle_close() gets
 * called.  To avoid a big temporary buffer, use the suggested functions, which
 * take a #GFile or a #GInputStream and do not require a temporary buffer.
 **/
gboolean
rsvg_handle_write (RsvgHandle *handle, const guchar *buf, gsize count, GError **error)
{
    return rsvg_rust_handle_write (handle, buf, count, error);
}

/**
 * rsvg_handle_close:
 * @handle: a #RsvgHandle
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Closes @handle, to indicate that loading the image is complete.  This will
 * return %TRUE if the loader closed successfully and the SVG data was parsed
 * correctly.  Note that @handle isn't freed until @g_object_unref is called.
 *
 * Returns: %TRUE on success, or %FALSE on error.
 *
 * Deprecated: 2.46.  Use rsvg_handle_read_stream_sync() or the constructor
 * functions rsvg_handle_new_from_gfile_sync() or
 * rsvg_handle_new_from_stream_sync().  See the deprecation notes for
 * rsvg_handle_write() for more information.
 **/
gboolean
rsvg_handle_close (RsvgHandle *handle, GError **error)
{
    return rsvg_rust_handle_close(handle, error);
}

/**
 * rsvg_handle_read_stream_sync:
 * @handle: a #RsvgHandle
 * @stream: a #GInputStream
 * @cancellable: (nullable): a #GCancellable, or %NULL
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Reads @stream and writes the data from it to @handle.
 *
 * Before calling this function, you may need to call rsvg_handle_set_base_uri()
 * or rsvg_handle_set_base_gfile() to set the "base file" for resolving
 * references to external resources.  SVG elements like
 * <literal>&lt;image&gt;</literal> which reference external resources will be
 * resolved relative to the location you specify with those functions.
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
    return rsvg_rust_handle_read_stream_sync (handle,
                                              stream,
                                              cancellable,
                                              error);
}

/**
 * rsvg_handle_set_base_uri:
 * @handle: A #RsvgHandle
 * @base_uri: The base uri
 *
 * Set the base URI for this SVG.
 *
 * Note: This function may only be called before rsvg_handle_write() or
 * rsvg_handle_read_stream_sync() have been called.
 *
 * Since: 2.9
 */
void
rsvg_handle_set_base_uri (RsvgHandle *handle, const char *base_uri)
{
    rsvg_rust_handle_set_base_url (handle, base_uri);
}

/**
 * rsvg_handle_set_base_gfile:
 * @handle: a #RsvgHandle
 * @base_file: a #GFile
 *
 * Set the base URI for @handle from @file.
 *
 * Note: This function may only be called before rsvg_handle_write() or
 * rsvg_handle_read_stream_sync() have been called.
 *
 * Since: 2.32
 */
void
rsvg_handle_set_base_gfile (RsvgHandle *handle,
                            GFile      *base_file)
{
    rsvg_rust_handle_set_base_gfile (handle, base_file);
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
rsvg_handle_get_base_uri (RsvgHandle *handle)
{
    return rsvg_rust_handle_get_base_url (handle);
}

/**
 * rsvg_handle_set_stylesheet:
 * @handle: A #RsvgHandle.
 * @css: (array length=css_len): String with CSS data; must be valid UTF-8.
 * @css_len: Length of the @css data in bytes.
 * @error: (optional): return location for errors.
 *
 * Sets a CSS stylesheet to use for an SVG document.
 *
 * The @css_len argument is mandatory; this function will not compute the length
 * of the @css string.  This is because a provided stylesheet, which the calling
 * program could read from a file, can have nul characters in it.
 *
 * During the CSS cascade, the specified stylesheet will be used with a "User"
 * <ulink
 * url="https://drafts.csswg.org/css-cascade-3/#cascading-origins">origin</ulink>.
 *
 * Note that `@import` rules will not be resolved, except for `data:` URLs.
 *
 * Since: 2.48
 */
gboolean
rsvg_handle_set_stylesheet (RsvgHandle   *handle,
                            const guint8 *css,
                            gsize         css_len,
                            GError      **error)
{
    return rsvg_rust_handle_set_stylesheet (handle, css, css_len, error);
}

/**
 * rsvg_handle_get_metadata:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns #NULL.
 *
 * Since: 2.9
 *
 * Deprecated: 2.36.  Librsvg does not read the metadata/desc/title elements;
 * this function always returns #NULL.
 */
const char *
rsvg_handle_get_metadata (RsvgHandle * handle)
{
    return rsvg_rust_handle_get_metadata (handle);
}

/**
 * rsvg_handle_get_title:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns NULL.
 *
 * Since: 2.4
 *
 * Deprecated: 2.36.  Librsvg does not read the metadata/desc/title elements;
 * this function always returns #NULL.
 */
const char *
rsvg_handle_get_title (RsvgHandle *handle)
{
    return rsvg_rust_handle_get_title (handle);
}

/**
 * rsvg_handle_get_desc:
 * @handle: An #RsvgHandle
 *
 * Returns: (nullable): This function always returns NULL.
 *
 * Since: 2.4
 *
 * Deprecated: 2.36.  Librsvg does not read the metadata/desc/title elements;
 * this function always returns #NULL.
 */
const char *
rsvg_handle_get_desc (RsvgHandle *handle)
{
    return rsvg_rust_handle_get_desc (handle);
}

/**
 * rsvg_handle_render_cairo_sub:
 * @handle: A #RsvgHandle
 * @cr: A Cairo context
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to render the whole SVG.
 *
 * Draws a subset of a loaded SVG handle to a Cairo context.  Drawing will occur with
 * respect to the @cr's current transformation:  for example, if the @cr has a
 * rotated current transformation matrix, the whole SVG will be rotated in the
 * rendered version.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Note that @cr must be a Cairo context that is not in an error state, that is,
 * cairo_status() must return #CAIRO_STATUS_SUCCESS for it.  Cairo can set a
 * context to be in an error state in various situations, for example, if it was
 * passed an invalid matrix or if it was created for an invalid surface.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * Returns: %TRUE if drawing succeeded; %FALSE otherwise.
 * Since: 2.14
 */
gboolean
rsvg_handle_render_cairo_sub (RsvgHandle *handle, cairo_t *cr, const char *id)
{
    return rsvg_rust_handle_render_cairo_sub (handle, cr, id);
}

/**
 * rsvg_handle_render_cairo:
 * @handle: A #RsvgHandle
 * @cr: A Cairo context
 *
 * Draws a loaded SVG handle to a Cairo context.  Drawing will occur with
 * respect to the @cr's current transformation:  for example, if the @cr has a
 * rotated current transformation matrix, the whole SVG will be rotated in the
 * rendered version.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Note that @cr must be a Cairo context that is not in an error state, that is,
 * cairo_status() must return #CAIRO_STATUS_SUCCESS for it.  Cairo can set a
 * context to be in an error state in various situations, for example, if it was
 * passed an invalid matrix or if it was created for an invalid surface.
 *
 * Returns: %TRUE if drawing succeeded; %FALSE otherwise.
 * Since: 2.14
 */
gboolean
rsvg_handle_render_cairo (RsvgHandle *handle, cairo_t *cr)
{
    return rsvg_rust_handle_render_cairo (handle, cr);
}

/**
 * rsvg_handle_get_dimensions:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 *
 * Get the SVG's size. Do not call from within the size_func callback, because
 * an infinite loop will occur.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Since: 2.14
 */
void
rsvg_handle_get_dimensions (RsvgHandle *handle, RsvgDimensionData *dimension_data)
{
    rsvg_rust_handle_get_dimensions (handle, dimension_data);
}

/**
 * rsvg_handle_get_dimensions_sub:
 * @handle: A #RsvgHandle
 * @dimension_data: (out): A place to store the SVG's size
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to use the whole SVG.
 *
 * Get the size of a subelement of the SVG file. Do not call from within the
 * size_func callback, because an infinite loop will occur.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * Deprecated: 2.46.  Use rsvg_handle_get_geometry_for_layer() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_dimensions_sub (RsvgHandle *handle,
                                RsvgDimensionData *dimension_data,
                                const char *id)
{
    return rsvg_rust_handle_get_dimensions_sub (handle, dimension_data, id);
}

/**
 * rsvg_handle_get_position_sub:
 * @handle: A #RsvgHandle
 * @position_data: (out): A place to store the SVG fragment's position.
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to use the whole SVG.
 *
 * Get the position of a subelement of the SVG file. Do not call from within
 * the size_func callback, because an infinite loop will occur.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * Deprecated: 2.46.  Use rsvg_handle_get_geometry_for_layer() instead.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_get_position_sub (RsvgHandle *handle,
                              RsvgPositionData *position_data,
                              const char *id)
{
    return rsvg_rust_handle_get_position_sub (handle, position_data, id);
}

/**
 * rsvg_handle_has_sub:
 * @handle: a #RsvgHandle
 * @id: An element's id within the SVG, starting with "##" (a single hash
 * character), for example, "##layer1".  This notation corresponds to a URL's
 * fragment ID.
 *
 * Checks whether the element @id exists in the SVG document.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * Returns: %TRUE if @id exists in the SVG document, %FALSE otherwise.
 *
 * Since: 2.22
 */
gboolean
rsvg_handle_has_sub (RsvgHandle *handle, const char *id)
{
    return rsvg_rust_handle_has_sub (handle, id);
}

/**
 * rsvg_handle_get_pixbuf_sub:
 * @handle: An #RsvgHandle
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to use the whole SVG.
 *
 * Creates a #GdkPixbuf the same size as the entire SVG loaded into @handle, but
 * only renders the sub-element that has the specified @id (and all its
 * sub-sub-elements recursively).  If @id is #NULL, this function renders the
 * whole SVG.
 *
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * If you need to render an image which is only big enough to fit a particular
 * sub-element of the SVG, consider using rsvg_handle_render_cairo_sub(), upon a
 * surface that is just the size returned by rsvg_handle_get_dimensions_sub().
 * You will need to offset the rendering by the amount returned in
 * rsvg_handle_get_position_sub().
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * Returns: (transfer full) (nullable): a pixbuf, or %NULL if an error occurs
 * during rendering.
 *
 * Since: 2.14
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf_sub (RsvgHandle *handle, const char *id)
{
    return rsvg_rust_handle_get_pixbuf_sub (handle, id);
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
 * This function depends on the #RsvgHandle's DPI to compute dimensions in
 * pixels, so you should call rsvg_handle_set_dpi() beforehand.
 *
 * Returns: (transfer full) (nullable): the pixbuf loaded by @handle, or %NULL.
 **/
GdkPixbuf *
rsvg_handle_get_pixbuf (RsvgHandle *handle)
{
    return rsvg_rust_handle_get_pixbuf (handle);
}

/**
 * rsvg_handle_set_dpi:
 * @handle: An #RsvgHandle
 * @dpi: Dots Per Inch (i.e. as Pixels Per Inch)
 *
 * Sets the DPI at which the @handle will be rendered. Common values are
 * 75, 90, and 300 DPI.
 *
 * Passing a number <= 0 to @dpi will reset the DPI to whatever the default
 * value happens to be, but since rsvg_set_default_dpi() is deprecated, please
 * do not pass values <= 0 to this function.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi (RsvgHandle *handle, double dpi)
{
    rsvg_rust_handle_set_dpi (handle, dpi);
}

/**
 * rsvg_handle_set_dpi_x_y:
 * @handle: An #RsvgHandle
 * @dpi_x: Dots Per Inch (i.e. Pixels Per Inch)
 * @dpi_y: Dots Per Inch (i.e. Pixels Per Inch)
 *
 * Sets the DPI at which the @handle will be rendered. Common values are
 * 75, 90, and 300 DPI.
 *
 * Passing a number <= 0 to @dpi will reset the DPI to whatever the default
 * value happens to be, but since rsvg_set_default_dpi_x_y() is deprecated,
 * please do not pass values <= 0 to this function.
 *
 * Since: 2.8
 */
void
rsvg_handle_set_dpi_x_y (RsvgHandle *handle, double dpi_x, double dpi_y)
{
    rsvg_rust_handle_set_dpi_x_y (handle, dpi_x, dpi_y);
}

/**
 * rsvg_handle_set_size_callback:
 * @handle: An #RsvgHandle
 * @size_func: (nullable): A sizing function, or %NULL
 * @user_data: User data to pass to @size_func, or %NULL
 * @user_data_destroy: Function to be called to destroy the data passed in @user_data,
 *   or %NULL.
 *
 * Sets the sizing function for the @handle, which can be used to override the
 * size that librsvg computes for SVG images.  The @size_func is called from the
 * following functions:
 *
 * <itemizedlist>
 *   <listitem>rsvg_handle_get_dimensions()</listitem>
 *   <listitem>rsvg_handle_get_dimensions_sub()</listitem>
 *   <listitem>rsvg_handle_get_position_sub()</listitem>
 *   <listitem>rsvg_handle_render_cairo()</listitem>
 *   <listitem>rsvg_handle_render_cairo_sub()</listitem>
 * </itemizedlist>
 *
 * Librsvg computes the size of the SVG being rendered, and passes it to the
 * @size_func, which may then modify these values to set the final size of the
 * generated image.
 *
 * Deprecated: 2.14.  Set up a cairo matrix and use rsvg_handle_render_cairo() instead.
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
 *
 * This function was deprecated because when the @size_func is used, it makes it
 * unclear when the librsvg functions which call the @size_func will use the
 * size computed originally, or the callback-specified size, or whether it
 * refers to the whole SVG or to just a sub-element of it.  It is easier, and
 * unambiguous, to use code similar to the example above.
 **/
void
rsvg_handle_set_size_callback (RsvgHandle *handle,
                               RsvgSizeFunc size_func,
                               gpointer user_data,
                               GDestroyNotify user_data_destroy)
{
    rsvg_rust_handle_set_size_callback (handle,
                                        size_func,
                                        user_data,
                                        user_data_destroy);
}

/**
 * rsvg_handle_get_intrinsic_dimensions:
 * @handle: An #RsvgHandle
 * @out_has_width: (out)(optional): Will be set to #TRUE if the toplevel SVG has a <literal>width</literal> attribute
 * @out_width: (out)(optional): Will be set to the value of the <literal>width</literal> attribute in the toplevel SVG
 * @out_has_height: (out)(optional): Will be set to #TRUE if the toplevel SVG has a <literal>height</literal> attribute
 * @out_height: (out)(optional): Will be set to the value of the <literal>height</literal> attribute in the toplevel SVG
 * @out_has_viewbox: (out)(optional): Will be set to #TRUE if the toplevel SVG has a <literal>viewBox</literal> attribute
 * @out_viewbox: (out)(optional): Will be set to the value of the <literal>viewBox</literal> attribute in the toplevel SVG
 *
 * Queries the <literal>width</literal>, <literal>height</literal>, and
 * <literal>viewBox</literal> attributes in an SVG document.
 *
 * If you are calling this function to compute a scaling factor to render the SVG,
 * consider simply using rsvg_handle_render_document() instead; it will do the
 * scaling computations automatically.
 *
 * As an example, the following SVG element has a <literal>width</literal> of 100 pixels and a <literal>height</literal> of 400 pixels, but no <literal>viewBox</literal>:
 *
 * |[
 * <svg xmlns="http://www.w3.org/2000/svg" width="100" height="400">
 * ]|
 *
 * Conversely, the following element has a <literal>viewBox</literal>, but no <literal>width</literal> or <literal>height</literal>:
 *
 * |[
 * <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 400">
 * ]|
 *
 * Note that the #RsvgLength return values have #RsvgUnits in them; you should
 * not assume that they are always in pixels.  For example, the following SVG element
 * will return a width value whose <literal>units</literal> field is RSVG_UNIT_MM.
 *
 * |[
 * <svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm">
 * ]|
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
void
rsvg_handle_get_intrinsic_dimensions (RsvgHandle *handle,
                                      gboolean   *out_has_width,
                                      RsvgLength *out_width,
                                      gboolean   *out_has_height,
                                      RsvgLength *out_height,
                                      gboolean   *out_has_viewbox,
                                      RsvgRectangle *out_viewbox)
{
    rsvg_rust_handle_get_intrinsic_dimensions (handle,
                                               out_has_width,
                                               out_width,
                                               out_has_height,
                                               out_height,
                                               out_has_viewbox,
                                               out_viewbox);
}

/**
 * rsvg_handle_render_document:
 * @handle: An #RsvgHandle
 * @cr: A Cairo context
 * @viewport: Viewport size at which the whole SVG would be fitted.
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Renders the whole SVG document fitted to a viewport.
 *
 * The @viewport gives the position and size at which the whole SVG
 * document will be rendered.
 *
 * The @cr must be in a #CAIRO_STATUS_SUCCESS state, or this function will not
 * render anything, and instead will return an error.
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_render_document (RsvgHandle           *handle,
                             cairo_t              *cr,
                             const RsvgRectangle  *viewport,
                             GError              **error)
{
    return rsvg_rust_handle_render_document (handle, cr, viewport, error);
}

/**
 * rsvg_handle_get_geometry_for_layer:
 * @handle: An #RsvgHandle
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to compute the geometry for the
 * whole SVG.
 * @viewport: Viewport size at which the whole SVG would be fitted.
 * @out_ink_rect: (out)(optional): Place to store the ink rectangle of the element.
 * @out_logical_rect: (out)(optional): Place to store the logical rectangle of the element.
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Computes the ink rectangle and logical rectangle of an SVG element, or the
 * whole SVG, as if the whole SVG were rendered to a specific viewport.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * The "ink rectangle" is the bounding box that would be painted
 * for fully- stroked and filled elements.
 *
 * The "logical rectangle" just takes into account the unstroked
 * paths and text outlines.
 *
 * Note that these bounds are not minimum bounds; for example,
 * clipping paths are not taken into account.
 *
 * You can pass #NULL for the @id if you want to measure all
 * the elements in the SVG, i.e. to measure everything from the
 * root element.
 *
 * This operation is not constant-time, as it involves going through all
 * the child elements.
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_get_geometry_for_layer (RsvgHandle     *handle,
                                    const char     *id,
                                    const RsvgRectangle *viewport,
                                    RsvgRectangle  *out_ink_rect,
                                    RsvgRectangle  *out_logical_rect,
                                    GError        **error)
{
    return rsvg_rust_handle_get_geometry_for_layer (handle,
                                                    id,
                                                    viewport,
                                                    out_ink_rect,
                                                    out_logical_rect,
                                                    error);
}

/**
 * rsvg_handle_render_layer:
 * @handle: An #RsvgHandle
 * @cr: A Cairo context
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to render the whole SVG document tree.
 * @viewport: Viewport size at which the whole SVG would be fitted.
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Renders a single SVG element in the same place as for a whole SVG document.
 *
 * This is equivalent to rsvg_handle_render_document(), but it renders only a
 * single element and its children, as if they composed an individual layer in
 * the SVG.  The element is rendered with the same transformation matrix as it
 * has within the whole SVG document.  Applications can use this to re-render a
 * single element and repaint it on top of a previously-rendered document, for
 * example.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * You can pass #NULL for the @id if you want to render all
 * the elements in the SVG, i.e. to render everything from the
 * root element.
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_render_layer (RsvgHandle           *handle,
                          cairo_t              *cr,
                          const char           *id,
                          const RsvgRectangle  *viewport,
                          GError              **error)
{
    return rsvg_rust_handle_render_layer (handle, cr, id, viewport, error);
}

/**
 * rsvg_handle_get_geometry_for_element:
 * @handle: An #RsvgHandle
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to compute the geometry for the
 * whole SVG.
 * @out_ink_rect: (out)(optional): Place to store the ink rectangle of the element.
 * @out_logical_rect: (out)(optional): Place to store the logical rectangle of the element.
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Computes the ink rectangle and logical rectangle of a single SVG element.
 *
 * While `rsvg_handle_get_geometry_for_layer` computes the geometry of an SVG element subtree with
 * its transformation matrix, this other function will compute the element's geometry
 * as if it were being rendered under an identity transformation by itself.  That is,
 * the resulting geometry is as if the element got extracted by itself from the SVG.
 *
 * This function is the counterpart to `rsvg_handle_render_element`.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * The "ink rectangle" is the bounding box that would be painted
 * for fully- stroked and filled elements.
 *
 * The "logical rectangle" just takes into account the unstroked
 * paths and text outlines.
 *
 * Note that these bounds are not minimum bounds; for example,
 * clipping paths are not taken into account.
 *
 * You can pass #NULL for the @id if you want to measure all
 * the elements in the SVG, i.e. to measure everything from the
 * root element.
 *
 * This operation is not constant-time, as it involves going through all
 * the child elements.
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_get_geometry_for_element (RsvgHandle     *handle,
                                      const char     *id,
                                      RsvgRectangle  *out_ink_rect,
                                      RsvgRectangle  *out_logical_rect,
                                      GError        **error)
{
    return rsvg_rust_handle_get_geometry_for_element (handle,
                                                      id,
                                                      out_ink_rect,
                                                      out_logical_rect,
                                                      error);
}

/**
 * rsvg_handle_render_element:
 * @handle: An #RsvgHandle
 * @cr: A Cairo context
 * @id: (nullable): An element's id within the SVG, starting with "##" (a single
 * hash character), for example, "##layer1".  This notation corresponds to a
 * URL's fragment ID.  Alternatively, pass %NULL to render the whole SVG document tree.
 * @element_viewport: Viewport size in which to fit the element
 * @error: (optional): a location to store a #GError, or %NULL
 *
 * Renders a single SVG element to a given viewport
 *
 * This function can be used to extract individual element subtrees and render them,
 * scaled to a given @element_viewport.  This is useful for applications which have
 * reusable objects in an SVG and want to render them individually; for example, an
 * SVG full of icons that are meant to be be rendered independently of each other.
 *
 * Element IDs should look like an URL fragment identifier; for example, pass
 * "##foo" (hash <literal>foo</literal>) to get the geometry of the element that
 * has an <literal>id="foo"</literal> attribute.
 *
 * You can pass #NULL for the @id if you want to render all
 * the elements in the SVG, i.e. to render everything from the
 * root element.
 *
 * The `element_viewport` gives the position and size at which the named element will
 * be rendered.  FIXME: mention proportional scaling.
 *
 * API ordering: This function must be called on a fully-loaded @handle.  See
 * the section <ulink url="#API-ordering">API ordering</ulink> for details.
 *
 * Panics: this function will panic if the @handle is not fully-loaded.
 *
 * Since: 2.46
 */
gboolean
rsvg_handle_render_element (RsvgHandle           *handle,
                            cairo_t              *cr,
                            const char           *id,
                            const RsvgRectangle  *element_viewport,
                            GError              **error)
{
    return rsvg_rust_handle_render_element (handle, cr, id, element_viewport, error);
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
    rsvg_rust_handle_set_testing (handle, testing);
}

GType
rsvg_error_get_type(void)
{
    return rsvg_rust_error_get_type();
}

GType
rsvg_handle_flags_get_type(void)
{
    return rsvg_rust_handle_flags_get_type();
}
