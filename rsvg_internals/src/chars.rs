use libc;
use pango::{self, ContextExt, LayoutExt};
use std;
use std::cell::RefCell;
use std::str;

use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use node::{NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode, boxed_node_new, rsvg_node_get_state};
use property_bag::PropertyBag;
use space::xml_space_normalize;
use state::{self, RsvgState, UnicodeBidi};

extern "C" {
    fn _rsvg_css_accumulate_baseline_shift(
        state: *mut RsvgState,
        draw_ctx: *const RsvgDrawingCtx,
    ) -> libc::c_double;
}

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

    fn measure(&self, draw_ctx: *const RsvgDrawingCtx) -> f64 {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, &s);
        let (width, _) = layout.get_size();

        f64::from(width) / f64::from(pango::SCALE)
    }

    fn render(&self, draw_ctx: *const RsvgDrawingCtx, x: &mut f64, y: &mut f64) {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, &s);
        let (width, _) = layout.get_size();
        let mut offset = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);

        let state = drawing_ctx::get_current_state(draw_ctx);

        unsafe {
            offset += _rsvg_css_accumulate_baseline_shift(state, draw_ctx);
        }

        let gravity = state::get_text_gravity(state);
        if gravity_is_vertical(gravity) {
            drawing_ctx::render_pango_layout(draw_ctx, &layout, *x + offset, *y);
            *y += f64::from(width) / f64::from(pango::SCALE);
        } else {
            drawing_ctx::render_pango_layout(draw_ctx, &layout, *x, *y - offset);
            *x += f64::from(width) / f64::from(pango::SCALE);
        }
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

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() /
// PANGO_GRAVITY_IS_IMPROPER()?
pub fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false,
    }
}

fn to_pango_units(v: f64) -> i32 {
    (v * f64::from(pango::SCALE)) as i32
}

fn create_pango_layout(draw_ctx: *const RsvgDrawingCtx, text: &str) -> pango::Layout {
    let state = drawing_ctx::get_current_state(draw_ctx);
    let pango_context = drawing_ctx::get_pango_context(draw_ctx);

    if let Some(lang) = state::get_language(state) {
        let pango_lang = pango::Language::from_string(&lang);
        pango_context.set_language(&pango_lang);
    }

    let unicode_bidi = state::get_unicode_bidi(state);
    match unicode_bidi {
        UnicodeBidi::Override | UnicodeBidi::Embed => {
            pango_context.set_base_dir(state::get_text_dir(state));
        }

        _ => (),
    }

    let gravity = state::get_text_gravity(state);
    if gravity_is_vertical(gravity) {
        pango_context.set_base_gravity(gravity);
    }

    let mut font_desc = pango_context.get_font_description().unwrap();

    if let Some(font_family) = state::get_font_family(state) {
        font_desc.set_family(&font_family);
    }

    font_desc.set_style(state::get_font_style(state));
    font_desc.set_variant(state::get_font_variant(state));
    font_desc.set_weight(state::get_font_weight(state));
    font_desc.set_stretch(state::get_font_stretch(state));

    let (_, dpi_y) = drawing_ctx::get_dpi(draw_ctx);
    font_desc.set_size(to_pango_units(
        drawing_ctx::get_normalized_font_size(draw_ctx) / dpi_y * 72.0,
    ));

    let layout = pango::Layout::new(&pango_context);
    layout.set_font_description(&font_desc);

    let attr_list = pango::AttrList::new();

    attr_list.insert(
        pango::Attribute::new_letter_spacing(to_pango_units(
            state::get_letter_spacing(state).normalize(draw_ctx),
        )).unwrap(),
    );

    if let Some(font_decor) = state::get_font_decor(state) {
        if font_decor.underline {
            attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single).unwrap());
        }

        if font_decor.strike {
            attr_list.insert(pango::Attribute::new_strikethrough(true).unwrap());
        }
    }

    layout.set_attributes(&attr_list);

    layout.set_alignment(match state::get_text_dir(state) {
        pango::Direction::Ltr => pango::Alignment::Left,
        _ => pango::Alignment::Right,
    });

    let t = xml_space_normalize(state::get_state_rust(state).xml_space, text);
    layout.set_text(&t);

    layout
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
pub extern "C" fn rsvg_node_chars_measure(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
) -> libc::c_double {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    let mut res: libc::c_double = 0f64;
    node.with_impl(|chars: &NodeChars| {
        res = chars.measure(draw_ctx)
    });

    res
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_render(
    raw_node: *const RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    raw_x: *mut libc::c_double,
    raw_y: *mut libc::c_double,
) {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert!(!raw_x.is_null());
    assert!(!raw_y.is_null());
    let x: &mut f64 = unsafe { &mut *raw_x };
    let y: &mut f64 = unsafe { &mut *raw_y };

    node.with_impl(|chars: &NodeChars| {
        chars.render(draw_ctx, x, y);
    });
}
