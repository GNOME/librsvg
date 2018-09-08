use std::cell::Cell;

use cairo::{self, MatrixTrait};

use attributes::Attribute;
use coord_units::CoordUnits;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

pub struct NodeClipPath {
    units: Cell<ClipPathUnits>,
}

impl NodeClipPath {
    pub fn new() -> NodeClipPath {
        NodeClipPath {
            units: Cell::new(ClipPathUnits::default()),
        }
    }

    pub fn get_units(&self) -> ClipPathUnits {
        self.units.get()
    }

    pub fn to_cairo_context(
        &self,
        node: &RsvgNode,
        affine_before_clip: &cairo::Matrix,
        draw_ctx: &mut DrawingCtx<'_>,
    ) -> Result<(), RenderingError> {
        let cascaded = node.get_cascaded_values();

        let clip_units = self.units.get();

        let orig_bbox = draw_ctx.get_bbox().clone();

        let child_matrix = if clip_units == ClipPathUnits(CoordUnits::ObjectBoundingBox) {
            if orig_bbox.rect.is_none() {
                // The node being clipped is empty / doesn't have a
                // bounding box, so there's nothing to clip!
                return Ok(());
            }

            let rect = orig_bbox.rect.unwrap();

            let mut bbtransform =
                cairo::Matrix::new(rect.width, 0.0, 0.0, rect.height, rect.x, rect.y);
            cairo::Matrix::multiply(&bbtransform, affine_before_clip)
        } else {
            *affine_before_clip
        };

        let cr = draw_ctx.get_cairo_context();
        let save_affine = cr.get_matrix();
        cr.set_matrix(child_matrix);

        // here we don't push a layer because we are clipping
        let res = node.draw_children(&cascaded, draw_ctx, true);

        cr.set_matrix(save_affine);

        // FIXME: this is an EPIC HACK to keep the clipping context from
        // accumulating bounding boxes.  We'll remove this later, when we
        // are able to extract bounding boxes from outside the
        // general drawing loop.
        draw_ctx.set_bbox(&orig_bbox);

        let cr = draw_ctx.get_cairo_context();
        cr.clip();

        res
    }
}

impl NodeTrait for NodeClipPath {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::ClipPathUnits => self.units.set(parse("clipPathUnits", value, ())?),

                _ => (),
            }
        }

        Ok(())
    }
}
