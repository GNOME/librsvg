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
use node::{boxed_node_new, NodeResult, NodeTrait, NodeType, RsvgCNodeImpl, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;
use space::xml_space_normalize;
use state::{
    ComputedValues,
    Direction,
    FontStretch,
    FontStyle,
    FontVariant,
    FontWeight,
    TextAnchor,
    UnicodeBidi,
    WritingMode,
};

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

    fn measure(
        &self,
        _node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *const RsvgDrawingCtx,
        length: &mut f64,
    ) {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, values, &s);
        let (width, _) = layout.get_size();

        *length = f64::from(width) / f64::from(pango::SCALE);
    }

    fn render(
        &self,
        _node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        x: &mut f64,
        y: &mut f64,
        clipping: bool,
    ) {
        let s = self.string.borrow();
        let layout = create_pango_layout(draw_ctx, values, &s);
        let (width, _) = layout.get_size();

        let baseline = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);
        let offset = baseline + values.baseline_shift.0.normalize(values, draw_ctx);

        if values.text_gravity_is_vertical() {
            draw_pango_layout(draw_ctx, values, &layout, *x + offset, *y, clipping);
            *y += f64::from(width) / f64::from(pango::SCALE);
        } else {
            draw_pango_layout(draw_ctx, values, &layout, *x, *y - offset, clipping);
            *x += f64::from(width) / f64::from(pango::SCALE);
        }
    }
}

impl NodeTrait for NodeChars {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag) -> NodeResult {
        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: &ComputedValues, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
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
                Attribute::Dx => self
                    .dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        _dominate: i32,
        clipping: bool,
    ) {
        let mut x = self.x.get().normalize(values, draw_ctx);
        let mut y = self.y.get().normalize(values, draw_ctx);
        let mut dx = self.dx.get().normalize(values, draw_ctx);
        let mut dy = self.dy.get().normalize(values, draw_ctx);

        let anchor = values.text_anchor;

        let offset = anchor_offset(node, values, draw_ctx, anchor, false);

        if values.text_gravity_is_vertical() {
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

        render_children(node, values, draw_ctx, &mut x, &mut y, false, clipping);
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

    fn measure(
        &self,
        _node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        length: &mut f64,
    ) -> bool {
        let l = self.link.borrow();

        if l.is_none() {
            return false;
        }

        let done =
            if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
                let c = acquired.get();
                measure_children(&c, values, draw_ctx, length, true)
            } else {
                false
            };

        done
    }

    fn render(
        &self,
        _node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        x: &mut f64,
        y: &mut f64,
        clipping: bool,
    ) {
        let l = self.link.borrow();

        if l.is_none() {
            return;
        }

        if let Some(acquired) = drawing_ctx::get_acquired_node(draw_ctx, l.as_ref().unwrap()) {
            let c = acquired.get();
            render_children(&c, values, draw_ctx, x, y, true, clipping)
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

    fn draw(&self, _: &RsvgNode, _: &ComputedValues, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
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
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        length: &mut f64,
        usetextonly: bool,
    ) -> bool {
        if self.x.get().is_some() || self.y.get().is_some() {
            return true;
        }

        if values.text_gravity_is_vertical() {
            *length += self.dy.get().normalize(values, draw_ctx);
        } else {
            *length += self.dx.get().normalize(values, draw_ctx);
        }

        measure_children(node, values, draw_ctx, length, usetextonly)
    }

    fn render(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: *mut RsvgDrawingCtx,
        x: &mut f64,
        y: &mut f64,
        usetextonly: bool,
        clipping: bool,
    ) {
        drawing_ctx::state_reinherit_top(draw_ctx, node.get_state(), 0);

        let mut dx = self.dx.get().normalize(values, draw_ctx);
        let mut dy = self.dy.get().normalize(values, draw_ctx);

        let vertical = values.text_gravity_is_vertical();
        let anchor = values.text_anchor;

        let offset = anchor_offset(node, values, draw_ctx, anchor, usetextonly);

        if let Some(self_x) = self.x.get() {
            *x = self_x.normalize(values, draw_ctx);
            if !vertical {
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
            *y = self_y.normalize(values, draw_ctx);
            if vertical {
                *y -= offset;
                dy = match anchor {
                    TextAnchor::Start => dy,
                    TextAnchor::Middle => dy / 2f64,
                    _ => 0f64,
                }
            }
        }
        *y += dy;

        render_children(node, values, draw_ctx, x, y, usetextonly, clipping);
    }
}

impl NodeTrait for NodeTSpan {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self
                    .x
                    .set(parse("x", value, LengthDir::Horizontal, None).map(Some)?),
                Attribute::Y => self
                    .y
                    .set(parse("y", value, LengthDir::Vertical, None).map(Some)?),
                Attribute::Dx => self
                    .dx
                    .set(parse("dx", value, LengthDir::Horizontal, None)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical, None)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: &ComputedValues, _: *mut RsvgDrawingCtx, _: i32, _: bool) {
        // nothing
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

fn to_pango_units(v: f64) -> i32 {
    (v * f64::from(pango::SCALE)) as i32
}

impl From<FontStyle> for pango::Style {
    fn from(s: FontStyle) -> pango::Style {
        match s {
            FontStyle::Normal => pango::Style::Normal,
            FontStyle::Italic => pango::Style::Italic,
            FontStyle::Oblique => pango::Style::Oblique,
        }
    }
}

impl From<FontVariant> for pango::Variant {
    fn from(v: FontVariant) -> pango::Variant {
        match v {
            FontVariant::Normal => pango::Variant::Normal,
            FontVariant::SmallCaps => pango::Variant::SmallCaps,
        }
    }
}

impl From<FontStretch> for pango::Stretch {
    fn from(s: FontStretch) -> pango::Stretch {
        match s {
            FontStretch::Normal => pango::Stretch::Normal,
            FontStretch::Wider => pango::Stretch::Expanded, // not quite correct
            FontStretch::Narrower => pango::Stretch::Condensed, // not quite correct
            FontStretch::UltraCondensed => pango::Stretch::UltraCondensed,
            FontStretch::ExtraCondensed => pango::Stretch::ExtraCondensed,
            FontStretch::Condensed => pango::Stretch::Condensed,
            FontStretch::SemiCondensed => pango::Stretch::SemiCondensed,
            FontStretch::SemiExpanded => pango::Stretch::SemiExpanded,
            FontStretch::Expanded => pango::Stretch::Expanded,
            FontStretch::ExtraExpanded => pango::Stretch::ExtraExpanded,
            FontStretch::UltraExpanded => pango::Stretch::UltraExpanded,
        }
    }
}

impl From<FontWeight> for pango::Weight {
    fn from(w: FontWeight) -> pango::Weight {
        match w {
            FontWeight::Normal => pango::Weight::Normal,
            FontWeight::Bold => pango::Weight::Bold,
            FontWeight::Bolder => pango::Weight::Ultrabold,
            FontWeight::Lighter => pango::Weight::Light,
            FontWeight::W100 => pango::Weight::Thin,
            FontWeight::W200 => pango::Weight::Ultralight,
            FontWeight::W300 => pango::Weight::Semilight,
            FontWeight::W400 => pango::Weight::Normal,
            FontWeight::W500 => pango::Weight::Medium,
            FontWeight::W600 => pango::Weight::Semibold,
            FontWeight::W700 => pango::Weight::Bold,
            FontWeight::W800 => pango::Weight::Ultrabold,
            FontWeight::W900 => pango::Weight::Heavy,
        }
    }
}

impl From<Direction> for pango::Direction {
    fn from(d: Direction) -> pango::Direction {
        match d {
            Direction::Ltr => pango::Direction::Ltr,
            Direction::Rtl => pango::Direction::Rtl,
        }
    }
}

impl From<Direction> for pango::Alignment {
    fn from(d: Direction) -> pango::Alignment {
        match d {
            Direction::Ltr => pango::Alignment::Left,
            Direction::Rtl => pango::Alignment::Right,
        }
    }
}

impl From<WritingMode> for pango::Direction {
    fn from(m: WritingMode) -> pango::Direction {
        match m {
            WritingMode::LrTb | WritingMode::Lr | WritingMode::Tb | WritingMode::TbRl => {
                pango::Direction::Ltr
            }
            WritingMode::RlTb | WritingMode::Rl => pango::Direction::Rtl,
        }
    }
}

impl From<WritingMode> for pango::Gravity {
    fn from(m: WritingMode) -> pango::Gravity {
        match m {
            WritingMode::Tb | WritingMode::TbRl => pango::Gravity::East,
            WritingMode::LrTb | WritingMode::Lr | WritingMode::RlTb | WritingMode::Rl => {
                pango::Gravity::South
            }
        }
    }
}

fn create_pango_layout(
    draw_ctx: *const RsvgDrawingCtx,
    values: &ComputedValues,
    text: &str,
) -> pango::Layout {
    let pango_context = drawing_ctx::get_pango_context(draw_ctx);

    // See the construction of the XmlLang property
    // We use "" there as the default value; this means that the language is not set.
    // If the language *is* set, we can use it here.
    if !values.xml_lang.0.is_empty() {
        let pango_lang = pango::Language::from_string(&values.xml_lang.0);
        pango_context.set_language(&pango_lang);
    }

    pango_context.set_base_gravity(pango::Gravity::from(values.writing_mode));

    match (values.unicode_bidi, values.direction) {
        (UnicodeBidi::Override, _) | (UnicodeBidi::Embed, _) => {
            pango_context.set_base_dir(pango::Direction::from(values.direction));
        }

        (_, direction) if direction != Direction::Ltr => {
            pango_context.set_base_dir(pango::Direction::from(direction));
        }

        (_, _) => {
            pango_context.set_base_dir(pango::Direction::from(values.writing_mode));
        }
    }

    let mut font_desc = pango_context.get_font_description().unwrap();

    font_desc.set_family(&values.font_family.0);

    font_desc.set_style(pango::Style::from(values.font_style));

    font_desc.set_variant(pango::Variant::from(values.font_variant));

    font_desc.set_weight(pango::Weight::from(values.font_weight));

    font_desc.set_stretch(pango::Stretch::from(values.font_stretch));

    let (_, dpi_y) = drawing_ctx::get_dpi(draw_ctx);
    font_desc.set_size(to_pango_units(
        values.font_size.0.normalize(values, draw_ctx) / dpi_y * 72.0,
    ));

    let layout = pango::Layout::new(&pango_context);
    layout.set_font_description(&font_desc);

    let attr_list = pango::AttrList::new();

    attr_list.insert(
        pango::Attribute::new_letter_spacing(to_pango_units(
            values.letter_spacing.0.normalize(values, draw_ctx),
        )).unwrap(),
    );

    if values.text_decoration.underline {
        attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single).unwrap());
    }

    if values.text_decoration.strike {
        attr_list.insert(pango::Attribute::new_strikethrough(true).unwrap());
    }

    layout.set_attributes(&attr_list);

    layout.set_alignment(pango::Alignment::from(values.direction));

    let t = xml_space_normalize(values.xml_space, text);
    layout.set_text(&t);

    layout
}

fn anchor_offset(
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: *mut RsvgDrawingCtx,
    anchor: TextAnchor,
    textonly: bool,
) -> f64 {
    let mut offset = 0f64;

    match anchor {
        TextAnchor::Start => {}
        TextAnchor::Middle => {
            measure_children(node, values, draw_ctx, &mut offset, textonly);
            offset /= 2f64;
        }
        _ => {
            measure_children(node, values, draw_ctx, &mut offset, textonly);
        }
    }

    offset
}

fn measure_children(
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: *mut RsvgDrawingCtx,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let mut done = false;

    for child in node.children() {
        done = measure_child(&child, values, draw_ctx, length, textonly);
        if done {
            break;
        }
    }

    done
}

fn measure_child(
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: *mut RsvgDrawingCtx,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let mut done = false;

    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    cr.save();

    cr.transform(node.get_transform());

    // FIXME: if we are inside a <use>, look at the dominate and
    // cascade, otherwise use the node's computed values
    let child_values = &node.get_computed_values();

    match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            // here we use the values from the current element,
            // instead of child_values because NodeChars does not
            // represent a real SVG element - it is just our container
            // for character data.
            node.with_impl(|chars: &NodeChars| chars.measure(node, values, draw_ctx, length));
        }
        (_, true) => {
            done = measure_children(node, child_values, draw_ctx, length, textonly);
        }
        (NodeType::TSpan, _) => {
            node.with_impl(|tspan: &NodeTSpan| {
                done = tspan.measure(node, child_values, draw_ctx, length, textonly);
            });
        }
        (NodeType::TRef, _) => {
            node.with_impl(|tref: &NodeTRef| {
                done = tref.measure(node, child_values, draw_ctx, length);
            });
        }
        (_, _) => {}
    }

    cr.restore();

    done
}

fn render_children(
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: *mut RsvgDrawingCtx,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) {
    drawing_ctx::push_discrete_layer(draw_ctx, values, clipping);

    for child in node.children() {
        render_child(&child, values, draw_ctx, x, y, textonly, clipping);
    }

    drawing_ctx::pop_discrete_layer(draw_ctx, values, clipping);
}

fn render_child(
    node: &RsvgNode,
    values: &ComputedValues,
    draw_ctx: *mut RsvgDrawingCtx,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) {
    let cr = drawing_ctx::get_cairo_context(draw_ctx);
    cr.save();

    cr.transform(node.get_transform());

    // FIXME: if we are inside a <use>, look at the dominate and
    // cascade, otherwise use the node's computed values
    let child_values = &node.get_computed_values();

    match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            node.with_impl(|chars: &NodeChars| {
                // here we use the values from the current element,
                // instead of child_values because NodeChars does not
                // represent a real SVG element - it is just our container
                // for character data.
                chars.render(node, values, draw_ctx, x, y, clipping)
            });
        }
        (_, true) => {
            render_children(node, child_values, draw_ctx, x, y, textonly, clipping);
        }
        (NodeType::TSpan, _) => {
            node.with_impl(|tspan: &NodeTSpan| {
                tspan.render(node, child_values, draw_ctx, x, y, textonly, clipping);
            });
        }
        (NodeType::TRef, _) => {
            node.with_impl(|tref: &NodeTRef| {
                tref.render(node, child_values, draw_ctx, x, y, clipping);
            });
        }
        (_, _) => {}
    }

    cr.restore();
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_new(raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new(NodeType::Chars, raw_parent, Box::new(NodeChars::new()))
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
