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
 * "<literal>2 pt</literal>" that must be converted from physical units
 * into device units.  To do this, librsvg needs to know the actual dots per
 * inch (DPI) of your target device.
 *
 * The recommended way to set the DPI is to use rsvg_handle_set_dpi() or
 * rsvg_handle_set_dpi_x_y() on an RsvgHandle before rendering it.
 *
 * Alternatively, you can use rsvg_set_default_dpi() or
 * rsvg_set_default_dpi_x_y() <emphasis>before</emphasis> creating any
 * RsvgHandle objects.  These functions will make RsvgHandle objects created
 * afterwards to have the default DPI value you specified.
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

#include "rsvg-private.h"
#include "rsvg-defs.h"

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
    self->priv->state = RSVG_HANDLE_STATE_START;
    self->priv->all_nodes = g_ptr_array_new ();
    self->priv->defs = rsvg_defs_new (self);
    self->priv->handler_nest = 0;
    self->priv->entities = g_hash_table_new_full (g_str_hash,
                                                  g_str_equal,
                                                  g_free,
                                                  (GDestroyNotify) xmlFreeNode);
    self->priv->dpi_x = rsvg_internal_dpi_x;
    self->priv->dpi_y = rsvg_internal_dpi_y;

    self->priv->css_props = g_hash_table_new_full (g_str_hash,
                                                   g_str_equal,
                                                   g_free,
                                                   (GDestroyNotify) g_hash_table_destroy);

    self->priv->ctxt = NULL;
    self->priv->currentnode = NULL;
    self->priv->treebase = NULL;
    self->priv->element_name_stack = NULL;

    self->priv->compressed_input_stream = NULL;
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

    self->priv->ctxt = rsvg_free_xml_parser_and_doc (self->priv->ctxt);

    g_hash_table_destroy (self->priv->entities);

    free_nodes (self);

    rsvg_defs_free (self->priv->defs);
    self->priv->defs = NULL;

    g_hash_table_destroy (self->priv->css_props);

    self->priv->treebase = rsvg_node_unref (self->priv->treebase);
    self->priv->currentnode = rsvg_node_unref (self->priv->currentnode);

    if (self->priv->user_data_destroy)
        (*self->priv->user_data_destroy) (self->priv->user_data);

    if (self->priv->title)
        g_string_free (self->priv->title, TRUE);
    if (self->priv->desc)
        g_string_free (self->priv->desc, TRUE);
    if (self->priv->metadata)
        g_string_free (self->priv->metadata, TRUE);
    if (self->priv->base_uri)
        g_free (self->priv->base_uri);

    if (self->priv->base_gfile) {
        g_object_unref (self->priv->base_gfile);
        self->priv->base_gfile = NULL;
    }
    if (self->priv->compressed_input_stream) {
        g_object_unref (self->priv->compressed_input_stream);
        self->priv->compressed_input_stream = NULL;
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
