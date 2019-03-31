use cairo;
use std::cell::Cell;
use std::cell::RefCell;
use std::ops::Deref;

use crate::attributes::Attribute;
use crate::drawing_ctx::DrawingCtx;
use crate::error::*;
use crate::length::*;
use crate::marker;
use crate::node::*;
use crate::parsers::{CssParserExt, Parse, ParseValue};
use crate::path_builder::*;
use crate::path_parser;
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use cssparser::{Parser, Token};

fn render_path_builder(
    builder: &PathBuilder,
    draw_ctx: &mut DrawingCtx,
    node: &RsvgNode,
    values: &ComputedValues,
    render_markers: bool,
    clipping: bool,
) -> Result<(), RenderingError> {
    draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
        let cr = dc.get_cairo_context();

        dc.set_affine_on_cr(&cr);
        builder.to_cairo(&cr);

        if clipping {
            cr.set_fill_rule(cairo::FillRule::from(values.clip_rule));
        } else {
            cr.set_fill_rule(cairo::FillRule::from(values.fill_rule));
            dc.stroke_and_fill(&cr, values)?;
        }

        Ok(())
    })?;

    if render_markers {
        marker::render_markers_for_path_builder(builder, draw_ctx, values, clipping)?;
    }

    Ok(())
}

fn render_ellipse(
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    draw_ctx: &mut DrawingCtx,
    node: &RsvgNode,
    values: &ComputedValues,
    clipping: bool,
) -> Result<(), RenderingError> {
    // Per the spec, rx and ry must be nonnegative
    if rx <= 0.0 || ry <= 0.0 {
        return Ok(());
    }

    // 4/3 * (1-cos 45°)/sin 45° = 4/3 * sqrt(2) - 1
    let arc_magic: f64 = 0.5522847498;

    // approximate an ellipse using 4 Bézier curves
    let mut builder = PathBuilder::new();

    builder.move_to(cx + rx, cy);

    builder.curve_to(
        cx + rx,
        cy + arc_magic * ry,
        cx + arc_magic * rx,
        cy + ry,
        cx,
        cy + ry,
    );

    builder.curve_to(
        cx - arc_magic * rx,
        cy + ry,
        cx - rx,
        cy + arc_magic * ry,
        cx - rx,
        cy,
    );

    builder.curve_to(
        cx - rx,
        cy - arc_magic * ry,
        cx - arc_magic * rx,
        cy - ry,
        cx,
        cy - ry,
    );

    builder.curve_to(
        cx + arc_magic * rx,
        cy - ry,
        cx + rx,
        cy - arc_magic * ry,
        cx + rx,
        cy,
    );

    builder.close_path();

    render_path_builder(&builder, draw_ctx, node, values, false, clipping)
}

pub struct NodePath {
    builder: RefCell<Option<PathBuilder>>,
}

impl NodePath {
    pub fn new() -> NodePath {
        NodePath {
            builder: RefCell::new(None),
        }
    }
}

impl NodeTrait for NodePath {
    fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            if attr == Attribute::D {
                let mut builder = PathBuilder::new();

                if let Err(e) = path_parser::parse_path_into_builder(value, &mut builder) {
                    // FIXME: we don't propagate errors upstream, but creating a partial
                    // path is OK per the spec

                    rsvg_log!(
                        "could not parse path {}: {}",
                        node.get_human_readable_name(),
                        e
                    );
                }

                *self.builder.borrow_mut() = Some(builder);
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

        if let Some(ref builder) = *self.builder.borrow() {
            render_path_builder(builder, draw_ctx, node, values, true, clipping)?;
        }

        Ok(())
    }
}

#[derive(Debug, PartialEq)]
enum PolyKind {
    Open,
    Closed,
}

#[derive(Debug, PartialEq)]
struct Points(Vec<(f64, f64)>);

impl Deref for Points {
    type Target = [(f64, f64)];

    fn deref(&self) -> &[(f64, f64)] {
        &self.0
    }
}

// Parse a list-of-points as for polyline and polygon elements
// https://www.w3.org/TR/SVG/shapes.html#PointsBNF
impl Parse for Points {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Points, ValueErrorKind> {
        let mut v = Vec::new();

        loop {
            let x = f64::from(parser.expect_finite_number()?);
            parser.optional_comma();
            let y = f64::from(parser.expect_finite_number()?);

            v.push((x, y));

            if parser.is_exhausted() {
                break;
            }

            match parser.next_including_whitespace() {
                Ok(&Token::WhiteSpace(_)) => (),
                _ => parser.optional_comma(),
            }
        }

        Ok(Points(v))
    }
}

pub struct NodePoly {
    points: RefCell<Option<Points>>,
    kind: PolyKind,
}

impl NodePoly {
    pub fn new_open() -> NodePoly {
        NodePoly {
            points: RefCell::new(None),
            kind: PolyKind::Open,
        }
    }

    pub fn new_closed() -> NodePoly {
        NodePoly {
            points: RefCell::new(None),
            kind: PolyKind::Closed,
        }
    }
}

impl NodeTrait for NodePoly {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            // support for svg < 1.0 which used verts
            if attr == Attribute::Points || attr == Attribute::Verts {
                *self.points.borrow_mut() = attr.parse(value.trim()).map(Some)?;
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

        if let Some(ref points) = *self.points.borrow() {
            let mut builder = PathBuilder::new();

            for (i, &(x, y)) in points.iter().enumerate() {
                if i == 0 {
                    builder.move_to(x, y);
                } else {
                    builder.line_to(x, y);
                }
            }

            if self.kind == PolyKind::Closed {
                builder.close_path();
            }

            render_path_builder(&builder, draw_ctx, node, values, true, clipping)?;
        }

        Ok(())
    }
}

pub struct NodeLine {
    x1: Cell<LengthHorizontal>,
    y1: Cell<LengthVertical>,
    x2: Cell<LengthHorizontal>,
    y2: Cell<LengthVertical>,
}

impl NodeLine {
    pub fn new() -> NodeLine {
        NodeLine {
            x1: Cell::new(Default::default()),
            y1: Cell::new(Default::default()),
            x2: Cell::new(Default::default()),
            y2: Cell::new(Default::default()),
        }
    }
}

impl NodeTrait for NodeLine {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X1 => self.x1.set(attr.parse(value)?),
                Attribute::Y1 => self.y1.set(attr.parse(value)?),
                Attribute::X2 => self.x2.set(attr.parse(value)?),
                Attribute::Y2 => self.y2.set(attr.parse(value)?),
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

        let mut builder = PathBuilder::new();

        let params = draw_ctx.get_view_params();

        let x1 = self.x1.get().normalize(values, &params);
        let y1 = self.y1.get().normalize(values, &params);
        let x2 = self.x2.get().normalize(values, &params);
        let y2 = self.y2.get().normalize(values, &params);

        builder.move_to(x1, y1);
        builder.line_to(x2, y2);

        render_path_builder(&builder, draw_ctx, node, values, true, clipping)
    }
}

pub struct NodeRect {
    // x, y, width, height
    x: Cell<LengthHorizontal>,
    y: Cell<LengthVertical>,
    w: Cell<LengthHorizontal>,
    h: Cell<LengthVertical>,

    // Radiuses for rounded corners
    rx: Cell<Option<LengthHorizontal>>,
    ry: Cell<Option<LengthVertical>>,
}

impl NodeRect {
    pub fn new() -> NodeRect {
        NodeRect {
            x: Cell::new(Default::default()),
            y: Cell::new(Default::default()),
            w: Cell::new(Default::default()),
            h: Cell::new(Default::default()),

            rx: Cell::new(None),
            ry: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeRect {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(attr.parse(value)?),
                Attribute::Y => self.y.set(attr.parse(value)?),
                Attribute::Width => self
                    .w
                    .set(attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?),
                Attribute::Height => self
                    .h
                    .set(attr.parse_and_validate(value, LengthVertical::check_nonnegative)?),
                Attribute::Rx => self.rx.set(
                    attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)
                        .map(Some)?,
                ),
                Attribute::Ry => self.ry.set(
                    attr.parse_and_validate(value, LengthVertical::check_nonnegative)
                        .map(Some)?,
                ),

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

        let x = self.x.get().normalize(values, &params);
        let y = self.y.get().normalize(values, &params);
        let w = self.w.get().normalize(values, &params);
        let h = self.h.get().normalize(values, &params);

        let mut rx;
        let mut ry;

        match (self.rx.get(), self.ry.get()) {
            (None, None) => {
                rx = 0.0;
                ry = 0.0;
            }

            (Some(_rx), None) => {
                rx = _rx.normalize(values, &params);
                ry = _rx.normalize(values, &params);
            }

            (None, Some(_ry)) => {
                rx = _ry.normalize(values, &params);
                ry = _ry.normalize(values, &params);
            }

            (Some(_rx), Some(_ry)) => {
                rx = _rx.normalize(values, &params);
                ry = _ry.normalize(values, &params);
            }
        }

        // Per the spec, w,h must be >= 0
        if w <= 0.0 || h <= 0.0 {
            return Ok(());
        }

        // ... and rx,ry must be nonnegative
        if rx < 0.0 || ry < 0.0 {
            return Ok(());
        }

        let half_w = w / 2.0;
        let half_h = h / 2.0;

        if rx > half_w {
            rx = half_w;
        }

        if ry > half_h {
            ry = half_h;
        }

        if rx == 0.0 {
            ry = 0.0;
        } else if ry == 0.0 {
            rx = 0.0;
        }

        let mut builder = PathBuilder::new();

        if rx == 0.0 {
            // Easy case, no rounded corners
            builder.move_to(x, y);
            builder.line_to(x + w, y);
            builder.line_to(x + w, y + h);
            builder.line_to(x, y + h);
            builder.line_to(x, y);
            builder.close_path();
        } else {
            /* Hard case, rounded corners
             *
             *      (top_x1, top_y)                   (top_x2, top_y)
             *     *--------------------------------*
             *    /                                  \
             *   * (left_x, left_y1)                  * (right_x, right_y1)
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   |                                    |
             *   * (left_x, left_y2)                  * (right_x, right_y2)
             *    \                                  /
             *     *--------------------------------*
             *      (bottom_x1, bottom_y)            (bottom_x2, bottom_y)
             */

            let top_x1 = x + rx;
            let top_x2 = x + w - rx;
            let top_y = y;

            let bottom_x1 = top_x1;
            let bottom_x2 = top_x2;
            let bottom_y = y + h;

            let left_x = x;
            let left_y1 = y + ry;
            let left_y2 = y + h - ry;

            let right_x = x + w;
            let right_y1 = left_y1;
            let right_y2 = left_y2;

            builder.move_to(top_x1, top_y);
            builder.line_to(top_x2, top_y);

            builder.arc(
                top_x2,
                top_y,
                rx,
                ry,
                0.0,
                LargeArc(false),
                Sweep::Positive,
                right_x,
                right_y1,
            );

            builder.line_to(right_x, right_y2);

            builder.arc(
                right_x,
                right_y2,
                rx,
                ry,
                0.0,
                LargeArc(false),
                Sweep::Positive,
                bottom_x2,
                bottom_y,
            );

            builder.line_to(bottom_x1, bottom_y);

            builder.arc(
                bottom_x1,
                bottom_y,
                rx,
                ry,
                0.0,
                LargeArc(false),
                Sweep::Positive,
                left_x,
                left_y2,
            );

            builder.line_to(left_x, left_y1);

            builder.arc(
                left_x,
                left_y1,
                rx,
                ry,
                0.0,
                LargeArc(false),
                Sweep::Positive,
                top_x1,
                top_y,
            );

            builder.close_path();
        }

        render_path_builder(&builder, draw_ctx, node, values, false, clipping)
    }
}

pub struct NodeCircle {
    cx: Cell<LengthHorizontal>,
    cy: Cell<LengthVertical>,
    r: Cell<LengthBoth>,
}

impl NodeCircle {
    pub fn new() -> NodeCircle {
        NodeCircle {
            cx: Cell::new(Default::default()),
            cy: Cell::new(Default::default()),
            r: Cell::new(Default::default()),
        }
    }
}

impl NodeTrait for NodeCircle {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Cx => self.cx.set(attr.parse(value)?),
                Attribute::Cy => self.cy.set(attr.parse(value)?),
                Attribute::R => self
                    .r
                    .set(attr.parse_and_validate(value, LengthBoth::check_nonnegative)?),

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

        let cx = self.cx.get().normalize(values, &params);
        let cy = self.cy.get().normalize(values, &params);
        let r = self.r.get().normalize(values, &params);

        render_ellipse(cx, cy, r, r, draw_ctx, node, values, clipping)
    }
}

pub struct NodeEllipse {
    cx: Cell<LengthHorizontal>,
    cy: Cell<LengthVertical>,
    rx: Cell<LengthHorizontal>,
    ry: Cell<LengthVertical>,
}

impl NodeEllipse {
    pub fn new() -> NodeEllipse {
        NodeEllipse {
            cx: Cell::new(Default::default()),
            cy: Cell::new(Default::default()),
            rx: Cell::new(Default::default()),
            ry: Cell::new(Default::default()),
        }
    }
}

impl NodeTrait for NodeEllipse {
    fn set_atts(&self, _: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                Attribute::Cx => self.cx.set(attr.parse(value)?),
                Attribute::Cy => self.cy.set(attr.parse(value)?),
                Attribute::Rx => self
                    .rx
                    .set(attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?),
                Attribute::Ry => self
                    .ry
                    .set(attr.parse_and_validate(value, LengthVertical::check_nonnegative)?),

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

        let cx = self.cx.get().normalize(values, &params);
        let cy = self.cy.get().normalize(values, &params);
        let rx = self.rx.get().normalize(values, &params);
        let ry = self.ry.get().normalize(values, &params);

        render_ellipse(cx, cy, rx, ry, draw_ctx, node, values, clipping)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_points() {
        assert_eq!(Points::parse_str(" 1 2 "), Ok(Points(vec![(1.0, 2.0)])));
        assert_eq!(
            Points::parse_str("1 2 3 4"),
            Ok(Points(vec![(1.0, 2.0), (3.0, 4.0)]))
        );
        assert_eq!(
            Points::parse_str("1,2,3,4"),
            Ok(Points(vec![(1.0, 2.0), (3.0, 4.0)]))
        );
        assert_eq!(
            Points::parse_str("1,2 3,4"),
            Ok(Points(vec![(1.0, 2.0), (3.0, 4.0)]))
        );
        assert_eq!(
            Points::parse_str("1,2 -3,4"),
            Ok(Points(vec![(1.0, 2.0), (-3.0, 4.0)]))
        );
        assert_eq!(
            Points::parse_str("1,2,-3,4"),
            Ok(Points(vec![(1.0, 2.0), (-3.0, 4.0)]))
        );
    }

    #[test]
    fn errors_on_invalid_points() {
        assert!(Points::parse_str("-1-2-3-4").is_err());
        assert!(Points::parse_str("1 2-3,-4").is_err());
    }
}
