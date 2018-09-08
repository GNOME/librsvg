use cairo;
use cairo_sys;
use glib::translate::*;
use libc;

use regex::{Captures, Regex};
use std::borrow::Cow;
use std::cell::RefCell;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use handle::RsvgHandle;
use node::*;
use property_bag::PropertyBag;

pub struct NodeLink {
    link: RefCell<Option<String>>,
}

impl NodeLink {
    pub fn new() -> NodeLink {
        NodeLink {
            link: RefCell::new(None),
        }
    }
}

impl NodeTrait for NodeLink {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let link = self.link.borrow();

        let cascaded = CascadedValues::new(cascaded, node);
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            if link.is_some() && link.as_ref().unwrap() != "" {
                const CAIRO_TAG_LINK: &str = "Link";

                let attributes = link.as_ref().map(|i| format!("uri='{}'", escape_value(i)));

                let cr = dc.get_cairo_context();

                cr.tag_begin(CAIRO_TAG_LINK, attributes.as_ref().map(|i| i.as_str()));

                let res = node.draw_children(&cascaded, dc, clipping);

                cr.tag_end(CAIRO_TAG_LINK);

                res
            } else {
                node.draw_children(&cascaded, dc, clipping)
            }
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

extern "C" {
    fn cairo_tag_begin(
        cr: *mut cairo_sys::cairo_t,
        tag_name: *const libc::c_char,
        attibutes: *const libc::c_char,
    );
    fn cairo_tag_end(cr: *mut cairo_sys::cairo_t, tag_name: *const libc::c_char);
}

/// Bindings that aren't supported by `cairo-rs` for now
trait CairoTagging {
    fn tag_begin(&self, tag_name: &str, attributes: Option<&str>);
    fn tag_end(&self, tag_name: &str);
}

impl CairoTagging for cairo::Context {
    fn tag_begin(&self, tag_name: &str, attributes: Option<&str>) {
        unsafe {
            cairo_tag_begin(
                self.to_glib_none().0,
                tag_name.to_glib_none().0,
                attributes.to_glib_none().0,
            );
        }
    }

    fn tag_end(&self, tag_name: &str) {
        unsafe {
            cairo_tag_end(self.to_glib_none().0, tag_name.to_glib_none().0);
        }
    }
}
