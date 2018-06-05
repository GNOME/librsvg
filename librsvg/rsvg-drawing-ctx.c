/* -*- Mode: C; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg-shapes.c: Draw shapes with cairo

   Copyright (C) 2005 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2005 Caleb Moore <c.moore@student.unsw.edu.au>
   Copyright (C) 2005 Red Hat, Inc.

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

   Authors: Dom Lachowicz <cinamod@hotmail.com>,
            Caleb Moore <c.moore@student.unsw.edu.au>
            Carl Worth <cworth@cworth.org>
*/

#include "config.h"

#include "rsvg-drawing-ctx.h"
#include "rsvg-styles.h"
#include "rsvg-defs.h"
#include "rsvg-filter.h"
#include "rsvg-structure.h"

#include <math.h>
#include <string.h>

#include <pango/pangocairo.h>

/* Implemented in rsvg_internals/src/drawing_ctx.rs */
G_GNUC_INTERNAL
void rsvg_drawing_ctx_transformed_image_bounding_box (cairo_matrix_t *affine,
                                                      double width, double height,
                                                      double *bbx, double *bby, double *bbw, double *bbh);

RsvgDrawingCtx *
rsvg_drawing_ctx_new (cairo_t *cr, RsvgHandle *handle)
{
    RsvgDimensionData data;
    RsvgDrawingCtx *draw;
    cairo_matrix_t affine;
    cairo_matrix_t scale;
    double bbx, bby, bbw, bbh;

    rsvg_handle_get_dimensions (handle, &data);
    if (data.width == 0 || data.height == 0)
        return NULL;

    draw = g_new0 (RsvgDrawingCtx, 1);

    cairo_get_matrix (cr, &affine);

    /* find bounding box of image as transformed by the current cairo context
     * The size of this bounding box determines the size of the intermediate
     * surfaces allocated during drawing. */
    rsvg_drawing_ctx_transformed_image_bounding_box (&affine,
                                                     data.width, data.height,
                                                     &bbx, &bby, &bbw, &bbh);

    draw->initial_cr = cr;
    draw->cr = cr;
    draw->cr_stack = NULL;
    draw->surfaces_stack = NULL;

    draw->rect.x = bbx;
    draw->rect.y = bby;
    draw->rect.width = bbw;
    draw->rect.height = bbh;

    draw->defs = handle->priv->defs;
    draw->dpi_x = handle->priv->dpi_x;
    draw->dpi_y = handle->priv->dpi_y;
    draw->vb.rect.width = data.em;
    draw->vb.rect.height = data.ex;
    draw->vb_stack = NULL;
    draw->drawsub_stack = NULL;
    draw->acquired_nodes = NULL;
    draw->is_testing = handle->priv->is_testing;

    /* scale according to size set by size_func callback */
    cairo_matrix_init_scale (&scale, data.width / data.em, data.height / data.ex);
    cairo_matrix_multiply (&affine, &affine, &scale);

    /* adjust transform so that the corner of the bounding box above is
     * at (0,0) - we compensate for this in _set_rsvg_affine() in
     * rsvg-cairo-render.c and a few other places */
    affine.x0 -= draw->rect.x;
    affine.y0 -= draw->rect.y;

    cairo_set_matrix (cr, &affine);

    draw->bbox = rsvg_bbox_new (&affine, NULL, NULL);

    return draw;
}

void
rsvg_drawing_ctx_free (RsvgDrawingCtx *ctx)
{
    g_assert (ctx->cr_stack == NULL);
    g_assert (ctx->surfaces_stack == NULL);

    g_slist_free_full (ctx->drawsub_stack, (GDestroyNotify) rsvg_node_unref);

    g_warn_if_fail (ctx->acquired_nodes == NULL);
    g_slist_free (ctx->acquired_nodes);

    g_assert (ctx->bb_stack == NULL);

    rsvg_bbox_free (ctx->bbox);

    g_free (ctx);
}

cairo_t *
rsvg_drawing_ctx_get_cairo_context (RsvgDrawingCtx *ctx)
{
    return ctx->cr;
}

/* FIXME: Usage of this function is more less a hack.  Some code does this:
 *
 *   save_cr = rsvg_drawing_ctx_get_cairo_context (ctx);
 *
 *   some_surface = create_surface ();
 *
 *   cr = cairo_create (some_surface);
 *
 *   rsvg_drawing_ctx_set_cairo_context (ctx, cr);
 *
 *   ... draw with ctx but to that temporary surface
 *
 *   rsvg_drawing_ctx_set_cairo_context (ctx, save_cr);
 *
 * It would be better to have an explicit push/pop for the cairo_t, or
 * pushing a temporary surface, or something that does not involve
 * monkeypatching the cr directly.
 */
void
rsvg_drawing_ctx_set_cairo_context (RsvgDrawingCtx *ctx, cairo_t *cr)
{
    ctx->cr = cr;
}

gboolean
rsvg_drawing_ctx_is_cairo_context_nested (RsvgDrawingCtx *ctx, cairo_t *cr)
{
    return cr != ctx->initial_cr;
}

void
rsvg_drawing_ctx_push_bounding_box (RsvgDrawingCtx *ctx)
{
    cairo_t *cr;
    cairo_matrix_t affine;

    cr = rsvg_drawing_ctx_get_cairo_context (ctx);
    cairo_get_matrix (cr, &affine);

    ctx->bb_stack = g_list_prepend (ctx->bb_stack, ctx->bbox);
    ctx->bbox = rsvg_bbox_new (&affine, NULL, NULL);
}

void
rsvg_drawing_ctx_pop_bounding_box (RsvgDrawingCtx *ctx)
{
    rsvg_bbox_insert ((RsvgBbox *) ctx->bb_stack->data, ctx->bbox);
    ctx->bbox = (RsvgBbox *) ctx->bb_stack->data;
    ctx->bb_stack = g_list_delete_link (ctx->bb_stack, ctx->bb_stack);
}

void
rsvg_drawing_ctx_push_surface (RsvgDrawingCtx *ctx, cairo_surface_t *surface)
{
    ctx->surfaces_stack = g_list_prepend (ctx->surfaces_stack, cairo_surface_reference (surface));
}

cairo_surface_t *
rsvg_drawing_ctx_pop_surface (RsvgDrawingCtx *ctx)
{
    cairo_surface_t *surface;

    g_assert (ctx->surfaces_stack != NULL);

    surface = ctx->surfaces_stack->data;
    ctx->surfaces_stack = g_list_delete_link (ctx->surfaces_stack, ctx->surfaces_stack);

    return surface;
}

void
rsvg_drawing_ctx_push_cr (RsvgDrawingCtx *ctx, cairo_t *cr)
{
    ctx->cr_stack = g_list_prepend (ctx->cr_stack, ctx->cr);
    ctx->cr = cairo_reference (cr);

    /* Note that the "top of the stack" will now be ctx->cr, even if it is not
     * really in the list.
     */
}

void
rsvg_drawing_ctx_pop_cr (RsvgDrawingCtx *ctx)
{
    g_assert (ctx->cr != NULL);
    cairo_destroy (ctx->cr);

    g_assert (ctx->cr_stack != NULL);
    ctx->cr = ctx->cr_stack->data;
    g_assert (ctx->cr != NULL);
    ctx->cr_stack = g_list_delete_link (ctx->cr_stack, ctx->cr_stack);
}

/*
 * rsvg_drawing_ctx_acquire_node:
 * @ctx: The drawing context in use
 * @url: The IRI to lookup, or %NULL
 *
 * Use this function when looking up urls to other nodes. This
 * function does proper recursion checking and thereby avoids
 * infinite loops.
 *
 * Nodes acquired by this function must be released using
 * rsvg_drawing_ctx_release_node() in reverse acquiring order.
 *
 * Note that if you acquire a node, you have to release it before trying to
 * acquire it again.  If you acquire a node "#foo" and don't release it before
 * trying to acquire "foo" again, you will obtain a %NULL the second time.
 *
 * Returns: The node referenced by @url; or %NULL if the @url
 *          is %NULL or it does not reference a node.
 */
RsvgNode *
rsvg_drawing_ctx_acquire_node (RsvgDrawingCtx *ctx, const char *url)
{
  RsvgNode *node;

  if (url == NULL)
      return NULL;

  node = rsvg_defs_lookup (ctx->defs, url);
  if (node == NULL)
    return NULL;

  if (g_slist_find (ctx->acquired_nodes, node))
    return NULL;

  ctx->acquired_nodes = g_slist_prepend (ctx->acquired_nodes, node);

  return node;
}

/**
 * rsvg_drawing_ctx_acquire_node_of_type:
 * @ctx: The drawing context in use
 * @url: The IRI to lookup
 * @type: Type which the node must have
 *
 * Use this function when looking up urls to other nodes, and when you expect
 * the node to be of a particular type. This function does proper recursion
 * checking and thereby avoids infinite loops.
 *
 * Malformed SVGs, for example, may reference a marker by its IRI, but
 * the object referenced by the IRI is not a marker.
 *
 * Nodes acquired by this function must be released using
 * rsvg_drawing_ctx_release_node() in reverse acquiring order.
 *
 * Note that if you acquire a node, you have to release it before trying to
 * acquire it again.  If you acquire a node "#foo" and don't release it before
 * trying to acquire "foo" again, you will obtain a %NULL the second time.
 *
 * Returns: The node referenced by @url or %NULL if the @url
 *          does not reference a node.  Also returns %NULL if
 *          the node referenced by @url is not of the specified @type.
 */
RsvgNode *
rsvg_drawing_ctx_acquire_node_of_type (RsvgDrawingCtx *ctx, const char *url, RsvgNodeType type)
{
    RsvgNode *node;

    node = rsvg_drawing_ctx_acquire_node (ctx, url);
    if (node == NULL || rsvg_node_get_type (node) != type) {
        rsvg_drawing_ctx_release_node (ctx, node);
        return NULL;
    }

    return node;
}

/*
 * rsvg_drawing_ctx_release_node:
 * @ctx: The drawing context the node was acquired from
 * @node: Node to release
 *
 * Releases a node previously acquired via rsvg_drawing_ctx_acquire_node() or
 * rsvg_drawing_ctx_acquire_node_of_type().
 *
 * if @node is %NULL, this function does nothing.
 */
void
rsvg_drawing_ctx_release_node (RsvgDrawingCtx *ctx, RsvgNode *node)
{
  if (node == NULL)
    return;

  g_return_if_fail (ctx->acquired_nodes != NULL);
  g_return_if_fail (ctx->acquired_nodes->data == node);

  ctx->acquired_nodes = g_slist_remove (ctx->acquired_nodes, node);
}

void
rsvg_drawing_ctx_add_node_and_ancestors_to_stack (RsvgDrawingCtx *draw_ctx, RsvgNode *node)
{
    if (node) {
        node = rsvg_node_ref (node);

        while (node != NULL) {
            draw_ctx->drawsub_stack = g_slist_prepend (draw_ctx->drawsub_stack, node);
            node = rsvg_node_get_parent (node);
        }
    }
}

gboolean
rsvg_drawing_ctx_should_draw_node_from_stack (RsvgDrawingCtx *ctx,
                                              RsvgNode *node,
                                              GSList **out_stacksave)
{
    GSList *stacksave;
    gboolean should_draw = TRUE;

    stacksave = ctx->drawsub_stack;
    if (stacksave) {
        RsvgNode *stack_node = stacksave->data;

        if (!rsvg_node_is_same (stack_node, node)) {
            should_draw = FALSE;
        }

        ctx->drawsub_stack = stacksave->next;
    }

    *out_stacksave = stacksave;
    return should_draw;
}

void
rsvg_drawing_ctx_restore_stack (RsvgDrawingCtx *ctx,
                                GSList *stacksave)
{
    ctx->drawsub_stack = stacksave;
}

double
rsvg_drawing_ctx_get_width (RsvgDrawingCtx *draw_ctx)
{
    return draw_ctx->rect.width;
}

double
rsvg_drawing_ctx_get_height (RsvgDrawingCtx *draw_ctx)
{
    return draw_ctx->rect.height;
}

void
rsvg_drawing_ctx_get_raw_offset (RsvgDrawingCtx *draw_ctx, double *x, double *y)
{
    if (x != NULL) {
        *x = draw_ctx->rect.x;
    }

    if (y != NULL) {
        *y = draw_ctx->rect.y;
    }
}

void
rsvg_drawing_ctx_get_offset (RsvgDrawingCtx *draw_ctx, double *x, double *y)
{
    double xofs, yofs;

    if (rsvg_drawing_ctx_is_cairo_context_nested (draw_ctx, draw_ctx->cr)) {
        xofs = 0.0;
        yofs = 0.0;
    } else {
        xofs = draw_ctx->rect.x;
        yofs = draw_ctx->rect.y;
    }

    if (x != NULL) {
        *x = xofs;
    }

    if (y != NULL) {
        *y = yofs;
    }
}

RsvgBbox *
rsvg_drawing_ctx_get_bbox (RsvgDrawingCtx *ctx)
{
    return ctx->bbox;
}

void
rsvg_drawing_ctx_push_view_box (RsvgDrawingCtx *ctx, double w, double h)
{
    RsvgViewBox *vb = g_new0 (RsvgViewBox, 1);
    *vb = ctx->vb;
    ctx->vb_stack = g_slist_prepend (ctx->vb_stack, vb);
    ctx->vb.rect.width = w;
    ctx->vb.rect.height = h;
}

void
rsvg_drawing_ctx_pop_view_box (RsvgDrawingCtx *ctx)
{
    ctx->vb = *((RsvgViewBox *) ctx->vb_stack->data);
    g_free (ctx->vb_stack->data);
    ctx->vb_stack = g_slist_delete_link (ctx->vb_stack, ctx->vb_stack);
}

void
rsvg_drawing_ctx_get_view_box_size (RsvgDrawingCtx *ctx, double *out_width, double *out_height)
{
    if (out_width)
        *out_width = ctx->vb.rect.width;

    if (out_height)
        *out_height = ctx->vb.rect.height;
}

void
rsvg_drawing_ctx_get_dpi (RsvgDrawingCtx *ctx, double *out_dpi_x, double *out_dpi_y)
{
    if (out_dpi_x)
        *out_dpi_x = ctx->dpi_x;

    if (out_dpi_y)
        *out_dpi_y = ctx->dpi_y;
}

GList *
rsvg_drawing_ctx_get_cr_stack (RsvgDrawingCtx *ctx)
{
    return ctx->cr_stack;
}

gboolean
rsvg_drawing_ctx_is_testing (RsvgDrawingCtx *ctx)
{
    return ctx->is_testing;
}

void
rsvg_drawing_ctx_draw_node_on_surface (RsvgDrawingCtx *ctx,
                                       RsvgNode *node,
                                       cairo_surface_t *surface,
                                       double width,
                                       double height)
{
    cairo_t *save_cr = ctx->cr;
    cairo_t *save_initial_cr = ctx->initial_cr;
    cairo_rectangle_t save_rect = ctx->rect;
    cairo_matrix_t save_affine;

    cairo_get_matrix (save_cr, &save_affine);

    ctx->cr = cairo_create (surface);
    cairo_set_matrix (ctx->cr, &save_affine);

    ctx->initial_cr = ctx->cr;
    ctx->rect.x = 0;
    ctx->rect.y = 0;
    ctx->rect.width = width;
    ctx->rect.height = height;

    rsvg_drawing_ctx_draw_node_from_stack (ctx, node, FALSE);

    cairo_destroy (ctx->cr);
    ctx->cr = save_cr;
    ctx->initial_cr = save_initial_cr;
    ctx->rect = save_rect;
}
