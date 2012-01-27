/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 ts=4 expandtab: */
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
 * @short_description: Create and manipulate SVG objects
 *
 * librsvg is a component used within software applications to enable
 * support for SVG-format scalable graphics. In contrast to raster
 * formats, scalable vector graphics provide users and artists a way
 * to create, view, and provide imagery that is not limited to the
 * pixel or dot density that an output device is capable of.
 *
 * Many software developers use the librsvg library to render
 * SVG graphics. It is lightweight and portable.
 */

#include "config.h"

#include "rsvg-private.h"
#include "rsvg-defs.h"
#include "librsvg-enum-types.h"

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
    self->priv->load_policy = RSVG_LOAD_POLICY_DEFAULT;
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

    self->priv->finished = 0;
    self->priv->data_input_stream = NULL;
    self->priv->first_write = TRUE;
    self->priv->cancellable = NULL;

    self->priv->is_disposed = FALSE;
    self->priv->in_loop = FALSE;
}

static void
rsvg_handle_dispose (GObject *instance)
{
    RsvgHandle *self = (RsvgHandle *) instance;

    if (self->priv->is_disposed)
      goto chain;

    self->priv->is_disposed = TRUE;

    g_hash_table_destroy (self->priv->entities);
    rsvg_defs_free (self->priv->defs);
    g_hash_table_destroy (self->priv->css_props);

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
    if (self->priv->data_input_stream) {
        g_object_unref (self->priv->data_input_stream);
        self->priv->data_input_stream = NULL;
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

    g_object_class_install_property (gobject_class,
                                     PROP_TITLE,
                                     g_param_spec_string ("title", _("Title"),
                                                          _("SVG file title"), NULL,
                                                          (GParamFlags) (G_PARAM_READABLE)));

    g_object_class_install_property (gobject_class,
                                     PROP_DESC,
                                     g_param_spec_string ("desc", _("Description"),
                                                          _("SVG file description"), NULL,
                                                          (GParamFlags) (G_PARAM_READABLE)));

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
 * Frees #handle.
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
