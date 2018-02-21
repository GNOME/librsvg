use cairo;
use cairo_sys;
use glib::translate::*;
use libc;

use std::borrow::Cow;
use std::cell::RefCell;
use regex::{Regex, Captures};

use attributes::Attribute;
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use node::*;
use property_bag::PropertyBag;

struct NodeLink {
    link: RefCell<Option<String>>,
}

impl NodeLink {
    fn new() -> NodeLink {
        NodeLink { link: RefCell::new(None) }
    }
}

impl NodeTrait for NodeLink {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, node: &RsvgNode, draw_ctx: *const RsvgDrawingCtx, dominate: i32) {
        const CAIRO_TAG_LINK: &str = "Link";

        let cairo_ctx = drawing_ctx::get_cairo_context(draw_ctx);
        let link = self.link.borrow().as_ref().map(|i| {
            format!("uri='{}'", escape_value(i))
        });

        cairo_ctx.with_tag(CAIRO_TAG_LINK, link.as_ref().map(|i| i.as_str()), || {
            node.draw_children(draw_ctx, dominate)
        })
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

/// escape quotes and backslashes with backslash
fn escape_value(value: &str) -> Cow<str> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new(r"['\\]").unwrap();
    }

    REGEX.replace_all(
        value,
        |caps: &Captures| match caps.get(0).unwrap().as_str() {
            "'" => "\\'".to_owned(),
            "\\" => "\\\\".to_owned(),
            _ => unreachable!(),
        },
    )
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
    fn with_tag<U, T>(&self, tag_name: &str, attributes: Option<&str>, f: U) -> T
    where
        U: Fn() -> T,
    {
        self.tag_begin(tag_name, attributes);
        let result = f();
        self.tag_end(tag_name);

        result
    }
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

/***** C Prototypes *****/

#[no_mangle]
pub extern "C" fn rsvg_node_link_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Link, raw_parent, Box::new(NodeLink::new()))
}
