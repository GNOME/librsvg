use lazy_static::lazy_static;
use markup5ever::local_name;
use regex::{Captures, Regex};
use std::borrow::Cow;

use crate::drawing_ctx::DrawingCtx;
use crate::error::RenderingError;
use crate::node::*;
use crate::property_bag::PropertyBag;

#[derive(Default)]
pub struct NodeLink {
    link: Option<String>,
}

impl NodeTrait for NodeLink {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("xlink:href") => self.link = Some(value.to_owned()),
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
    ) -> Result<(), RenderingError> {
        let cascaded = CascadedValues::new(cascaded, node);
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| match self.link.as_ref() {
            Some(l) if !l.is_empty() => {
                const CAIRO_TAG_LINK: &str = "Link";

                let attributes = format!("uri='{}'", escape_value(l));

                let cr = dc.get_cairo_context();

                cr.tag_begin(CAIRO_TAG_LINK, &attributes);

                let res = node.draw_children(&cascaded, dc, clipping);

                cr.tag_end(CAIRO_TAG_LINK);

                res
            }
            _ => node.draw_children(&cascaded, dc, clipping),
        })
    }
}

/// escape quotes and backslashes with backslash
fn escape_value(value: &str) -> Cow<'_, str> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"['\\]").unwrap();
    }

    REGEX.replace_all(value, |caps: &Captures<'_>| {
        match caps.get(0).unwrap().as_str() {
            "'" => "\\'".to_owned(),
            "\\" => "\\\\".to_owned(),
            _ => unreachable!(),
        }
    })
}
