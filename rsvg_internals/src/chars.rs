use libc;
use std;
use std::cell::RefCell;
use std::str;

use drawing_ctx::RsvgDrawingCtx;
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new, rsvg_node_get_state};
use property_bag::PropertyBag;
use state;

/// Container for XML character data.
///
/// In SVG text elements, we use `NodeChars` to store character data.  For example,
/// an element like `<text>Foo Bar</text>` will be a `NodeText` with a single child,
/// and the child will be a `NodeChars` with "Foo Bar" for its contents.
/// ```
///
/// Text elements can contain `<tspan>` sub-elements.  In this case,
/// those `tspan` nodes will also contain `NodeChars` children.
///
/// A text or tspan element can contain more than one `NodeChars` child, for example,
/// if there is an XML comment that splits the character contents in two:
///
/// ```xml
/// <text>
///   This sentence will create a NodeChars.
///   <!-- this comment is ignored -->
///   This sentence will cretea another NodeChars.
/// </text>
/// ```
///
/// When rendering a text element, it will take care of concatenating
/// the strings in its `NodeChars` children as appropriate, depending
/// on the `xml:space="preserve"` attribute.  A `NodeChars` stores the
/// characters verbatim as they come out of the XML parser, after
/// ensuring that they are valid UTF-8.
struct NodeChars {
    string: RefCell<String>
}

impl NodeChars {
    fn new() -> NodeChars {
        NodeChars {
            string: RefCell::new(String::new())
        }
    }

    fn append(&self, s: &str) {
        self.string.borrow_mut().push_str(s);
    }
}

impl NodeTrait for NodeChars {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern fn rsvg_node_chars_new(raw_parent: *const RsvgNode) -> *const RsvgNode {
    let node = boxed_node_new(NodeType::Chars,
                              raw_parent,
                              Box::new(NodeChars::new()));

    let state = rsvg_node_get_state(node);
    state::set_cond_true(state, false);

    node
}

#[no_mangle]
pub extern fn rsvg_node_chars_append(raw_node: *const RsvgNode,
                                     text: *const libc::c_char,
                                     len:  isize) {
    assert!(!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert!(!text.is_null());
    assert!(len >= 0);

    // libxml2 already validated the incoming string as UTF-8.  Note that
    // it is *not* nul-terminated; this is why we create a byte slice first.
    let bytes = unsafe { std::slice::from_raw_parts(text as *const u8, len as usize) };
    let utf8 = unsafe { str::from_utf8_unchecked(bytes) };

    node.with_impl(|chars: &NodeChars| {
        chars.append(utf8);
    });
}

#[no_mangle]
pub extern fn rsvg_node_chars_get_string(raw_node: *const RsvgNode,
                                         out_str: *mut *const libc::c_char,
                                         out_len: *mut usize) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert!(!out_str.is_null());
    assert!(!out_len.is_null());

    node.with_impl(|chars: &NodeChars| {
        let s = chars.string.borrow();
        unsafe {
            *out_str = s.as_ptr() as *const libc::c_char;
            *out_len = s.len();
        }
    });
}
