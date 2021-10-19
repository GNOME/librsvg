//! Text elements: `text`, `tspan`, `tref`.

use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::cell::RefCell;
use std::rc::Rc;

use crate::bbox::BoundingBox;
use crate::document::{AcquiredNodes, NodeId};
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::layout::{self, FontProperties, StackingContext, Stroke, TextSpan};
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::paint_server::PaintSource;
use crate::parsers::ParseValue;
use crate::properties::{
    ComputedValues, Direction, FontStretch, FontStyle, FontVariant, FontWeight, PaintOrder,
    TextAnchor, TextRendering, UnicodeBidi, WritingMode, XmlLang, XmlSpace,
};
use crate::rect::Rect;
use crate::space::{xml_space_normalize, NormalizeDefault, XmlSpaceNormalize};
use crate::transform::Transform;
use crate::xml::Attributes;

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
    values: Rc<ComputedValues>,
    x: Option<f64>,
    y: Option<f64>,
    spans: Vec<Span>,
    link: Option<String>,
}

struct MeasuredChunk {
    values: Rc<ComputedValues>,
    x: Option<f64>,
    y: Option<f64>,
    spans: Vec<MeasuredSpan>,
    link: Option<String>,
}

struct PositionedChunk {
    next_chunk_x: f64,
    next_chunk_y: f64,
    spans: Vec<PositionedSpan>,
    link: Option<String>,
}

struct Span {
    values: Rc<ComputedValues>,
    text: String,
    dx: f64,
    dy: f64,
    _depth: usize,
}

struct MeasuredSpan {
    values: Rc<ComputedValues>,
    layout: pango::Layout,
    _layout_size: (f64, f64),
    advance: (f64, f64),
    dx: f64,
    dy: f64,
}

struct PositionedSpan {
    layout: pango::Layout,
    values: Rc<ComputedValues>,
    rendered_position: (f64, f64),
}

/// A laid-out and resolved text span.
///
/// The only thing not in user-space units are the `stroke_paint` and `fill_paint`.
///
/// This is the non-user-space version of `layout::TextSpan`.
struct LayoutSpan {
    layout: pango::Layout,
    gravity: pango::Gravity,
    bbox: Option<BoundingBox>,
    is_visible: bool,
    x: f64,
    y: f64,
    paint_order: PaintOrder,
    stroke: Stroke,
    stroke_paint: PaintSource,
    fill_paint: PaintSource,
    text_rendering: TextRendering,
    link_target: Option<String>,
    values: Rc<ComputedValues>,
}

impl Chunk {
    fn new(values: &ComputedValues, x: Option<f64>, y: Option<f64>, link: Option<String>) -> Chunk {
        Chunk {
            values: Rc::new(values.clone()),
            x,
            y,
            link,
            spans: Vec::new(),
        }
    }
}

impl MeasuredChunk {
    fn from_chunk(
        chunk: &Chunk,
        text_writing_mode: WritingMode,
        draw_ctx: &DrawingCtx,
    ) -> MeasuredChunk {
        let measured_spans: Vec<MeasuredSpan> = chunk
            .spans
            .iter()
            .map(|span| MeasuredSpan::from_span(span, text_writing_mode, draw_ctx))
            .collect();

        MeasuredChunk {
            values: chunk.values.clone(),
            x: chunk.x,
            y: chunk.y,
            spans: measured_spans,
            link: chunk.link.clone(),
        }
    }
}

impl PositionedChunk {
    fn from_measured(
        measured: &MeasuredChunk,
        view_params: &ViewParams,
        text_writing_mode: WritingMode,
        x: f64,
        y: f64,
    ) -> PositionedChunk {
        let mut positioned = Vec::new();

        let chunk_direction = measured.values.direction();

        let advance = measured.spans.iter().fold((0.0, 0.0), |acc, measured| {
            (acc.0 + measured.advance.0, acc.1 + measured.advance.1)
        });

        // measured.advance is the size of the chunk.  Compute the offsets needed to align
        // it per the text-anchor property (start, middle, end):
        let anchor_offset = text_anchor_offset(
            measured.values.text_anchor(),
            chunk_direction,
            text_writing_mode,
            advance,
        );

        let mut x = x + anchor_offset.0;
        let mut y = y + anchor_offset.1;

        // Position each span

        for mspan in &measured.spans {
            let params = NormalizeParams::new(&mspan.values, view_params);

            let layout = mspan.layout.clone();
            let values = mspan.values.clone();
            let dx = mspan.dx;
            let dy = mspan.dy;
            let advance = mspan.advance;

            let baseline_offset = compute_baseline_offset(&layout, &values, &params);

            let start_pos = match chunk_direction {
                Direction::Ltr => (x, y),
                Direction::Rtl => (x - advance.0, y),
            };

            let span_advance = match chunk_direction {
                Direction::Ltr => (advance.0, advance.1),
                Direction::Rtl => (-advance.0, advance.1),
            };

            let rendered_position = if text_writing_mode.is_horizontal() {
                (start_pos.0 + dx, start_pos.1 - baseline_offset + dy)
            } else {
                (start_pos.0 + baseline_offset + dx, start_pos.1 + dy)
            };

            let positioned_span = PositionedSpan {
                layout,
                values,
                rendered_position,
            };

            positioned.push(positioned_span);

            x = x + span_advance.0 + dx;
            y = y + span_advance.1 + dy;
        }

        PositionedChunk {
            next_chunk_x: x,
            next_chunk_y: y,
            spans: positioned,
            link: measured.link.clone(),
        }
    }
}

fn compute_baseline_offset(
    layout: &pango::Layout,
    values: &ComputedValues,
    params: &NormalizeParams,
) -> f64 {
    let baseline = f64::from(layout.baseline()) / f64::from(pango::SCALE);
    let baseline_shift = values.baseline_shift().0.to_user(&params);
    baseline + baseline_shift
}

/// Computes the (x, y) offsets to be applied to spans after applying the text-anchor property (start, middle, end).
#[rustfmt::skip]
fn text_anchor_offset(
    anchor: TextAnchor,
    direction: Direction,
    text_writing_mode: WritingMode,
    chunk_size: (f64, f64),
) -> (f64, f64) {
    let (w, h) = chunk_size;

    if text_writing_mode.is_horizontal() {
        match (anchor, direction) {
            (TextAnchor::Start,  Direction::Ltr) => (0.0, 0.0),
            (TextAnchor::Start,  Direction::Rtl) => (0.0, 0.0),

            (TextAnchor::Middle, Direction::Ltr) => (-w / 2.0, 0.0),
            (TextAnchor::Middle, Direction::Rtl) => (w / 2.0, 0.0),

            (TextAnchor::End,    Direction::Ltr) => (-w, 0.0),
            (TextAnchor::End,    Direction::Rtl) => (w, 0.0),
        }
    } else {
        // FIXME: we don't deal with text direction for vertical text yet.
        match anchor {
            TextAnchor::Start => (0.0, 0.0),
            TextAnchor::Middle => (0.0, -h / 2.0),
            TextAnchor::End => (0.0, -h),
        }
    }
}

impl Span {
    fn new(text: &str, values: Rc<ComputedValues>, dx: f64, dy: f64, depth: usize) -> Span {
        Span {
            values,
            text: text.to_string(),
            dx,
            dy,
            _depth: depth,
        }
    }
}

impl MeasuredSpan {
    fn from_span(
        span: &Span,
        text_writing_mode: WritingMode,
        draw_ctx: &DrawingCtx,
    ) -> MeasuredSpan {
        let values = span.values.clone();

        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(&values, &view_params);

        let properties = FontProperties::new(&values, text_writing_mode, &params);
        let layout = create_pango_layout(draw_ctx, &properties, &span.text);
        let (w, h) = layout.size();

        let w = f64::from(w) / f64::from(pango::SCALE);
        let h = f64::from(h) / f64::from(pango::SCALE);

        // This is the logical size of the layout, regardless of text direction, so it's always positive.
        assert!(w >= 0.0);
        assert!(h >= 0.0);

        let advance = if text_writing_mode.is_horizontal() {
            (w, 0.0)
        } else {
            (0.0, w)
        };

        MeasuredSpan {
            values,
            layout,
            _layout_size: (w, h),
            advance,
            dx: span.dx,
            dy: span.dy,
        }
    }
}

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() ?
fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    matches!(gravity, pango::Gravity::East | pango::Gravity::West)
}

fn compute_text_box(
    layout: &pango::Layout,
    x: f64,
    y: f64,
    transform: Transform,
    gravity: pango::Gravity,
) -> Option<BoundingBox> {
    #![allow(clippy::many_single_char_names)]

    let (ink, _) = layout.extents();
    if ink.width == 0 || ink.height == 0 {
        return None;
    }

    let ink_x = f64::from(ink.x);
    let ink_y = f64::from(ink.y);
    let ink_width = f64::from(ink.width);
    let ink_height = f64::from(ink.height);
    let pango_scale = f64::from(pango::SCALE);

    let (x, y, w, h) = if gravity_is_vertical(gravity) {
        (
            x + (ink_x - ink_height) / pango_scale,
            y + ink_y / pango_scale,
            ink_height / pango_scale,
            ink_width / pango_scale,
        )
    } else {
        (
            x + ink_x / pango_scale,
            y + ink_y / pango_scale,
            ink_width / pango_scale,
            ink_height / pango_scale,
        )
    };

    let r = Rect::new(x, y, x + w, y + h);
    let bbox = BoundingBox::new()
        .with_transform(transform)
        .with_rect(r)
        .with_ink_rect(r);

    Some(bbox)
}

impl PositionedSpan {
    fn layout(
        &self,
        acquired_nodes: &mut AcquiredNodes<'_>,
        draw_ctx: &mut DrawingCtx,
        view_params: &ViewParams,
        link_target: Option<String>,
    ) -> LayoutSpan {
        let params = NormalizeParams::new(&self.values, view_params);

        let layout = self.layout.clone();
        let is_visible = self.values.is_visible();
        let (x, y) = self.rendered_position;

        let stroke = Stroke::new(&self.values, &params);

        let gravity = layout.context().unwrap().gravity();

        let bbox = compute_text_box(&layout, x, y, draw_ctx.get_transform(), gravity);

        let stroke_paint = self.values.stroke().0.resolve(
            acquired_nodes,
            self.values.stroke_opacity().0,
            self.values.color().0,
            None,
            None,
        );

        let fill_paint = self.values.fill().0.resolve(
            acquired_nodes,
            self.values.fill_opacity().0,
            self.values.color().0,
            None,
            None,
        );

        let paint_order = self.values.paint_order();
        let text_rendering = self.values.text_rendering();

        LayoutSpan {
            layout,
            gravity,
            bbox,
            is_visible,
            x,
            y,
            paint_order,
            stroke,
            stroke_paint,
            fill_paint,
            text_rendering,
            link_target,
            values: self.values.clone(),
        }
    }
}

/// Walks the children of a `<text>`, `<tspan>`, or `<tref>` element
/// and appends chunks/spans from them into the specified `chunks`
/// array.
fn children_to_chunks(
    chunks: &mut Vec<Chunk>,
    node: &Node,
    acquired_nodes: &mut AcquiredNodes<'_>,
    cascaded: &CascadedValues<'_>,
    draw_ctx: &mut DrawingCtx,
    dx: f64,
    dy: f64,
    depth: usize,
    link: Option<String>,
) {
    for child in node.children() {
        if child.is_chars() {
            let values = cascaded.get();
            child
                .borrow_chars()
                .to_chunks(&child, Rc::new(values.clone()), chunks, dx, dy, depth);
        } else {
            assert!(child.is_element());

            match *child.borrow_element() {
                Element::TSpan(ref tspan) => {
                    let cascaded = CascadedValues::new(cascaded, &child);
                    tspan.to_chunks(
                        &child,
                        acquired_nodes,
                        &cascaded,
                        draw_ctx,
                        chunks,
                        dx,
                        dy,
                        depth + 1,
                        link.clone(),
                    );
                }

                Element::Link(ref link) => {
                    // TSpan::default tes all offsets to 0,
                    // which is what we want in links.
                    let tspan = TSpan::default();
                    let cascaded = CascadedValues::new(cascaded, &child);
                    tspan.to_chunks(
                        &child,
                        acquired_nodes,
                        &cascaded,
                        draw_ctx,
                        chunks,
                        dx,
                        dy,
                        depth + 1,
                        link.link.clone(),
                    );
                }

                Element::TRef(ref tref) => {
                    let cascaded = CascadedValues::new(cascaded, &child);
                    tref.to_chunks(&child, acquired_nodes, &cascaded, chunks, depth + 1);
                }

                _ => (),
            }
        }
    }
}

/// In SVG text elements, we use `Chars` to store character data.  For example,
/// an element like `<text>Foo Bar</text>` will be a `Text` with a single child,
/// and the child will be a `Chars` with "Foo Bar" for its contents.
///
/// Text elements can contain `<tspan>` sub-elements.  In this case,
/// those `tspan` nodes will also contain `Chars` children.
///
/// A text or tspan element can contain more than one `Chars` child, for example,
/// if there is an XML comment that splits the character contents in two:
///
/// ```xml
/// <text>
///   This sentence will create a Chars.
///   <!-- this comment is ignored -->
///   This sentence will cretea another Chars.
/// </text>
/// ```
///
/// When rendering a text element, it will take care of concatenating the strings
/// in its `Chars` children as appropriate, depending on the
/// `xml:space="preserve"` attribute.  A `Chars` stores the characters verbatim
/// as they come out of the XML parser, after ensuring that they are valid UTF-8.

#[derive(Default)]
pub struct Chars {
    string: RefCell<String>,
    space_normalized: RefCell<Option<String>>,
}

impl Chars {
    pub fn new(initial_text: &str) -> Chars {
        Chars {
            string: RefCell::new(String::from(initial_text)),
            space_normalized: RefCell::new(None),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.string.borrow().is_empty()
    }

    pub fn append(&self, s: &str) {
        self.string.borrow_mut().push_str(s);
        *self.space_normalized.borrow_mut() = None;
    }

    fn ensure_normalized_string(&self, node: &Node, values: &ComputedValues) {
        let mut normalized = self.space_normalized.borrow_mut();

        if (*normalized).is_none() {
            let mode = match values.xml_space() {
                XmlSpace::Default => XmlSpaceNormalize::Default(NormalizeDefault {
                    has_element_before: node.previous_sibling().is_some(),
                    has_element_after: node.next_sibling().is_some(),
                }),

                XmlSpace::Preserve => XmlSpaceNormalize::Preserve,
            };

            *normalized = Some(xml_space_normalize(mode, &self.string.borrow()));
        }
    }

    fn make_span(
        &self,
        node: &Node,
        values: Rc<ComputedValues>,
        dx: f64,
        dy: f64,
        depth: usize,
    ) -> Option<Span> {
        self.ensure_normalized_string(node, &*values);

        if self.space_normalized.borrow().as_ref().unwrap() == "" {
            None
        } else {
            Some(Span::new(
                self.space_normalized.borrow().as_ref().unwrap(),
                values,
                dx,
                dy,
                depth,
            ))
        }
    }

    fn to_chunks(
        &self,
        node: &Node,
        values: Rc<ComputedValues>,
        chunks: &mut Vec<Chunk>,
        dx: f64,
        dy: f64,
        depth: usize,
    ) {
        if let Some(span) = self.make_span(node, values, dx, dy, depth) {
            let num_chunks = chunks.len();
            assert!(num_chunks > 0);

            chunks[num_chunks - 1].spans.push(span);
        }
    }

    pub fn get_string(&self) -> String {
        self.string.borrow().clone()
    }
}

#[derive(Default)]
pub struct Text {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    dx: Length<Horizontal>,
    dy: Length<Vertical>,
}

impl Text {
    fn make_chunks(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        x: f64,
        y: f64,
    ) -> Vec<Chunk> {
        let mut chunks = Vec::new();

        let values = cascaded.get();
        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        chunks.push(Chunk::new(values, Some(x), Some(y), None));

        let dx = self.dx.to_user(&params);
        let dy = self.dy.to_user(&params);

        children_to_chunks(
            &mut chunks,
            node,
            acquired_nodes,
            cascaded,
            draw_ctx,
            dx,
            dy,
            0,
            None,
        );
        chunks
    }
}

impl SetAttributes for Text {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "dx") => self.dx = attr.parse(value)?,
                expanded_name!("", "dy") => self.dy = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Text {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        let elt = node.borrow_element();

        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, values.transform(), values);

        let text_writing_mode = values.writing_mode();

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc| {
                let mut x = self.x.to_user(&params);
                let mut y = self.y.to_user(&params);

                let chunks = self.make_chunks(node, an, cascaded, dc, x, y);

                let mut measured_chunks = Vec::new();
                for chunk in &chunks {
                    measured_chunks.push(MeasuredChunk::from_chunk(chunk, text_writing_mode, dc));
                }

                let mut positioned_chunks = Vec::new();
                for chunk in &measured_chunks {
                    let chunk_x = chunk.x.unwrap_or(x);
                    let chunk_y = chunk.y.unwrap_or(y);

                    let positioned = PositionedChunk::from_measured(
                        chunk,
                        &view_params,
                        text_writing_mode,
                        chunk_x,
                        chunk_y,
                    );

                    x = positioned.next_chunk_x;
                    y = positioned.next_chunk_y;

                    positioned_chunks.push(positioned);
                }

                let view_params = dc.get_view_params();

                let mut layout_spans = Vec::new();
                for chunk in &positioned_chunks {
                    for span in &chunk.spans {
                        layout_spans.push(span.layout(an, dc, &view_params, chunk.link.clone()));
                    }
                }

                let text_bbox = layout_spans.iter().fold(dc.empty_bbox(), |mut bbox, span| {
                    if let Some(ref span_bbox) = span.bbox {
                        bbox.insert(span_bbox);
                    }

                    bbox
                });

                let mut text_spans = Vec::new();
                for span in layout_spans {
                    let stroke_paint =
                        span.stroke_paint
                            .to_user_space(&text_bbox, &view_params, &span.values);
                    let fill_paint =
                        span.fill_paint
                            .to_user_space(&text_bbox, &view_params, &span.values);

                    let text_span = TextSpan {
                        layout: span.layout,
                        gravity: span.gravity,
                        bbox: span.bbox,
                        is_visible: span.is_visible,
                        x: span.x,
                        y: span.y,
                        paint_order: span.paint_order,
                        stroke: span.stroke,
                        stroke_paint,
                        fill_paint,
                        text_rendering: span.text_rendering,
                        link_target: span.link_target,
                    };

                    text_spans.push(text_span);
                }

                let text_layout = layout::Text { spans: text_spans };

                dc.draw_text(&text_layout, an, clipping)
            },
        )
    }
}

#[derive(Default)]
pub struct TRef {
    link: Option<NodeId>,
}

impl TRef {
    fn to_chunks(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        chunks: &mut Vec<Chunk>,
        depth: usize,
    ) {
        if self.link.is_none() {
            return;
        }

        let link = self.link.as_ref().unwrap();

        let values = cascaded.get();
        if !values.is_displayed() {
            return;
        }

        if let Ok(acquired) = acquired_nodes.acquire(link) {
            let c = acquired.get();
            extract_chars_children_to_chunks_recursively(chunks, c, Rc::new(values.clone()), depth);
        } else {
            rsvg_log!(
                "element {} references a nonexistent text source \"{}\"",
                node,
                link,
            );
        }
    }
}

fn extract_chars_children_to_chunks_recursively(
    chunks: &mut Vec<Chunk>,
    node: &Node,
    values: Rc<ComputedValues>,
    depth: usize,
) {
    for child in node.children() {
        let values = values.clone();

        if child.is_chars() {
            child
                .borrow_chars()
                .to_chunks(&child, values, chunks, 0.0, 0.0, depth)
        } else {
            extract_chars_children_to_chunks_recursively(chunks, &child, values, depth + 1)
        }
    }
}

impl SetAttributes for TRef {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        self.link = attrs
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!(xlink "href"))
            // Unlike other elements which use `href` in SVG2 versus `xlink:href` in SVG1.1,
            // the <tref> element got removed in SVG2.  So, here we still use a match
            // against the full namespaced version of the attribute.
            .and_then(|(attr, value)| NodeId::parse(value).attribute(attr).ok());

        Ok(())
    }
}

impl Draw for TRef {}

#[derive(Default)]
pub struct TSpan {
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    dx: Length<Horizontal>,
    dy: Length<Vertical>,
}

impl TSpan {
    fn to_chunks(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        chunks: &mut Vec<Chunk>,
        dx: f64,
        dy: f64,
        depth: usize,
        link: Option<String>,
    ) {
        let values = cascaded.get();
        if !values.is_displayed() {
            return;
        }

        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        let x = self.x.map(|l| l.to_user(&params));
        let y = self.y.map(|l| l.to_user(&params));

        let span_dx = dx + self.dx.to_user(&params);
        let span_dy = dy + self.dy.to_user(&params);

        if x.is_some() || y.is_some() {
            chunks.push(Chunk::new(values, x, y, link.clone()));
        }

        children_to_chunks(
            chunks,
            node,
            acquired_nodes,
            cascaded,
            draw_ctx,
            span_dx,
            span_dy,
            depth,
            link,
        );
    }
}

impl SetAttributes for TSpan {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "dx") => self.dx = attr.parse(value)?,
                expanded_name!("", "dy") => self.dy = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for TSpan {}

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

impl From<FontWeight> for pango::Weight {
    fn from(w: FontWeight) -> pango::Weight {
        pango::Weight::__Unknown(w.numeric_weight().into())
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
        use WritingMode::*;
        match m {
            HorizontalTb | VerticalRl | VerticalLr | LrTb | Lr | Tb | TbRl => pango::Direction::Ltr,
            RlTb | Rl => pango::Direction::Rtl,
        }
    }
}

impl From<WritingMode> for pango::Gravity {
    fn from(m: WritingMode) -> pango::Gravity {
        use WritingMode::*;
        match m {
            HorizontalTb | LrTb | Lr | RlTb | Rl => pango::Gravity::South,
            VerticalRl | Tb | TbRl => pango::Gravity::East,
            VerticalLr => pango::Gravity::West,
        }
    }
}

fn create_pango_layout(draw_ctx: &DrawingCtx, props: &FontProperties, text: &str) -> pango::Layout {
    let pango_context = draw_ctx.create_pango_context();

    if let XmlLang(Some(ref lang)) = props.xml_lang {
        pango_context.set_language(&pango::Language::from_string(lang.as_str()));
    }

    pango_context.set_base_gravity(pango::Gravity::from(props.writing_mode));

    match (props.unicode_bidi, props.direction) {
        (UnicodeBidi::Override, _) | (UnicodeBidi::Embed, _) => {
            pango_context.set_base_dir(pango::Direction::from(props.direction));
        }

        (_, direction) if direction != Direction::Ltr => {
            pango_context.set_base_dir(pango::Direction::from(direction));
        }

        (_, _) => {
            pango_context.set_base_dir(pango::Direction::from(props.writing_mode));
        }
    }

    let mut font_desc = pango_context.font_description().unwrap();
    font_desc.set_family(props.font_family.as_str());
    font_desc.set_style(pango::Style::from(props.font_style));

    // PANGO_VARIANT_SMALL_CAPS does nothing: https://gitlab.gnome.org/GNOME/pango/-/issues/566
    // see below for using the "smcp" OpenType feature for fonts that support it.
    // font_desc.set_variant(pango::Variant::from(props.font_variant));

    font_desc.set_weight(pango::Weight::from(props.font_weight));
    font_desc.set_stretch(pango::Stretch::from(props.font_stretch));

    font_desc.set_size(to_pango_units(props.font_size));

    let layout = pango::Layout::new(&pango_context);
    layout.set_auto_dir(false);
    layout.set_font_description(Some(&font_desc));

    // FIXME: For now we ignore the `line-height` property, even though we parse it.
    // We would need to do something like this:
    //
    // layout.set_line_spacing(0.0); // "actually use the spacing I'll give you"
    // layout.set_spacing(to_pango_units(???));
    //
    // However, Layout::set_spacing() takes an inter-line spacing (from the baseline of
    // one line to the top of the next line), not the line height (from baseline to
    // baseline).
    //
    // Maybe we need to implement layout of individual lines by hand.

    let attr_list = pango::AttrList::new();

    attr_list.insert(pango::Attribute::new_letter_spacing(to_pango_units(
        props.letter_spacing,
    )));

    if props.text_decoration.underline {
        attr_list.insert(pango::Attribute::new_underline(pango::Underline::Single));
    }

    if props.text_decoration.strike {
        attr_list.insert(pango::Attribute::new_strikethrough(true));
    }

    // FIXME: Using the "smcp" OpenType feature only works for fonts that support it.  We
    // should query if the font supports small caps, and synthesize them if it doesn't.
    if props.font_variant == FontVariant::SmallCaps {
        // smcp - small capitals - https://docs.microsoft.com/en-ca/typography/opentype/spec/features_pt#smcp
        attr_list.insert(pango::Attribute::new_font_features("'smcp' 1"));
    }

    layout.set_attributes(Some(&attr_list));
    layout.set_alignment(pango::Alignment::from(props.direction));
    layout.set_text(text);

    layout
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chars_default() {
        let c = Chars::default();
        assert!(c.is_empty());
        assert!(c.space_normalized.borrow().is_none());
    }

    #[test]
    fn chars_new() {
        let example = "Test 123";
        let c = Chars::new(example);
        assert_eq!(c.get_string(), example);
        assert!(c.space_normalized.borrow().is_none());
    }

    // This is called _horizontal because the property value in "CSS Writing Modes 3"
    // is `horizontal-tb`.  Eventually we will support that and this will make more sense.
    #[test]
    fn adjusted_advance_horizontal_ltr() {
        use Direction::*;
        use TextAnchor::*;

        assert_eq!(
            text_anchor_offset(Start, Ltr, WritingMode::Lr, (2.0, 4.0)),
            (0.0, 0.0)
        );

        assert_eq!(
            text_anchor_offset(Middle, Ltr, WritingMode::Lr, (2.0, 4.0)),
            (-1.0, 0.0)
        );

        assert_eq!(
            text_anchor_offset(End, Ltr, WritingMode::Lr, (2.0, 4.0)),
            (-2.0, 0.0)
        );
    }

    #[test]
    fn adjusted_advance_horizontal_rtl() {
        use Direction::*;
        use TextAnchor::*;

        assert_eq!(
            text_anchor_offset(Start, Rtl, WritingMode::Rl, (2.0, 4.0)),
            (0.0, 0.0)
        );
        assert_eq!(
            text_anchor_offset(Middle, Rtl, WritingMode::Rl, (2.0, 4.0)),
            (1.0, 0.0)
        );
        assert_eq!(
            text_anchor_offset(TextAnchor::End, Direction::Rtl, WritingMode::Rl, (2.0, 4.0)),
            (2.0, 0.0)
        );
    }

    // This is called _vertical because "CSS Writing Modes 3" has both `vertical-rl` (East
    // Asia), and `vertical-lr` (Manchu, Mongolian), but librsvg does not support block
    // flow direction properly yet.  Eventually we will support that and this will make
    // more sense.
    #[test]
    fn adjusted_advance_vertical() {
        use Direction::*;
        use TextAnchor::*;

        assert_eq!(
            text_anchor_offset(Start, Ltr, WritingMode::Tb, (2.0, 4.0)),
            (0.0, 0.0)
        );

        assert_eq!(
            text_anchor_offset(Middle, Ltr, WritingMode::Tb, (2.0, 4.0)),
            (0.0, -2.0)
        );

        assert_eq!(
            text_anchor_offset(End, Ltr, WritingMode::Tb, (2.0, 4.0)),
            (0.0, -4.0)
        );
    }
}
