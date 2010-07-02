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

enum {
    PROP_0,
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

static GObjectClass *rsvg_parent_class = NULL;

static void
instance_init (RsvgHandle * self)
{
    self->priv = g_new0 (RsvgHandlePrivate, 1);
    self->priv->defs = rsvg_defs_new ();
    self->priv->handler_nest = 0;
    self->priv->entities = g_hash_table_new (g_str_hash, g_str_equal);
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
#if GLIB_CHECK_VERSION (2, 24, 0)
    self->priv->data_input_stream = NULL;
#elif defined(HAVE_GSF)
    self->priv->gzipped_data = NULL;
#endif
    self->priv->first_write = TRUE;

    self->priv->is_disposed = FALSE;
    self->priv->in_loop = FALSE;
}

static void
rsvg_ctx_free_helper (gpointer key, gpointer value, gpointer user_data)
{
    xmlEntityPtr entval = (xmlEntityPtr) value;

#if LIBXML_VERSION < 20700
    /* key == entval->name, so it's implicitly freed below */

    xmlFree ((xmlChar *) entval->name);
    xmlFree ((xmlChar *) entval->ExternalID);
    xmlFree ((xmlChar *) entval->SystemID);
    xmlFree (entval->content);
    xmlFree (entval->orig);
    xmlFree (entval);
#else
    xmlFreeNode((xmlNode *) entval);
#endif
}

static void
instance_dispose (GObject * instance)
{
    RsvgHandle *self = (RsvgHandle *) instance;

    self->priv->is_disposed = TRUE;

    g_hash_table_foreach (self->priv->entities, rsvg_ctx_free_helper, NULL);
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

#if GLIB_CHECK_VERSION (2, 24, 0)
    if (self->priv->base_gfile) {
        g_object_unref (self->priv->base_gfile);
        self->priv->base_gfile = NULL;
    }
    if (self->priv->data_input_stream) {
        g_object_unref (self->priv->data_input_stream);
        self->priv->data_input_stream = NULL;
    }
#elif defined(HAVE_GSF)
    if (self->priv->gzipped_data) {
        g_object_unref (self->priv->gzipped_data);
        self->priv->gzipped_data = NULL;
    }
#endif

    g_free (self->priv);

    rsvg_parent_class->dispose (instance);
}

static void
set_property (GObject * instance, guint prop_id, GValue const *value, GParamSpec * pspec)
{
    RsvgHandle *self = RSVG_HANDLE (instance);

    switch (prop_id) {
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
get_property (GObject * instance, guint prop_id, GValue * value, GParamSpec * pspec)
{
    RsvgHandle *self = RSVG_HANDLE (instance);
    RsvgDimensionData dim;

    switch (prop_id) {
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
class_init (RsvgHandleClass * klass)
{
    GObjectClass *gobject_class = G_OBJECT_CLASS (klass);

    /* hook gobject vfuncs */
    gobject_class->dispose = instance_dispose;

    rsvg_parent_class = (GObjectClass *) g_type_class_peek_parent (klass);

    gobject_class->set_property = set_property;
    gobject_class->get_property = get_property;

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

    rsvg_SAX_handler_struct_init ();
}

const GTypeInfo rsvg_type_info = {
    sizeof (RsvgHandleClass),
    NULL,                       /* base_init */
    NULL,                       /* base_finalize */
    (GClassInitFunc) class_init,
    NULL,                       /* class_finalize */
    NULL,                       /* class_data */
    sizeof (RsvgHandle),
    0,                          /* n_preallocs */
    (GInstanceInitFunc) instance_init,
};

static GType rsvg_type = 0;

/* HACK to get around bugs 357406 and 362217. private API for now. */
GType
_rsvg_register_types (GTypeModule * module)
{
    rsvg_type = g_type_module_register_type (module,
                                             G_TYPE_OBJECT,
                                             "RsvgHandle", &rsvg_type_info, (GTypeFlags) 0);
    return rsvg_type;
}

GType
rsvg_handle_get_type (void)
{
    if (!rsvg_type) {
        rsvg_type =
            g_type_register_static (G_TYPE_OBJECT, "RsvgHandle", &rsvg_type_info, (GTypeFlags) 0);
    }
    return rsvg_type;
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
