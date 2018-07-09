use cairo;
use std::cell::Cell;
use std::cell::RefCell;

use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use marker;
use node::*;
use parsers::{self, parse, parse_and_validate};
use path_builder::*;
use path_parser;
use property_bag::PropertyBag;
use state::ComputedValues;

fn render_path_builder(
    builder: &PathBuilder,
    draw_ctx: &mut DrawingCtx,
    node: &RsvgNode,
    values: &ComputedValues,
    render_markers: bool,
    clipping: bool,
) {
    draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
        let cr = dc.get_cairo_context();

        dc.set_affine_on_cr(&cr);
        builder.to_cairo(&cr);

        if clipping {
            cr.set_fill_rule(cairo::FillRule::from(values.clip_rule));
        } else {
            cr.set_fill_rule(cairo::FillRule::from(values.fill_rule));
            dc.stroke_and_fill(&cr, values);
        }
    });

    if render_markers {
        marker::render_markers_for_path_builder(builder, draw_ctx, values, clipping);
    }
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
) {
    // Per the spec, rx and ry must be nonnegative
    if rx <= 0.0 || ry <= 0.0 {
        return;
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

    render_path_builder(&builder, draw_ctx, node, values, false, clipping);
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
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            if attr == Attribute::D {
                let mut builder = PathBuilder::new();

                if path_parser::parse_path_into_builder(value, &mut builder).is_err() {
                    // FIXME: we don't propagate errors upstream, but creating a partial
                    // path is OK per the spec
                }

                *self.builder.borrow_mut() = Some(builder);
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        if let Some(ref builder) = *self.builder.borrow() {
            render_path_builder(builder, draw_ctx, node, values, true, clipping);
        }
    }
}

#[derive(Debug, PartialEq)]
enum PolyKind {
    Open,
    Closed,
}

pub struct NodePoly {
    points: RefCell<Option<Vec<(f64, f64)>>>,
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
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            // support for svg < 1.0 which used verts
            if attr == Attribute::Points || attr == Attribute::Verts {
                let result = parsers::list_of_points(value.trim());

                match result {
                    Ok(v) => {
                        *self.points.borrow_mut() = Some(v);
                        break;
                    }

                    Err(e) => {
                        return Err(NodeError::parse_error(attr, e));
                    }
                }
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
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

            render_path_builder(&builder, draw_ctx, node, values, true, clipping);
        }
    }
}

pub struct NodeLine {
    x1: Cell<RsvgLength>,
    y1: Cell<RsvgLength>,
    x2: Cell<RsvgLength>,
    y2: Cell<RsvgLength>,
}

impl NodeLine {
    pub fn new() -> NodeLine {
        NodeLine {
            x1: Cell::new(RsvgLength::default()),
            y1: Cell::new(RsvgLength::default()),
            x2: Cell::new(RsvgLength::default()),
            y2: Cell::new(RsvgLength::default()),
        }
    }
}

impl NodeTrait for NodeLine {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X1 => self.x1.set(parse("x1", value, LengthDir::Horizontal)?),
                Attribute::Y1 => self.y1.set(parse("y1", value, LengthDir::Vertical)?),
                Attribute::X2 => self.x2.set(parse("x2", value, LengthDir::Horizontal)?),
                Attribute::Y2 => self.y2.set(parse("y2", value, LengthDir::Vertical)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let mut builder = PathBuilder::new();

        let x1 = self.x1.get().normalize(values, draw_ctx);
        let y1 = self.y1.get().normalize(values, draw_ctx);
        let x2 = self.x2.get().normalize(values, draw_ctx);
        let y2 = self.y2.get().normalize(values, draw_ctx);

        builder.move_to(x1, y1);
        builder.line_to(x2, y2);

        render_path_builder(&builder, draw_ctx, node, values, true, clipping);
    }
}

pub struct NodeRect {
    // x, y, width, height
    x: Cell<RsvgLength>,
    y: Cell<RsvgLength>,
    w: Cell<RsvgLength>,
    h: Cell<RsvgLength>,

    // Radiuses for rounded corners
    rx: Cell<Option<RsvgLength>>,
    ry: Cell<Option<RsvgLength>>,
}

impl NodeRect {
    pub fn new() -> NodeRect {
        NodeRect {
            x: Cell::new(RsvgLength::default()),
            y: Cell::new(RsvgLength::default()),
            w: Cell::new(RsvgLength::default()),
            h: Cell::new(RsvgLength::default()),

            rx: Cell::new(None),
            ry: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeRect {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),
                Attribute::Width => self.w.set(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    RsvgLength::check_nonnegative,
                )?),
                Attribute::Height => self.h.set(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    RsvgLength::check_nonnegative,
                )?),

                Attribute::Rx => self.rx.set(
                    parse_and_validate(
                        "rx",
                        value,
                        LengthDir::Horizontal,
                        RsvgLength::check_nonnegative,
                    ).map(Some)?,
                ),
                Attribute::Ry => self.ry.set(
                    parse_and_validate(
                        "ry",
                        value,
                        LengthDir::Vertical,
                        RsvgLength::check_nonnegative,
                    ).map(Some)?,
                ),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let x = self.x.get().normalize(values, draw_ctx);
        let y = self.y.get().normalize(values, draw_ctx);

        let w = self.w.get().normalize(values, draw_ctx);
        let h = self.h.get().normalize(values, draw_ctx);

        let mut rx;
        let mut ry;

        match (self.rx.get(), self.ry.get()) {
            (None, None) => {
                rx = 0.0;
                ry = 0.0;
            }

            (Some(_rx), None) => {
                rx = _rx.normalize(values, draw_ctx);
                ry = _rx.normalize(values, draw_ctx);
            }

            (None, Some(_ry)) => {
                rx = _ry.normalize(values, draw_ctx);
                ry = _ry.normalize(values, draw_ctx);
            }

            (Some(_rx), Some(_ry)) => {
                rx = _rx.normalize(values, draw_ctx);
                ry = _ry.normalize(values, draw_ctx);
            }
        }

        // Per the spec, w,h must be >= 0
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // ... and rx,ry must be nonnegative
        if rx < 0.0 || ry < 0.0 {
            return;
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
            let top_y  = y;

            let bottom_x1 = top_x1;
            let bottom_x2 = top_x2;
            let bottom_y  = y + h;

            let left_x  = x;
            let left_y1 = y + ry;
            let left_y2 = y + h - ry;

            let right_x  = x + w;
            let right_y1 = left_y1;
            let right_y2 = left_y2;

            builder.move_to (top_x1, top_y);
            builder.line_to (top_x2, top_y);

            builder.arc (top_x2, top_y,
                         rx, ry, 0.0, LargeArc (false), Sweep::Positive,
                         right_x, right_y1);

            builder.line_to (right_x, right_y2);

            builder.arc (right_x, right_y2,
                         rx, ry, 0.0, LargeArc (false), Sweep::Positive,
                         bottom_x2, bottom_y);

            builder.line_to (bottom_x1, bottom_y);

            builder.arc (bottom_x1, bottom_y,
                         rx, ry, 0.0, LargeArc (false), Sweep::Positive,
                         left_x, left_y2);

            builder.line_to (left_x, left_y1);

            builder.arc (left_x, left_y1,
                         rx, ry, 0.0, LargeArc (false), Sweep::Positive,
                         top_x1, top_y);

            builder.close_path ();
        }

        render_path_builder(&builder, draw_ctx, node, values, false, clipping);
    }
}

pub struct NodeCircle {
    cx: Cell<RsvgLength>,
    cy: Cell<RsvgLength>,
    r: Cell<RsvgLength>,
}

impl NodeCircle {
    pub fn new() -> NodeCircle {
        NodeCircle {
            cx: Cell::new(RsvgLength::default()),
            cy: Cell::new(RsvgLength::default()),
            r: Cell::new(RsvgLength::default()),
        }
    }
}

impl NodeTrait for NodeCircle {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Cx => self.cx.set(parse("cx", value, LengthDir::Horizontal)?),
                Attribute::Cy => self.cy.set(parse("cy", value, LengthDir::Vertical)?),
                Attribute::R => self.r.set(parse_and_validate(
                    "r",
                    value,
                    LengthDir::Both,
                    RsvgLength::check_nonnegative,
                )?),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let cx = self.cx.get().normalize(values, draw_ctx);
        let cy = self.cy.get().normalize(values, draw_ctx);
        let r = self.r.get().normalize(values, draw_ctx);

        render_ellipse(cx, cy, r, r, draw_ctx, node, values, clipping);
    }
}

pub struct NodeEllipse {
    cx: Cell<RsvgLength>,
    cy: Cell<RsvgLength>,
    rx: Cell<RsvgLength>,
    ry: Cell<RsvgLength>,
}

impl NodeEllipse {
    pub fn new() -> NodeEllipse {
        NodeEllipse {
            cx: Cell::new(RsvgLength::default()),
            cy: Cell::new(RsvgLength::default()),
            rx: Cell::new(RsvgLength::default()),
            ry: Cell::new(RsvgLength::default()),
        }
    }
}

impl NodeTrait for NodeEllipse {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::Cx => self.cx.set(parse("cx", value, LengthDir::Horizontal)?),
                Attribute::Cy => self.cy.set(parse("cy", value, LengthDir::Vertical)?),

                Attribute::Rx => self.rx.set(parse_and_validate(
                    "rx",
                    value,
                    LengthDir::Horizontal,
                    RsvgLength::check_nonnegative,
                )?),
                Attribute::Ry => self.ry.set(parse_and_validate(
                    "ry",
                    value,
                    LengthDir::Vertical,
                    RsvgLength::check_nonnegative,
                )?),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) {
        let values = cascaded.get();

        let cx = self.cx.get().normalize(values, draw_ctx);
        let cy = self.cy.get().normalize(values, draw_ctx);
        let rx = self.rx.get().normalize(values, draw_ctx);
        let ry = self.ry.get().normalize(values, draw_ctx);

        render_ellipse(cx, cy, rx, ry, draw_ctx, node, values, clipping);
    }
}
