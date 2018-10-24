use libc;
use pango::{self, ContextExt, LayoutExt};
use std;
use std::cell::{Cell, RefCell};
use std::str;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use font_props::FontWeightSpec;
use handle::RsvgHandle;
use length::*;
use node::{boxed_node_new, CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use parsers::parse;
use property_bag::PropertyBag;
use space::{xml_space_normalize, NormalizeDefault, XmlSpaceNormalize};
use state::{
    ComputedValues,
    Direction,
    FontStretch,
    FontStyle,
    FontVariant,
    TextAnchor,
    UnicodeBidi,
    WritingMode,
    XmlSpace,
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

pub struct NodeChars {
    string: RefCell<String>,
    space_normalized: RefCell<Option<String>>,
}

impl NodeChars {
    fn new() -> NodeChars {
        NodeChars {
            string: RefCell::new(String::new()),
            space_normalized: RefCell::new(None),
        }
    }

    fn append(&self, s: &str) {
        self.string.borrow_mut().push_str(s);
        *self.space_normalized.borrow_mut() = None;
    }

    fn ensure_normalized_string(&self, node: &RsvgNode, values: &ComputedValues) {
        let mut normalized = self.space_normalized.borrow_mut();

        if (*normalized).is_none() {
            let mode = match values.xml_space {
                XmlSpace::Default => XmlSpaceNormalize::Default(NormalizeDefault {
                    has_element_before: node.has_previous_sibling(),
                    has_element_after: node.has_next_sibling(),
                }),

                XmlSpace::Preserve => XmlSpaceNormalize::Preserve,
            };

            *normalized = Some(xml_space_normalize(mode, &self.string.borrow()));
        }
    }

    fn create_layout(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx<'_>,
    ) -> pango::Layout {
        self.ensure_normalized_string(node, values);
        let norm = self.space_normalized.borrow();
        let s = norm.as_ref().unwrap();
        create_pango_layout(draw_ctx, values, &s)
    }

    fn measure(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: &DrawingCtx<'_>,
        length: &mut f64,
    ) {
        let layout = self.create_layout(node, values, draw_ctx);
        let (width, _) = layout.get_size();

        *length = f64::from(width) / f64::from(pango::SCALE);
    }

    fn render(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx<'_>,
        x: &mut f64,
        y: &mut f64,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let layout = self.create_layout(node, values, draw_ctx);
        let (width, _) = layout.get_size();

        let baseline = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);
        let offset = baseline
            + values
                .baseline_shift
                .0
                .normalize(values, &draw_ctx.get_view_params());

        if values.text_gravity_is_vertical() {
            draw_ctx.draw_pango_layout(&layout, values, *x + offset, *y, clipping)?;
            *y += f64::from(width) / f64::from(pango::SCALE);
        } else {
            draw_ctx.draw_pango_layout(&layout, values, *x, *y - offset, clipping)?;
            *x += f64::from(width) / f64::from(pango::SCALE);
        }

        Ok(())
    }
}

impl NodeTrait for NodeChars {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }
}

pub struct NodeText {
    x: Cell<Length>,
    y: Cell<Length>,
    dx: Cell<Length>,
    dy: Cell<Length>,
}

impl NodeText {
    pub fn new() -> NodeText {
        NodeText {
            x: Cell::new(Length::default()),
            y: Cell::new(Length::default()),
            dx: Cell::new(Length::default()),
            dy: Cell::new(Length::default()),
        }
    }
}

impl NodeTrait for NodeText {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),
                Attribute::Dx => self.dx.set(parse("dx", value, LengthDir::Horizontal)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn accept_chars(&self) -> bool {
        true
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let mut x = self.x.get().normalize(values, &params);
        let mut y = self.y.get().normalize(values, &params);
        let mut dx = self.dx.get().normalize(values, &params);
        let mut dy = self.dy.get().normalize(values, &params);

        let anchor = values.text_anchor;

        let offset = anchor_offset(node, cascaded, draw_ctx, anchor, false);

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

        render_children(node, cascaded, draw_ctx, &mut x, &mut y, false, clipping)
    }
}

pub struct NodeTRef {
    link: RefCell<Option<String>>,
}

impl NodeTRef {
    pub fn new() -> NodeTRef {
        NodeTRef {
            link: RefCell::new(Default::default()),
        }
    }

    fn measure(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        length: &mut f64,
    ) -> bool {
        let l = self.link.borrow();

        if l.is_none() {
            return false;
        }

        let url = l.as_ref().unwrap();

        let done = if let Some(acquired) = draw_ctx.get_acquired_node(url) {
            let c = acquired.get();
            measure_children(&c, cascaded, draw_ctx, length, true)
        } else {
            rsvg_log!(
                "element {} references a nonexistent text source \"{}\"",
                node.get_human_readable_name(),
                url,
            );
            false
        };

        done
    }

    fn render(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        x: &mut f64,
        y: &mut f64,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let l = self.link.borrow();

        if l.is_none() {
            return Ok(());
        }

        let url = l.as_ref().unwrap();

        if let Some(acquired) = draw_ctx.get_acquired_node(url) {
            let c = acquired.get();
            render_children(&c, cascaded, draw_ctx, x, y, true, clipping)?;
        } else {
            rsvg_log!(
                "element {} references a nonexistent text source \"{}\"",
                node.get_human_readable_name(),
                url,
            );
        }

        Ok(())
    }
}

impl NodeTrait for NodeTRef {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }
}

pub struct NodeTSpan {
    x: Cell<Option<Length>>,
    y: Cell<Option<Length>>,
    dx: Cell<Length>,
    dy: Cell<Length>,
}

impl NodeTSpan {
    pub fn new() -> NodeTSpan {
        NodeTSpan {
            x: Cell::new(Default::default()),
            y: Cell::new(Default::default()),
            dx: Cell::new(Length::default()),
            dy: Cell::new(Length::default()),
        }
    }

    fn measure(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        length: &mut f64,
        usetextonly: bool,
    ) -> bool {
        let values = cascaded.get();

        if self.x.get().is_some() || self.y.get().is_some() {
            return true;
        }

        let params = draw_ctx.get_view_params();

        if values.text_gravity_is_vertical() {
            *length += self.dy.get().normalize(values, &params);
        } else {
            *length += self.dx.get().normalize(values, &params);
        }

        measure_children(node, cascaded, draw_ctx, length, usetextonly)
    }

    fn render(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        x: &mut f64,
        y: &mut f64,
        usetextonly: bool,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let mut dx = self.dx.get().normalize(values, &params);
        let mut dy = self.dy.get().normalize(values, &params);

        let vertical = values.text_gravity_is_vertical();
        let anchor = values.text_anchor;

        let offset = anchor_offset(node, cascaded, draw_ctx, anchor, usetextonly);

        if let Some(self_x) = self.x.get() {
            *x = self_x.normalize(values, &params);
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
            *y = self_y.normalize(values, &params);
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

        render_children(node, cascaded, draw_ctx, x, y, usetextonly, clipping)
    }
}

impl NodeTrait for NodeTSpan {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self
                    .x
                    .set(parse("x", value, LengthDir::Horizontal).map(Some)?),
                Attribute::Y => self
                    .y
                    .set(parse("y", value, LengthDir::Vertical).map(Some)?),
                Attribute::Dx => self.dx.set(parse("dx", value, LengthDir::Horizontal)?),
                Attribute::Dy => self.dy.set(parse("dy", value, LengthDir::Vertical)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn accept_chars(&self) -> bool {
        true
    }
}

fn to_pango_units(v: f64) -> i32 {
    (v * f64::from(pango::SCALE) + 0.5) as i32
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

impl From<FontWeightSpec> for pango::Weight {
    fn from(w: FontWeightSpec) -> pango::Weight {
        match w {
            FontWeightSpec::Normal => pango::Weight::Normal,
            FontWeightSpec::Bold => pango::Weight::Bold,
            FontWeightSpec::Bolder => pango::Weight::Ultrabold,
            FontWeightSpec::Lighter => pango::Weight::Light,
            FontWeightSpec::W100 => pango::Weight::Thin,
            FontWeightSpec::W200 => pango::Weight::Ultralight,
            FontWeightSpec::W300 => pango::Weight::Semilight,
            FontWeightSpec::W400 => pango::Weight::Normal,
            FontWeightSpec::W500 => pango::Weight::Medium,
            FontWeightSpec::W600 => pango::Weight::Semibold,
            FontWeightSpec::W700 => pango::Weight::Bold,
            FontWeightSpec::W800 => pango::Weight::Ultrabold,
            FontWeightSpec::W900 => pango::Weight::Heavy,
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
    draw_ctx: &DrawingCtx<'_>,
    values: &ComputedValues,
    text: &str,
) -> pango::Layout {
    let pango_context = draw_ctx.get_pango_context();

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

    font_desc.set_family(&(values.font_family.0).0);

    font_desc.set_style(pango::Style::from(values.font_style));

    font_desc.set_variant(pango::Variant::from(values.font_variant));

    font_desc.set_weight(pango::Weight::from(values.font_weight.0));

    font_desc.set_stretch(pango::Stretch::from(values.font_stretch));

    let params = draw_ctx.get_view_params();

    font_desc.set_size(to_pango_units(
        values.font_size.0.normalize(values, &params),
    ));

    let layout = pango::Layout::new(&pango_context);
    layout.set_font_description(&font_desc);

    let attr_list = pango::AttrList::new();

    attr_list.insert(
        pango::Attribute::new_letter_spacing(to_pango_units(
            values.letter_spacing.0.normalize(values, &params),
        ))
        .unwrap(),
    );

    if values.text_decoration.underline {
        attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single).unwrap());
    }

    if values.text_decoration.strike {
        attr_list.insert(pango::Attribute::new_strikethrough(true).unwrap());
    }

    layout.set_attributes(&attr_list);

    layout.set_alignment(pango::Alignment::from(values.direction));

    layout.set_text(text);

    layout
}

fn anchor_offset(
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx<'_>,
    anchor: TextAnchor,
    textonly: bool,
) -> f64 {
    let mut offset = 0f64;

    match anchor {
        TextAnchor::Start => {}
        TextAnchor::Middle => {
            measure_children(node, cascaded, draw_ctx, &mut offset, textonly);
            offset /= 2f64;
        }
        _ => {
            measure_children(node, cascaded, draw_ctx, &mut offset, textonly);
        }
    }

    offset
}

fn measure_children(
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx<'_>,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let mut done = false;

    for child in node.children() {
        done = measure_child(
            &child,
            &CascadedValues::new(cascaded, &child),
            draw_ctx,
            length,
            textonly,
        );
        if done {
            break;
        }
    }

    done
}

fn measure_child(
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx<'_>,
    length: &mut f64,
    textonly: bool,
) -> bool {
    let values = cascaded.get();

    let mut done = false;

    let cr = draw_ctx.get_cairo_context();
    cr.save();

    cr.transform(node.get_transform());

    match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            // here we use the values from the current element,
            // instead of child_values because NodeChars does not
            // represent a real SVG element - it is just our container
            // for character data.
            node.with_impl(|chars: &NodeChars| chars.measure(node, values, draw_ctx, length));
        }
        (_, true) => {
            done = measure_children(
                node,
                &CascadedValues::new(cascaded, node),
                draw_ctx,
                length,
                textonly,
            );
        }
        (NodeType::TSpan, _) => {
            node.with_impl(|tspan: &NodeTSpan| {
                done = tspan.measure(
                    node,
                    &CascadedValues::new(cascaded, node),
                    draw_ctx,
                    length,
                    textonly,
                );
            });
        }
        (NodeType::TRef, _) => {
            node.with_impl(|tref: &NodeTRef| {
                done = tref.measure(node, &CascadedValues::new(cascaded, node), draw_ctx, length);
            });
        }
        (_, _) => {}
    }

    cr.restore();

    done
}

fn render_children(
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx<'_>,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) -> Result<(), RenderingError> {
    let values = cascaded.get();

    draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
        for child in node.children() {
            render_child(&child, cascaded, dc, x, y, textonly, clipping)?;
        }

        Ok(())
    })
}

fn render_child(
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx<'_>,
    x: &mut f64,
    y: &mut f64,
    textonly: bool,
    clipping: bool,
) -> Result<(), RenderingError> {
    let values = cascaded.get();

    let cr = draw_ctx.get_cairo_context();
    cr.save();

    cr.transform(node.get_transform());

    let res = match (node.get_type(), textonly) {
        (NodeType::Chars, _) => {
            node.with_impl(|chars: &NodeChars| {
                // here we use the values from the current element,
                // instead of child_values because NodeChars does not
                // represent a real SVG element - it is just our container
                // for character data.
                chars.render(node, values, draw_ctx, x, y, clipping)
            })
        }
        (_, true) => render_children(
            node,
            &CascadedValues::new(cascaded, node),
            draw_ctx,
            x,
            y,
            textonly,
            clipping,
        ),
        (NodeType::TSpan, _) => node.with_impl(|tspan: &NodeTSpan| {
            tspan.render(
                node,
                &CascadedValues::new(cascaded, node),
                draw_ctx,
                x,
                y,
                textonly,
                clipping,
            )
        }),
        (NodeType::TRef, _) => node.with_impl(|tref: &NodeTRef| {
            tref.render(
                node,
                &CascadedValues::new(cascaded, node),
                draw_ctx,
                x,
                y,
                clipping,
            )
        }),
        (_, _) => Ok(()),
    };

    cr.restore();

    res
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_new(raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new(
        NodeType::Chars,
        raw_parent,
        "rsvg_chars",
        None,
        None,
        Box::new(NodeChars::new()),
    )
}

#[no_mangle]
pub extern "C" fn rsvg_node_chars_append(
    raw_node: *const RsvgNode,
    text: *const libc::c_char,
    len: isize,
) {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(node.get_type() == NodeType::Chars);
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
