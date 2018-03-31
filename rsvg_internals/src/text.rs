use libc;
use pango::{self, ContextExt, LayoutExt};
use std;
use std::cell::{Cell, RefCell};
use std::str;

use attributes::Attribute;
use draw::draw_pango_layout;
use drawing_ctx::{self, RsvgDrawingCtx};
use handle::RsvgHandle;
use length::*;
use node::{
    boxed_node_new,
    rsvg_node_get_state,
    NodeResult,
    NodeTrait,
    NodeType,
    RsvgCNodeImpl,
    RsvgNode,
};
use parsers::parse;
use property_bag::PropertyBag;
use space::xml_space_normalize;
use state::{self, LetterSpacing, RsvgState, TextAnchor, UnicodeBidi, XmlLang};

extern "C" {
    fn _rsvg_css_normalize_font_size(
        state: *mut RsvgState,
        draw_ctx: *const RsvgDrawingCtx,
    ) -> libc::c_double;
}

/// In SVG text elements, we use `NodeChars` to store character data.  For example,
/// an element like `<text>Foo Bar</text>` will be a `NodeText` with a single child,
/// and the child will be a `NodeChars` with "Foo Bar" for its contents.
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
/// When rendering a text element, it will take care of concatenating the strings
/// in its `NodeChars` children as appropriate, depending on the
/// `xml:space="preserve"` attribute.  A `NodeChars` stores the characters verbatim
/// as they come out of the XML parser, after ensuring that they are valid UTF-8.

struct NodeChars {
    string: RefCell<String>,
}

impl NodeChars {
    fn new() -> NodeChars {
        NodeChars {
            string: RefCell::new(String::new()),
        }
    }

    fn append(&self, s: &str) {
        self.string.borrow_mut().push_str(s);
    }

    fn measure(&self, draw_ctx: *const RsvgDrawingCtx, length: &mut f64) {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, &s);
        let (width, _) = layout.get_size();

        *length = f64::from(width) / f64::from(pango::SCALE);
    }

    fn render(&self, draw_ctx: *mut RsvgDrawingCtx, x: &mut f64, y: &mut f64, clipping: bool) {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, &s);
        let (width, _) = layout.get_size();

        let state = drawing_ctx::get_current_state(draw_ctx);

        let baseline = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);
        let offset = baseline + drawing_ctx::get_accumulated_baseline_shift(draw_ctx);

        let gravity = state::get_text_gravity(state);
        if gravity_is_vertical(gravity) {
            draw_pango_layout(draw_ctx, &layout, *x + offset, *y, clipping);
            *y += f64::from(width) / f64::from(pango::SCALE);
        } else {
            draw_pango_layout(draw_ctx, &layout, *x, *y - offset, clipping);
            *x += f64::from(width) / f64::from(pango::SCALE);
        }
    }
}

impl NodeTrait for NodeChars {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

struct NodeText {
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl NodeText {
    fn new() -> NodeText {
        NodeText {
            x: Cell::new(RsvgLength::default()),
            y: Cell::new(RsvgLength::default()),
            dx: Cell::new(RsvgLength::default()),
            dy: Cell::new(RsvgLength::default()),
        }
    }
}

impl NodeTrait for NodeText {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal, None)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical, None)?),
                Attribute::Dx => self.dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, node: &RsvgNode, draw_ctx: *mut RsvgDrawingCtx, dominate: i32, clipping: bool) {
        drawing_ctx::state_reinherit_top(draw_ctx, node.get_state(), dominate);

        let mut x = self.x.get().normalize(draw_ctx);
        let mut y = self.y.get().normalize(draw_ctx);
        let mut dx = self.dx.get().normalize(draw_ctx);
        let mut dy = self.dy.get().normalize(draw_ctx);

        let state = drawing_ctx::get_current_state(draw_ctx);
        let anchor = state::get_state_rust(state).text_anchor.unwrap_or_default();
        let gravity = state::get_text_gravity(state);

        let offset = anchor_offset(node, draw_ctx, anchor, false);

        if gravity_is_vertical(gravity) {
            y -= offset;
            dy = match anchor {
                TextAnchor::Start => dy,
                TextAnchor::Middle => dy / 2f64,
                _ => 0f64,
            }
        } else {
            x -= offset;
            dx = match anchor {
                TextAnchor::Start => dx,
                TextAnchor::Middle => dx / 2f64,
                _ => 0f64,
            }
        }

        x += dx;
        y += dy;

        render_children(node, draw_ctx, &mut x, &mut y, false, clipping);
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

struct NodeTRef {
    link: RefCell<Option<String>>,
}

impl NodeTRef {
    fn new() -> NodeTRef {
        NodeTRef {
            link: RefCell::new(Default::default()),
        }
    }

    fn measure(&self, draw_ctx: *const RsvgDrawingCtx, length: &mut f64) -> bool {
        let l = self.link.borrow();

        if l.is_none() {
            return false;
        }

        let done =
            if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
                let c = acquired.get();
                measure_children(&c, draw_ctx, length, true)
            } else {
                false
            };

        done
    }

    fn render(&self, draw_ctx: *mut RsvgDrawingCtx, x: &mut f64, y: &mut f64, clipping: bool) {
        let l = self.link.borrow();

        if l.is_none() {
            return;
        }

        if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
            let c = acquired.get();
            render_children(&c, draw_ctx, x, y, true, clipping)
        }
    }
}

impl NodeTrait for NodeTRef {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

struct NodeTSpan {
    x: Cell<Option<RsvgLength>>,
    y: Cell<Option<RsvgLength>>,
    dx: Cell<RsvgLength>,
    dy: Cell<RsvgLength>,
}

impl NodeTSpan {
    fn new() -> NodeTSpan {
        NodeTSpan {
            x: Cell::new(Default::default()),
            y: Cell::new(Default::default()),
            dx: Cell::new(RsvgLength::default()),
            dy: Cell::new(RsvgLength::default()),
        }
    }

    fn measure(
        &self,
        node: &RsvgNode,
        draw_ctx: *const RsvgDrawingCtx,
        length: &mut f64,
        usetextonly: bool,
    ) -> bool {
        if self.x.get().is_some() || self.y.get().is_some() {
            return true;
        }

        let state = drawing_ctx::get_current_state(draw_ctx);
        let gravity = state::get_text_gravity(state);
        if gravity_is_vertical(gravity) {
            *length += self.dy.get().normalize(draw_ctx);
        } else {
            *length += self.dx.get().normalize(draw_ctx);
        }

        measure_children(node, draw_ctx, length, usetextonly)
    }

    fn render(
        &self,
        node: &RsvgNode,
        draw_ctx: *mut RsvgDrawingCtx,
        x: &mut f64,
        y: &mut f64,
        usetextonly: bool,
        clipping: bool,
    ) {
        drawing_ctx::state_push(draw_ctx);
        drawing_ctx::state_reinherit_top(draw_ctx, node.get_state(), 0);

        let mut dx = self.dx.get().normalize(draw_ctx);
        let mut dy = self.dy.get().normalize(draw_ctx);

        let state = drawing_ctx::get_current_state(draw_ctx);
        let anchor = state::get_state_rust(state).text_anchor.unwrap_or_default();
        let gravity = state::get_text_gravity(state);

        let offset = anchor_offset(node, draw_ctx, anchor, usetextonly);

        if let Some(self_x) = self.x.get() {
            *x = self_x.normalize(draw_ctx);
            if !gravity_is_vertical(gravity) {
                *x -= offset;
                dx = match anchor {
                    TextAnchor::Start => dx,
                    TextAnchor::Middle => dx / 2f64,
                    _ => 0f64,
                }
            }
        }
        *x += dx;

        if let Some(self_y) = self.y.get() {
            *y = self_y.normalize(draw_ctx);
            if gravity_is_vertical(gravity) {
                *y -= offset;
                dy = match anchor {
                    TextAnchor::Start => dy,
                    TextAnchor::Middle => dy / 2f64,
                    _ => 0f64,
                }
            }
        }
        *y += dy;

        render_children(node, draw_ctx, x, y, usetextonly, clipping);

        drawing_ctx::state_pop(draw_ctx);
    }
}

impl NodeTrait for NodeTSpan {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x
                    .set(parse("x", value, LengthDir::Horizontal, None).map(Some)?),
                Attribute::Y => self.y
                    .set(parse("y", value, LengthDir::Vertical, None).map(Some)?),
                Attribute::Dx => self.dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
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

    if let Some(XmlLang(ref lang)) = state::get_state_rust(state).xml_lang {
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

    if let Some(LetterSpacing(ref ls)) = state::get_state_rust(state).letter_spacing {
        attr_list.insert(
            pango::Attribute::new_letter_spacing(to_pango_units(ls.normalize(draw_ctx))).unwrap(),
        );
    }

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

    let t = xml_space_normalize(
        state::get_state_rust(state).xml_space.unwrap_or_default(),
        text,
    );
    layout.set_text(&t);

    layout
}

fn anchor_offset(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    anchor: TextAnchor,
    textonly: bool,
) -> f64 {
    let mut offset = 0f64;

    match anchor {
        TextAnchor::Start => {}
        TextAnchor::Middle => {
            measure_children(node, draw_ctx, &mut offset, textonly);
            offset /= 2f64;
        }
        _ => {
            measure_children(node, draw_ctx, &mut offset, textonly);
        }
    }

    offset
}

fn measure_children(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let mut done = false;

    for child in node.children() {
        done = measure_child(&child, draw_ctx, length, textonly);
        if done {
            break;
        }
    }

    done
}

fn measure_child(
    node: &RsvgNode,
    draw_ctx: *const RsvgDrawingCtx,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let mut done = false;

    drawing_ctx::state_push(draw_ctx);
    drawing_ctx::state_reinherit_top(draw_ctx, node.get_state(), 0);

    match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            node.with_impl(|chars: &NodeChars| chars.measure(draw_ctx, length));
        }
        (_, true) => {
            done = measure_children(node, draw_ctx, length, textonly);
        }
        (NodeType::TSpan, _) => {
            node.with_impl(|tspan: &NodeTSpan| {
                done = tspan.measure(node, draw_ctx, length, textonly);
            });
        }
        (NodeType::TRef, _) => {
            node.with_impl(|tref: &NodeTRef| {
                done = tref.measure(draw_ctx, length);
            });
        }
        (_, _) => {}
    }

    drawing_ctx::state_pop(draw_ctx);

    done
}

fn render_children(
    node: &RsvgNode,
    draw_ctx: *mut RsvgDrawingCtx,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) {
    drawing_ctx::push_discrete_layer(draw_ctx, clipping);

    for child in node.children() {
        render_child(&child, draw_ctx, x, y, textonly, clipping);
    }

    drawing_ctx::pop_discrete_layer(draw_ctx, clipping);
}

fn render_child(
    node: &RsvgNode,
    draw_ctx: *mut RsvgDrawingCtx,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) {
    match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            node.with_impl(|chars: &NodeChars| chars.render(draw_ctx, x, y, clipping));
        }
        (_, true) => {
            render_children(node, draw_ctx, x, y, textonly, clipping);
        }
        (NodeType::TSpan, _) => {
            node.with_impl(|tspan: &NodeTSpan| {
                tspan.render(node, draw_ctx, x, y, textonly, clipping);
            });
        }
        (NodeType::TRef, _) => {
            node.with_impl(|tref: &NodeTRef| {
                tref.render(draw_ctx, x, y, clipping);
            });
        }
        (_, _) => {}
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_new(raw_parent: *const RsvgNode) -> *const RsvgNode {
    let node = boxed_node_new(NodeType::Chars, raw_parent, Box::new(NodeChars::new()));

    let state = rsvg_node_get_state(node);
    state::set_cond_true(state, false);

    node
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_append(
    raw_node: *const RsvgNode,
    text: *const libc::c_char,
    len: isize,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

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
pub extern "C" fn rsvg_node_text_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::Text, raw_parent, Box::new(NodeText::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_tref_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::TRef, raw_parent, Box::new(NodeTRef::new()))
}

#[no_mangle]
pub extern "C" fn rsvg_node_tspan_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(NodeType::TSpan, raw_parent, Box::new(NodeTSpan::new()))
}
