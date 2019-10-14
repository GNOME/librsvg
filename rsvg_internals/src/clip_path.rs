use cairo;
use markup5ever::local_name;

use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::DrawingCtx;
use crate::error::RenderingError;
use crate::node::{CascadedValues, NodeDraw, NodeResult, NodeTrait, RsvgNode};
use crate::parsers::ParseValue;
use crate::property_bag::PropertyBag;

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

#[derive(Default)]
pub struct NodeClipPath {
    units: ClipPathUnits,
}

impl NodeClipPath {
    pub fn get_units(&self) -> ClipPathUnits {
        self.units
    }

    pub fn to_cairo_context(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        if self.units == ClipPathUnits(CoordUnits::ObjectBoundingBox) && bbox.rect.is_none() {
            // The node being clipped is empty / doesn't have a
            // bounding box, so there's nothing to clip!
            return Ok(());
        }

        let cascaded = CascadedValues::new_from_node(node);

        draw_ctx
            .with_saved_matrix(&mut |dc| {
                let cr = dc.get_cairo_context();

                if self.units == ClipPathUnits(CoordUnits::ObjectBoundingBox) {
                    let bbox_rect = bbox.rect.as_ref().unwrap();

                    cr.transform(cairo::Matrix::new(
                        bbox_rect.width,
                        0.0,
                        0.0,
                        bbox_rect.height,
                        bbox_rect.x,
                        bbox_rect.y,
                    ))
                }

                // here we don't push a layer because we are clipping
                let res = node.draw_children(&cascaded, dc, true);

                cr.clip();

                res
            })
            .and_then(|_bbox|
                      // Clipping paths do not contribute to bounding boxes (they should,
                      // but we need Real Computational Geometry(tm), so ignore the
                      // bbox from the clip path.
                      Ok(()))
    }
}

impl NodeTrait for NodeClipPath {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("clipPathUnits") => self.units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}
