/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-filter.c: Provides filters

   Copyright (C) 2004 Caleb Moore

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

   Author: Caleb Moore <c.moore@student.unsw.edu.au>
*/

#include "config.h"

#include "../rsvg-private.h"
#include "../rsvg-styles.h"
#include "../rsvg-css.h"
#include "../rsvg-cairo-draw.h"
#include "common.h"

typedef struct _RsvgNodeComponentTransferFunc RsvgNodeComponentTransferFunc;

typedef gint (*ComponentTransferFunc) (gint C, RsvgNodeComponentTransferFunc * user_data);

typedef struct _RsvgFilterPrimitiveComponentTransfer
 RsvgFilterPrimitiveComponentTransfer;

struct _RsvgNodeComponentTransferFunc {
    ComponentTransferFunc function;
    gint *tableValues;
    gsize nbTableValues;
    gint slope;
    gint intercept;
    gint amplitude;
    gint offset;
    gdouble exponent;
    char channel;
};

struct _RsvgFilterPrimitiveComponentTransfer {
    RsvgFilterPrimitive super;
};

static gint
identity_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    return C;
}

static gint
table_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    guint k;
    gint vk, vk1, distancefromlast;
    guint num_values;

    if (!user_data->nbTableValues)
        return C;

    num_values = user_data->nbTableValues;

    k = (C * (num_values - 1)) / 255;

    vk = user_data->tableValues[MIN (k, num_values - 1)];
    vk1 = user_data->tableValues[MIN (k + 1, num_values - 1)];

    distancefromlast = (C * (user_data->nbTableValues - 1)) - k * 255;

    return vk + distancefromlast * (vk1 - vk) / 255;
}

static gint
discrete_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    gint k;

    if (!user_data->nbTableValues)
        return C;

    k = (C * user_data->nbTableValues) / 255;

    return user_data->tableValues[CLAMP (k, 0, user_data->nbTableValues - 1)];
}

static gint
linear_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    return (user_data->slope * C) / 255 + user_data->intercept;
}

static gint
fixpow (gint base, gint exp)
{
    int out = 255;
    for (; exp > 0; exp--)
        out = out * base / 255;
    return out;
}

static gint
gamma_component_transfer_func (gint C, RsvgNodeComponentTransferFunc * user_data)
{
    if (floor (user_data->exponent) == user_data->exponent)
        return user_data->amplitude * fixpow (C, user_data->exponent) / 255 + user_data->offset;
    else
        return (double) user_data->amplitude * pow ((double) C / 255.,
                                                    user_data->exponent) + user_data->offset;
}

struct component_transfer_closure {
    int channel_num;
    char channel;
    gboolean set_func;
    RsvgNodeComponentTransferFunc *channels[4];
    ComponentTransferFunc functions[4];
    RsvgFilterContext *ctx;
};

static void
component_transfer_render_child (RsvgNode *node, struct component_transfer_closure *closure)
{
    RsvgNodeComponentTransferFunc *f;

    if (rsvg_node_get_type (node) != RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION)
        return;

    f = rsvg_rust_cnode_get_impl (node);

    if (f->channel == closure->channel) {
        closure->functions[closure->ctx->channelmap[closure->channel_num]] = f->function;
        closure->channels[closure->ctx->channelmap[closure->channel_num]] = f;
        closure->set_func = TRUE;
    }
}

static void
rsvg_filter_primitive_component_transfer_render (RsvgNode *node, RsvgFilterPrimitive *primitive, RsvgFilterContext *ctx)
{
    gint x, y, c;
    gint rowstride, height, width;
    RsvgIRect boundarys;
    guchar *inpix, outpix[4];
    gint achan = ctx->channelmap[3];
    guchar *in_pixels;
    guchar *output_pixels;
    cairo_surface_t *output, *in;
    RsvgNodeChildrenIter *iter;
    RsvgNode *child;
    struct component_transfer_closure closure;

    boundarys = rsvg_filter_primitive_get_bounds (primitive, ctx);

    closure.ctx = ctx;

    for (c = 0; c < 4; c++) {
        closure.channel_num = c;
        closure.channel = "rgba"[c]; /* see rsvg_new_node_component_transfer_function() for where these chars come from */
        closure.set_func = FALSE;

        iter = rsvg_node_children_iter_begin (node);

        while (rsvg_node_children_iter_next (iter, &child)) {
            component_transfer_render_child (child, &closure);
            child = rsvg_node_unref (child);
        }

        rsvg_node_children_iter_end (iter);

        if (!closure.set_func)
            closure.functions[ctx->channelmap[c]] = identity_component_transfer_func;
    }

    in = rsvg_filter_get_in (primitive->in, ctx);
    if (in == NULL)
        return;

    cairo_surface_flush (in);

    in_pixels = cairo_image_surface_get_data (in);

    height = cairo_image_surface_get_height (in);
    width = cairo_image_surface_get_width (in);

    rowstride = cairo_image_surface_get_stride (in);

    output = _rsvg_image_surface_new (width, height);
    if (output == NULL) {
        cairo_surface_destroy (in);
        return;
    }

    output_pixels = cairo_image_surface_get_data (output);

    for (y = boundarys.y0; y < boundarys.y1; y++)
        for (x = boundarys.x0; x < boundarys.x1; x++) {
            inpix = in_pixels + (y * rowstride + x * 4);
            for (c = 0; c < 4; c++) {
                gint temp;
                int inval;
                if (c != achan) {
                    if (inpix[achan] == 0)
                        inval = 0;
                    else
                        inval = inpix[c] * 255 / inpix[achan];
                } else
                    inval = inpix[c];

                temp = closure.functions[c] (inval, closure.channels[c]);
                if (temp > 255)
                    temp = 255;
                else if (temp < 0)
                    temp = 0;
                outpix[c] = temp;
            }
            for (c = 0; c < 3; c++)
                output_pixels[y * rowstride + x * 4 + ctx->channelmap[c]] =
                    outpix[ctx->channelmap[c]] * outpix[achan] / 255;
            output_pixels[y * rowstride + x * 4 + achan] = outpix[achan];
        }

    cairo_surface_mark_dirty (output);

    rsvg_filter_store_result (primitive->result, output, ctx);

    cairo_surface_destroy (in);
    cairo_surface_destroy (output);
}

static void
rsvg_filter_primitive_component_transfer_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgFilterPrimitiveComponentTransfer *filter = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    filter_primitive_set_x_y_width_height_atts ((RsvgFilterPrimitive *) filter, atts);

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_IN:
            g_string_assign (filter->super.in, value);
            break;

        case RSVG_ATTRIBUTE_RESULT:
            g_string_assign (filter->super.result, value);
            break;

        default:
            break;
        }
    }

    rsvg_property_bag_iter_end (iter);
}

RsvgNode *
rsvg_new_filter_primitive_component_transfer (const char *element_name, RsvgNode *parent)
{
    RsvgFilterPrimitiveComponentTransfer *filter;

    filter = g_new0 (RsvgFilterPrimitiveComponentTransfer, 1);
    filter->super.result = g_string_new ("none");
    filter->super.in = g_string_new ("none");
    filter->super.render = rsvg_filter_primitive_component_transfer_render;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_FILTER_PRIMITIVE_COMPONENT_TRANSFER,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_filter_primitive_component_transfer_set_atts,
                                rsvg_filter_draw,
                                rsvg_filter_primitive_free);
}

static void
rsvg_node_component_transfer_function_set_atts (RsvgNode *node, gpointer impl, RsvgHandle *handle, RsvgPropertyBag atts)
{
    RsvgNodeComponentTransferFunc *data = impl;
    RsvgPropertyBagIter *iter;
    const char *key;
    RsvgAttribute attr;
    const char *value;

    iter = rsvg_property_bag_iter_begin (atts);

    while (rsvg_property_bag_iter_next (iter, &key, &attr, &value)) {
        switch (attr) {
        case RSVG_ATTRIBUTE_TYPE:
            if (!strcmp (value, "identity"))
                data->function = identity_component_transfer_func;
            else if (!strcmp (value, "table"))
                data->function = table_component_transfer_func;
            else if (!strcmp (value, "discrete"))
                data->function = discrete_component_transfer_func;
            else if (!strcmp (value, "linear"))
                data->function = linear_component_transfer_func;
            else if (!strcmp (value, "gamma"))
                data->function = gamma_component_transfer_func;
            break;

        case RSVG_ATTRIBUTE_TABLE_VALUES: {
            unsigned int i;
            double *temp;
            if (!rsvg_css_parse_number_list (value,
                                             NUMBER_LIST_LENGTH_MAXIMUM,
                                             256,
                                             &temp,
                                             &data->nbTableValues)) {
                rsvg_node_set_attribute_parse_error (node, "tableValues", "invalid number list");
                goto out;
            }

            data->tableValues = g_new0 (gint, data->nbTableValues);
            for (i = 0; i < data->nbTableValues; i++)
                data->tableValues[i] = temp[i] * 255.;
            g_free (temp);
            break;
        }

        case RSVG_ATTRIBUTE_SLOPE:
            data->slope = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_INTERCEPT:
            data->intercept = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_AMPLITUDE:
            data->amplitude = g_ascii_strtod (value, NULL) * 255.;
            break;

        case RSVG_ATTRIBUTE_EXPONENT:
            data->exponent = g_ascii_strtod (value, NULL);
            break;

        case RSVG_ATTRIBUTE_OFFSET:
            data->offset = g_ascii_strtod (value, NULL) * 255.;
            break;

        default:
            break;
        }
    }

out:
    rsvg_property_bag_iter_end (iter);
}

static void
rsvg_node_component_transfer_function_free (gpointer impl)
{
    RsvgNodeComponentTransferFunc *filter = impl;

    if (filter->nbTableValues)
        g_free (filter->tableValues);

    g_free (filter);
}

RsvgNode *
rsvg_new_node_component_transfer_function (const char *element_name, RsvgNode *parent)
{
    RsvgNodeComponentTransferFunc *filter;

    char channel;

    if (strcmp (element_name, "feFuncR") == 0)
        channel = 'r';
    else if (strcmp (element_name, "feFuncG") == 0)
        channel = 'g';
    else if (strcmp (element_name, "feFuncB") == 0)
        channel = 'b';
    else if (strcmp (element_name, "feFuncA") == 0)
        channel = 'a';
    else {
        g_assert_not_reached ();
        channel = '\0';
    }

    filter = g_new0 (RsvgNodeComponentTransferFunc, 1);
    filter->function = identity_component_transfer_func;
    filter->nbTableValues = 0;
    filter->channel = channel;

    return rsvg_rust_cnode_new (RSVG_NODE_TYPE_COMPONENT_TRANFER_FUNCTION,
                                parent,
                                rsvg_state_new (),
                                filter,
                                rsvg_node_component_transfer_function_set_atts,
                                rsvg_filter_draw,
                                rsvg_node_component_transfer_function_free);
}
