//! The `link` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::bbox::BoundingBox;
use crate::drawing_ctx::DrawingCtx;
use crate::error::RenderingError;
use crate::node::*;
use crate::property_bag::PropertyBag;

#[derive(Default)]
pub struct Link {
    link: Option<String>,
}

impl NodeTrait for Link {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(xlink "href") => self.link = Some(value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let cascaded = CascadedValues::new(cascaded, node);
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| match self.link.as_ref() {
            Some(l) if !l.is_empty() => {
                dc.with_link_tag(l, &mut |dc| node.draw_children(&cascaded, dc, clipping))
            }
            _ => node.draw_children(&cascaded, dc, clipping),
        })
    }
}
