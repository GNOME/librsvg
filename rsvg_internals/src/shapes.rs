//! Basic SVG shapes: the `path`, `polygon`, `polyline`, `line`,
//! `rect`, `circle`, `ellipse` elements.

use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::ops::Deref;
use std::rc::Rc;

use crate::attributes::Attributes;
use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::{optional_comma, Parse, ParseValue};
use crate::path_builder::{LargeArc, Path as SvgPath, PathBuilder, Sweep};
use crate::path_parser;
use crate::properties::ComputedValues;
use cssparser::{Parser, Token};

#[derive(Copy, Clone, PartialEq)]
pub enum Markers {
    No,
    Yes,
}

pub struct Shape {
    path: Rc<SvgPath>,
    markers: Markers,
}

impl Shape {
    fn new(path: Rc<SvgPath>, markers: Markers) -> Shape {
        Shape { path, markers }
    }

    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        draw_ctx.draw_path(
            &self.path,
            node,
            acquired_nodes,
            values,
            self.markers,
            clipping,
        )
    }
}

fn make_ellipse(cx: f64, cy: f64, rx: f64, ry: f64) -> SvgPath {
    let mut builder = PathBuilder::default();

    // Per the spec, rx and ry must be nonnegative
    if rx <= 0.0 || ry <= 0.0 {
        return builder.into_path();
    }

    // 4/3 * (1-cos 45°)/sin 45° = 4/3 * sqrt(2) - 1
    let arc_magic: f64 = 0.5522847498;

    // approximate an ellipse using 4 Bézier curves

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

    builder.into_path()
}

#[derive(Default)]
pub struct Path {
    path: Option<Rc<SvgPath>>,
}

impl SetAttributes for Path {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            if attr.expanded() == expanded_name!("", "d") {
                let mut builder = PathBuilder::default();
                if let Err(e) = path_parser::parse_path_into_builder(value, &mut builder) {
                    // FIXME: we don't propagate errors upstream, but creating a partial
                    // path is OK per the spec

                    rsvg_log!("could not parse path: {}", e);
                }
                self.path = Some(Rc::new(builder.into_path()));
            }
        }

        Ok(())
    }
}

impl Draw for Path {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if let Some(path) = self.path.as_ref() {
            let values = cascaded.get();
            Shape::new(path.clone(), Markers::Yes).draw(
                node,
                acquired_nodes,
                values,
                draw_ctx,
                clipping,
            )
        } else {
            Ok(draw_ctx.empty_bbox())
        }
    }
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
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Points, ParseError<'i>> {
        let mut v = Vec::new();

        loop {
            let x = f64::parse(parser)?;
            optional_comma(parser);
            let y = f64::parse(parser)?;

            v.push((x, y));

            if parser.is_exhausted() {
                break;
            }

            match parser.next_including_whitespace() {
                Ok(&Token::WhiteSpace(_)) => (),
                _ => optional_comma(parser),
            }
        }

        Ok(Points(v))
    }
}

fn make_poly(points: Option<&Points>, closed: bool) -> SvgPath {
    let mut builder = PathBuilder::default();

    if let Some(points) = points {
        for (i, &(x, y)) in points.iter().enumerate() {
            if i == 0 {
                builder.move_to(x, y);
            } else {
                builder.line_to(x, y);
            }
        }

        if closed {
            builder.close_path();
        }
    }

    builder.into_path()
}

#[derive(Default)]
pub struct Polygon {
    points: Option<Points>,
}

impl SetAttributes for Polygon {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            if attr.expanded() == expanded_name!("", "points") {
                self.points = attr.parse(value).map(Some)?;
            }
        }

        Ok(())
    }
}

impl Draw for Polygon {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(Rc::new(make_poly(self.points.as_ref(), true)), Markers::Yes).draw(
            node,
            acquired_nodes,
            values,
            draw_ctx,
            clipping,
        )
    }
}

#[derive(Default)]
pub struct Polyline {
    points: Option<Points>,
}

impl SetAttributes for Polyline {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            if attr.expanded() == expanded_name!("", "points") {
                self.points = attr.parse(value).map(Some)?;
            }
        }

        Ok(())
    }
}

impl Draw for Polyline {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(
            Rc::new(make_poly(self.points.as_ref(), false)),
            Markers::Yes,
        )
        .draw(node, acquired_nodes, values, draw_ctx, clipping)
    }
}

#[derive(Default)]
pub struct Line {
    x1: Length<Horizontal>,
    y1: Length<Vertical>,
    x2: Length<Horizontal>,
    y2: Length<Vertical>,
}

impl SetAttributes for Line {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x1") => self.x1 = attr.parse(value)?,
                expanded_name!("", "y1") => self.y1 = attr.parse(value)?,
                expanded_name!("", "x2") => self.x2 = attr.parse(value)?,
                expanded_name!("", "y2") => self.y2 = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Line {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(Rc::new(self.make_path(values, draw_ctx)), Markers::Yes).draw(
            node,
            acquired_nodes,
            values,
            draw_ctx,
            clipping,
        )
    }
}

impl Line {
    fn make_path(&self, values: &ComputedValues, draw_ctx: &mut DrawingCtx) -> SvgPath {
        let mut builder = PathBuilder::default();

        let params = draw_ctx.get_view_params();

        let x1 = self.x1.normalize(values, &params);
        let y1 = self.y1.normalize(values, &params);
        let x2 = self.x2.normalize(values, &params);
        let y2 = self.y2.normalize(values, &params);

        builder.move_to(x1, y1);
        builder.line_to(x2, y2);

        builder.into_path()
    }
}

#[derive(Default)]
pub struct Rect {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    w: Length<Horizontal>,
    h: Length<Vertical>,

    // Radiuses for rounded corners
    rx: Option<Length<Horizontal>>,
    ry: Option<Length<Vertical>>,
}

impl SetAttributes for Rect {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => {
                    self.w =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!("", "height") => {
                    self.h =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
                expanded_name!("", "rx") => {
                    self.rx = attr
                        .parse_and_validate(value, Length::<Horizontal>::check_nonnegative)
                        .map(Some)?
                }
                expanded_name!("", "ry") => {
                    self.ry = attr
                        .parse_and_validate(value, Length::<Vertical>::check_nonnegative)
                        .map(Some)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Rect {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(Rc::new(self.make_path(values, draw_ctx)), Markers::No).draw(
            node,
            acquired_nodes,
            values,
            draw_ctx,
            clipping,
        )
    }
}

impl Rect {
    fn make_path(&self, values: &ComputedValues, draw_ctx: &mut DrawingCtx) -> SvgPath {
        let params = draw_ctx.get_view_params();

        let x = self.x.normalize(values, &params);
        let y = self.y.normalize(values, &params);
        let w = self.w.normalize(values, &params);
        let h = self.h.normalize(values, &params);

        let mut rx;
        let mut ry;

        match (self.rx, self.ry) {
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

        let mut builder = PathBuilder::default();

        // Per the spec, w,h must be >= 0
        if w <= 0.0 || h <= 0.0 {
            return builder.into_path();
        }

        // ... and rx,ry must be nonnegative
        if rx < 0.0 || ry < 0.0 {
            return builder.into_path();
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

        builder.into_path()
    }
}

#[derive(Default)]
pub struct Circle {
    cx: Length<Horizontal>,
    cy: Length<Vertical>,
    r: Length<Both>,
}

impl SetAttributes for Circle {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "cx") => self.cx = attr.parse(value)?,
                expanded_name!("", "cy") => self.cy = attr.parse(value)?,
                expanded_name!("", "r") => {
                    self.r = attr.parse_and_validate(value, Length::<Both>::check_nonnegative)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Circle {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(Rc::new(self.make_path(values, draw_ctx)), Markers::No).draw(
            node,
            acquired_nodes,
            values,
            draw_ctx,
            clipping,
        )
    }
}

impl Circle {
    fn make_path(&self, values: &ComputedValues, draw_ctx: &mut DrawingCtx) -> SvgPath {
        let params = draw_ctx.get_view_params();

        let cx = self.cx.normalize(values, &params);
        let cy = self.cy.normalize(values, &params);
        let r = self.r.normalize(values, &params);

        make_ellipse(cx, cy, r, r)
    }
}

#[derive(Default)]
pub struct Ellipse {
    cx: Length<Horizontal>,
    cy: Length<Vertical>,
    rx: Length<Horizontal>,
    ry: Length<Vertical>,
}

impl SetAttributes for Ellipse {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "cx") => self.cx = attr.parse(value)?,
                expanded_name!("", "cy") => self.cy = attr.parse(value)?,
                expanded_name!("", "rx") => {
                    self.rx =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!("", "ry") => {
                    self.ry =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Ellipse {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();
        Shape::new(Rc::new(self.make_path(values, draw_ctx)), Markers::No).draw(
            node,
            acquired_nodes,
            values,
            draw_ctx,
            clipping,
        )
    }
}

impl Ellipse {
    fn make_path(&self, values: &ComputedValues, draw_ctx: &mut DrawingCtx) -> SvgPath {
        let params = draw_ctx.get_view_params();

        let cx = self.cx.normalize(values, &params);
        let cy = self.cy.normalize(values, &params);
        let rx = self.rx.normalize(values, &params);
        let ry = self.ry.normalize(values, &params);

        make_ellipse(cx, cy, rx, ry)
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
