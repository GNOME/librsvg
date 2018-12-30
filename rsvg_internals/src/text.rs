use pango::{self, ContextExt, LayoutExt};
use std::cell::{Cell, RefCell};

use attributes::Attribute;
use defs::Fragment;
use drawing_ctx::DrawingCtx;
use error::{AttributeResultExt, RenderingError};
use font_props::FontWeightSpec;
use handle::RsvgHandle;
use length::*;
use node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use parsers::ParseValue;
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

/// An absolutely-positioned array of `Span`s
///
/// SVG defines a "[text chunk]" to occur when a text-related element
/// has an absolute position adjustment, that is, `x` or `y`
/// attributes.
///
/// A `<text>` element always starts with an absolute position from
/// such attributes, or (0, 0) if they are not specified.
///
/// Subsequent children of the `<text>` element will create new chunks
/// whenever they have `x` or `y` attributes.
///
/// [text chunk]: https://www.w3.org/TR/SVG11/text.html#TextLayoutIntroduction
struct Chunk {
    values: ComputedValues,
    x: Option<Length>,
    y: Option<Length>,
    spans: Vec<Span>,
}

struct MeasuredChunk {
    values: ComputedValues,
    x: Option<Length>,
    y: Option<Length>,
    advance: (f64, f64),
    spans: Vec<MeasuredSpan>,
}

struct PositionedChunk {
    next_chunk_x: f64,
    next_chunk_y: f64,
    spans: Vec<PositionedSpan>,
}

struct Span {
    values: ComputedValues,
    text: String,
    dx: Option<Length>,
    dy: Option<Length>,
    depth: usize,
}

struct MeasuredSpan {
    values: ComputedValues,
    layout: pango::Layout,
    layout_size: (f64, f64),
    advance: (f64, f64),
    dx: Option<Length>,
    dy: Option<Length>,
}

struct PositionedSpan {
    layout: pango::Layout,
    values: ComputedValues,
    position: (f64, f64),
    rendered_position: (f64, f64),
}

impl Chunk {
    fn new(values: &ComputedValues, x: Option<Length>, y: Option<Length>) -> Chunk {
        Chunk {
            values: values.clone(),
            x,
            y,
            spans: Vec::new(),
        }
    }
}

impl MeasuredChunk {
    fn from_chunk(chunk: &Chunk, draw_ctx: &DrawingCtx) -> MeasuredChunk {
        let measured_spans: Vec<MeasuredSpan> = chunk
            .spans
            .iter()
            .map(|span| MeasuredSpan::from_span(span, draw_ctx))
            .collect();

        let advance = measured_spans.iter().fold((0.0, 0.0), |acc, measured| {
            (acc.0 + measured.advance.0, acc.1 + measured.advance.1)
        });

        MeasuredChunk {
            values: chunk.values.clone(),
            x: chunk.x,
            y: chunk.y,
            advance,
            spans: measured_spans,
        }
    }
}

impl PositionedChunk {
    fn from_measured(
        measured: &MeasuredChunk,
        draw_ctx: &DrawingCtx,
        x: f64,
        y: f64,
    ) -> PositionedChunk {
        let mut positioned = Vec::new();

        // Adjust the specified coordinates with the text_anchor

        let adjusted_advance = text_anchor_advance(
            measured.values.text_anchor,
            measured.values.writing_mode,
            measured.advance,
        );

        let mut x = x + adjusted_advance.0;
        let mut y = y + adjusted_advance.1;

        // Position each span

        for measured_span in &measured.spans {
            positioned.push(PositionedSpan::from_measured(measured_span, draw_ctx, x, y));

            x += measured_span.advance.0;
            y += measured_span.advance.1;
        }

        PositionedChunk {
            next_chunk_x: x,
            next_chunk_y: y,
            spans: positioned,
        }
    }
}

fn text_anchor_advance(
    anchor: TextAnchor,
    writing_mode: WritingMode,
    advance: (f64, f64),
) -> (f64, f64) {
    if writing_mode.is_vertical() {
        match anchor {
            TextAnchor::Start => (0.0, 0.0),
            TextAnchor::Middle => (0.0, -advance.1 / 2.0),
            TextAnchor::End => (0.0, -advance.1),
        }
    } else {
        match anchor {
            TextAnchor::Start => (0.0, 0.0),
            TextAnchor::Middle => (-advance.0 / 2.0, 0.0),
            TextAnchor::End => (-advance.0, 0.0),
        }
    }
}

impl Span {
    fn new(
        text: &str,
        values: ComputedValues,
        dx: Option<Length>,
        dy: Option<Length>,
        depth: usize,
    ) -> Span {
        Span {
            values,
            text: text.to_string(),
            dx,
            dy,
            depth,
        }
    }
}

impl MeasuredSpan {
    fn from_span(span: &Span, draw_ctx: &DrawingCtx) -> MeasuredSpan {
        let values = span.values.clone();

        let layout = create_pango_layout(draw_ctx, &values, &span.text);
        let (w, h) = layout.get_size();

        let w = f64::from(w) / f64::from(pango::SCALE);
        let h = f64::from(h) / f64::from(pango::SCALE);

        let advance = if values.writing_mode.is_vertical() {
            (0.0, w)
        } else {
            (w, 0.0)
        };

        MeasuredSpan {
            values,
            layout,
            layout_size: (w, h),
            advance,
            dx: span.dx,
            dy: span.dy,
        }
    }
}

impl PositionedSpan {
    fn from_measured(
        measured: &MeasuredSpan,
        draw_ctx: &DrawingCtx,
        x: f64,
        y: f64,
    ) -> PositionedSpan {
        let layout = measured.layout.clone();
        let values = measured.values.clone();

        let params = draw_ctx.get_view_params();

        let baseline = f64::from(layout.get_baseline()) / f64::from(pango::SCALE);
        let baseline_shift = values.baseline_shift.0.normalize(&values, &params);
        let offset = baseline + baseline_shift;

        let dx = measured
            .dx
            .map(|l| l.normalize(&values, &params))
            .unwrap_or(0.0);
        let dy = measured
            .dy
            .map(|l| l.normalize(&values, &params))
            .unwrap_or(0.0);

        let (render_x, render_y) = if values.text_gravity_is_vertical() {
            (x + offset + dx, y + dy)
        } else {
            (x + dx, y - offset + dy)
        };

        PositionedSpan {
            layout: measured.layout.clone(),
            values,
            position: (x, y),
            rendered_position: (render_x, render_y),
        }
    }

    fn draw(&self, draw_ctx: &mut DrawingCtx, clipping: bool) -> Result<(), RenderingError> {
        draw_ctx.draw_pango_layout(
            &self.layout,
            &self.values,
            self.rendered_position.0,
            self.rendered_position.1,
            clipping,
        )
    }
}

/// Walks the children of a `<text>`, `<tspan>`, or `<tref>` element
/// and appends chunks/spans from them into the specified `chunks`
/// array.
///
/// `x` and `y` are the absolute position for the first chunk.  If the
/// first child is a `<tspan>` with a specified absolute position, it
/// will be used instead of the given arguments.
fn children_to_chunks(
    chunks: &mut Vec<Chunk>,
    node: &RsvgNode,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx,
    dx: Option<Length>,
    dy: Option<Length>,
    depth: usize,
) {
    for child in node.children() {
        match child.get_type() {
            NodeType::Chars => child.with_impl(|chars: &NodeChars| {
                let values = cascaded.get();
                chars.to_chunks(&child, values, chunks, dx, dy, depth);
            }),

            NodeType::TSpan => child.with_impl(|tspan: &NodeTSpan| {
                let cascaded = CascadedValues::new(cascaded, &child);
                tspan.to_chunks(&child, &cascaded, draw_ctx, chunks, depth + 1);
            }),

            NodeType::TRef => child.with_impl(|tref: &NodeTRef| {
                let cascaded = CascadedValues::new(cascaded, &child);
                tref.to_chunks(&child, &cascaded, draw_ctx, chunks, depth + 1);
            }),

            _ => (),
        }
    }
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

pub struct NodeChars {
    string: RefCell<String>,
    space_normalized: RefCell<Option<String>>,
}

impl NodeChars {
    pub fn new() -> NodeChars {
        NodeChars {
            string: RefCell::new(String::new()),
            space_normalized: RefCell::new(None),
        }
    }

    pub fn get_string(&self) -> String {
        self.string.borrow().clone()
    }

    pub fn append(&self, s: &str) {
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

    fn make_span(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        dx: Option<Length>,
        dy: Option<Length>,
        depth: usize,
    ) -> Span {
        self.ensure_normalized_string(node, values);

        Span::new(
            self.space_normalized.borrow().as_ref().unwrap(),
            values.clone(),
            dx,
            dy,
            depth,
        )
    }

    fn to_chunks(
        &self,
        node: &RsvgNode,
        values: &ComputedValues,
        chunks: &mut Vec<Chunk>,
        dx: Option<Length>,
        dy: Option<Length>,
        depth: usize,
    ) {
        let span = self.make_span(&node, values, dx, dy, depth);

        let num_chunks = chunks.len();
        assert!(num_chunks > 0);

        chunks[num_chunks - 1].spans.push(span);
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
    dx: Cell<Option<Length>>,
    dy: Cell<Option<Length>>,
}

impl NodeText {
    pub fn new() -> NodeText {
        NodeText {
            x: Cell::new(Length::default()),
            y: Cell::new(Length::default()),
            dx: Cell::new(None),
            dy: Cell::new(None),
        }
    }

    fn make_chunks(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();

        let x = self.x.get();
        let y = self.y.get();
        let dx = self.dx.get();
        let dy = self.dy.get();

        chunks.push(Chunk::new(cascaded.get(), Some(x), Some(y)));

        children_to_chunks(&mut chunks, node, cascaded, draw_ctx, dx, dy, 0);
        chunks
    }
}

impl NodeTrait for NodeText {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(attr.parse(value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(attr.parse(value, LengthDir::Vertical)?),
                Attribute::Dx => self
                    .dx
                    .set(attr.parse(value, LengthDir::Horizontal).map(Some)?),
                Attribute::Dy => self
                    .dy
                    .set(attr.parse(value, LengthDir::Vertical).map(Some)?),
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
        let values = cascaded.get();
        let params = draw_ctx.get_view_params();

        let mut x = self.x.get().normalize(values, &params);
        let mut y = self.y.get().normalize(values, &params);

        let chunks = self.make_chunks(node, cascaded, draw_ctx);

        let mut measured_chunks = Vec::new();
        for chunk in &chunks {
            measured_chunks.push(MeasuredChunk::from_chunk(chunk, draw_ctx));
        }

        let mut positioned_chunks = Vec::new();
        for chunk in &measured_chunks {
            let normalize = |l: Length| l.normalize(&chunk.values, &params);

            let chunk_x = chunk.x.map_or(x, normalize);
            let chunk_y = chunk.y.map_or(y, normalize);

            let positioned = PositionedChunk::from_measured(&chunk, draw_ctx, chunk_x, chunk_y);

            x = positioned.next_chunk_x;
            y = positioned.next_chunk_y;

            positioned_chunks.push(positioned);
        }

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            for chunk in &positioned_chunks {
                for span in &chunk.spans {
                    span.draw(dc, clipping)?;
                }
            }

            Ok(())
        })
    }
}

pub struct NodeTRef {
    link: RefCell<Option<Fragment>>,
}

impl NodeTRef {
    pub fn new() -> NodeTRef {
        NodeTRef {
            link: RefCell::new(Default::default()),
        }
    }

    fn to_chunks(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        chunks: &mut Vec<Chunk>,
        depth: usize,
    ) {
        let link = self.link.borrow();

        if link.is_none() {
            return;
        }

        let link = link.as_ref().unwrap();

        let values = cascaded.get();

        if let Some(acquired) = draw_ctx.get_acquired_node(link) {
            let c = acquired.get();
            extract_chars_children_to_chunks_recursively(chunks, &c, values, depth);
        } else {
            rsvg_log!(
                "element {} references a nonexistent text source \"{}\"",
                node.get_human_readable_name(),
                link,
            );
        }
    }
}

fn extract_chars_children_to_chunks_recursively(
    chunks: &mut Vec<Chunk>,
    node: &RsvgNode,
    values: &ComputedValues,
    depth: usize,
) {
    for child in node.children() {
        match child.get_type() {
            NodeType::Chars => child.with_impl(|chars: &NodeChars| {
                chars.to_chunks(&child, values, chunks, None, None, depth);
            }),

            _ => extract_chars_children_to_chunks_recursively(chunks, &child, values, depth + 1),
        }
    }
}

impl NodeTrait for NodeTRef {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => {
                    *self.link.borrow_mut() =
                        Some(Fragment::parse(value).attribute(Attribute::XlinkHref)?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}

pub struct NodeTSpan {
    x: Cell<Option<Length>>,
    y: Cell<Option<Length>>,
    dx: Cell<Option<Length>>,
    dy: Cell<Option<Length>>,
}

impl NodeTSpan {
    pub fn new() -> NodeTSpan {
        NodeTSpan {
            x: Cell::new(Default::default()),
            y: Cell::new(Default::default()),
            dx: Cell::new(None),
            dy: Cell::new(None),
        }
    }

    fn to_chunks(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        chunks: &mut Vec<Chunk>,
        depth: usize,
    ) {
        let x = self.x.get();
        let y = self.y.get();
        let dx = self.dx.get();
        let dy = self.dy.get();

        if x.is_some() || y.is_some() {
            // Any absolute position creates a new chunk
            let values = cascaded.get();
            chunks.push(Chunk::new(values, x, y));
        }

        children_to_chunks(chunks, node, cascaded, draw_ctx, dx, dy, depth);
    }
}

impl NodeTrait for NodeTSpan {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self
                    .x
                    .set(attr.parse(value, LengthDir::Horizontal).map(Some)?),
                Attribute::Y => self
                    .y
                    .set(attr.parse(value, LengthDir::Vertical).map(Some)?),
                Attribute::Dx => self
                    .dx
                    .set(attr.parse(value, LengthDir::Horizontal).map(Some)?),
                Attribute::Dy => self
                    .dy
                    .set(attr.parse(value, LengthDir::Vertical).map(Some)?),
                _ => (),
            }
        }

        Ok(())
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
    draw_ctx: &DrawingCtx,
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
