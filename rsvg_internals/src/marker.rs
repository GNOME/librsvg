use std::cell::Cell;
use std::f64::consts::*;

use cairo::MatrixTrait;
use cssparser::{CowRcStr, Parser, Token};

use aspect_ratio::*;
use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::*;
use float_eq_cairo::ApproxEqCairo;
use handle::RsvgHandle;
use iri::IRI;
use length::{Length, LengthDir};
use node::*;
use parsers;
use parsers::ParseError;
use parsers::{parse, parse_and_validate, Parse};
use path_builder::*;
use property_bag::PropertyBag;
use state::{ComputedValues, SpecifiedValue, State};
use viewbox::*;

// markerUnits attribute: https://www.w3.org/TR/SVG/painting.html#MarkerElement
#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerUnits {
    UserSpaceOnUse,
    StrokeWidth,
}

impl Default for MarkerUnits {
    fn default() -> MarkerUnits {
        MarkerUnits::StrokeWidth
    }
}

impl Parse for MarkerUnits {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<MarkerUnits, ValueErrorKind> {
        let loc = parser.current_source_location();

        parser
            .expect_ident()
            .and_then(|cow| match cow.as_ref() {
                "userSpaceOnUse" => Ok(MarkerUnits::UserSpaceOnUse),
                "strokeWidth" => Ok(MarkerUnits::StrokeWidth),
                _ => Err(
                    loc.new_basic_unexpected_token_error(Token::Ident(CowRcStr::from(
                        cow.as_ref().to_string(),
                    ))),
                ),
            })
            .map_err(|_| {
                ValueErrorKind::Parse(ParseError::new(
                    "expected \"userSpaceOnUse\" or \"strokeWidth\"",
                ))
            })
    }
}

// orient attribute: https://www.w3.org/TR/SVG/painting.html#MarkerElement
#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerOrient {
    Auto,
    Degrees(f64),
}

impl Default for MarkerOrient {
    fn default() -> MarkerOrient {
        MarkerOrient::Degrees(0.0)
    }
}

impl Parse for MarkerOrient {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<MarkerOrient, ValueErrorKind> {
        if parser.try(|p| p.expect_ident_matching("auto")).is_ok() {
            Ok(MarkerOrient::Auto)
        } else {
            parsers::angle_degrees(parser)
                .map(MarkerOrient::Degrees)
                .map_err(ValueErrorKind::Parse)
        }
    }
}

pub struct NodeMarker {
    units: Cell<MarkerUnits>,
    ref_x: Cell<Length>,
    ref_y: Cell<Length>,
    width: Cell<Length>,
    height: Cell<Length>,
    orient: Cell<MarkerOrient>,
    aspect: Cell<AspectRatio>,
    vbox: Cell<Option<ViewBox>>,
}

impl NodeMarker {
    pub fn new() -> NodeMarker {
        NodeMarker {
            units: Cell::new(MarkerUnits::default()),
            ref_x: Cell::new(Length::default()),
            ref_y: Cell::new(Length::default()),
            width: Cell::new(NodeMarker::get_default_size(LengthDir::Horizontal)),
            height: Cell::new(NodeMarker::get_default_size(LengthDir::Vertical)),
            orient: Cell::new(MarkerOrient::default()),
            aspect: Cell::new(AspectRatio::default()),
            vbox: Cell::new(None),
        }
    }

    fn get_default_size(dir: LengthDir) -> Length {
        // per the spec
        Length::parse_str("3", dir).unwrap()
    }

    fn render(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx<'_>,
        xpos: f64,
        ypos: f64,
        computed_angle: f64,
        line_width: f64,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let cascaded = node.get_cascaded_values();
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let marker_width = self.width.get().normalize(&values, &params);
        let marker_height = self.height.get().normalize(&values, &params);

        if marker_width.approx_eq_cairo(&0.0) || marker_height.approx_eq_cairo(&0.0) {
            // markerWidth or markerHeight set to 0 disables rendering of the element
            // https://www.w3.org/TR/SVG/painting.html#MarkerWidthAttribute
            return Ok(());
        }

        let cr = draw_ctx.get_cairo_context();

        let mut affine = cr.get_matrix();

        affine.translate(xpos, ypos);

        let rotation = match self.orient.get() {
            MarkerOrient::Auto => computed_angle,
            MarkerOrient::Degrees(d) => d * PI / 180.0,
        };

        affine.rotate(rotation);

        if self.units.get() == MarkerUnits::StrokeWidth {
            affine.scale(line_width, line_width);
        }

        let params = if let Some(vbox) = self.vbox.get() {
            let (_, _, w, h) = self.aspect.get().compute(
                vbox.0.width,
                vbox.0.height,
                0.0,
                0.0,
                marker_width,
                marker_height,
            );

            if vbox.0.width.approx_eq_cairo(&0.0) || vbox.0.height.approx_eq_cairo(&0.0) {
                return Ok(());
            }

            affine.scale(w / vbox.0.width, h / vbox.0.height);

            draw_ctx.push_view_box(vbox.0.width, vbox.0.height)
        } else {
            draw_ctx.push_view_box(marker_width, marker_height)
        };

        affine.translate(
            -self.ref_x.get().normalize(&values, &params),
            -self.ref_y.get().normalize(&values, &params),
        );

        cr.save();

        cr.set_matrix(affine);

        if !values.is_overflow() {
            if let Some(vbox) = self.vbox.get() {
                draw_ctx.clip(vbox.0.x, vbox.0.y, vbox.0.width, vbox.0.height);
            } else {
                draw_ctx.clip(0.0, 0.0, marker_width, marker_height);
            }
        }

        let res = draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            node.draw_children(&cascaded, dc, clipping)
        });

        cr.restore();

        res
    }
}

impl NodeTrait for NodeMarker {
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // marker element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::MarkerUnits => self.units.set(parse("markerUnits", value, ())?),

                Attribute::RefX => self.ref_x.set(parse("refX", value, LengthDir::Horizontal)?),

                Attribute::RefY => self.ref_y.set(parse("refY", value, LengthDir::Vertical)?),

                Attribute::MarkerWidth => self.width.set(parse_and_validate(
                    "markerWidth",
                    value,
                    LengthDir::Horizontal,
                    Length::check_nonnegative,
                )?),

                Attribute::MarkerHeight => self.height.set(parse_and_validate(
                    "markerHeight",
                    value,
                    LengthDir::Vertical,
                    Length::check_nonnegative,
                )?),

                Attribute::Orient => self.orient.set(parse("orient", value, ())?),

                Attribute::PreserveAspectRatio => {
                    self.aspect.set(parse("preserveAspectRatio", value, ())?)
                }

                Attribute::ViewBox => self.vbox.set(Some(parse("viewBox", value, ())?)),

                _ => (),
            }
        }

        Ok(())
    }

    fn set_overridden_properties(&self, state: &mut State) {
        // markers are always displayed, even if <marker> or its ancestors are display:none
        state.values.display = SpecifiedValue::Specified(Default::default());
    }
}

// Machinery to figure out marker orientations
#[derive(Debug, PartialEq)]
pub enum Segment {
    Degenerate {
        // A single lone point
        x: f64,
        y: f64,
    },

    LineOrCurve {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        x3: f64,
        y3: f64,
        x4: f64,
        y4: f64,
    },
}

enum SegmentState {
    Initial,
    NewSubpath,
    InSubpath,
    ClosedSubpath,
}

// This converts a cairo_path_t into a list of curveto-like segments.  Each segment can be:
// 1. Segment::Degenerate => the segment is actually a single point (x, y)
//
// 2. Segment::LineOrCurve => either a lineto or a curveto (or the effective lineto that results
// from a closepath).    We have the following points:
//       P1 = (x1, y1)
//       P2 = (x2, y2)
//       P3 = (x3, y3)
//       P4 = (x4, y4)
//
//    The start and end points are P1 and P4, respectively.
//    The tangent at the start point is given by the vector (P2 - P1).
//    The tangent at the end point is given by the vector (P4 - P3).
// The tangents also work if the segment refers to a lineto (they will both just point in the
// same direction).
fn make_degenerate(x: f64, y: f64) -> Segment {
    Segment::Degenerate { x, y }
}

fn make_curve(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Segment {
    Segment::LineOrCurve {
        x1,
        y1,
        x2,
        y2,
        x3,
        y3,
        x4,
        y4,
    }
}

fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
    make_curve(x1, y1, x2, y2, x1, y1, x2, y2)
}

pub fn path_builder_to_segments(builder: &PathBuilder) -> Vec<Segment> {
    let mut last_x: f64;
    let mut last_y: f64;
    let mut cur_x: f64;
    let mut cur_y: f64;
    let mut subpath_start_x: f64;
    let mut subpath_start_y: f64;
    let mut segments: Vec<Segment>;
    let mut state: SegmentState;

    cur_x = 0.0;
    cur_y = 0.0;
    subpath_start_x = 0.0;
    subpath_start_y = 0.0;

    segments = Vec::new();
    state = SegmentState::Initial;

    for path_command in builder.get_path_commands() {
        last_x = cur_x;
        last_y = cur_y;

        match *path_command {
            PathCommand::MoveTo(x, y) => {
                cur_x = x;
                cur_y = y;

                subpath_start_x = cur_x;
                subpath_start_y = cur_y;

                match state {
                    SegmentState::Initial | SegmentState::InSubpath => {
                        // Ignore the very first moveto in a sequence (Initial state), or if we
                        // were already drawing within a subpath, start
                        // a new subpath.
                        state = SegmentState::NewSubpath;
                    }

                    SegmentState::NewSubpath => {
                        // We had just begun a new subpath (i.e. from a moveto) and we got
                        // another moveto?  Output a stray point for the
                        // previous moveto.
                        segments.push(make_degenerate(last_x, last_y));
                        state = SegmentState::NewSubpath;
                    }

                    SegmentState::ClosedSubpath => {
                        // Cairo outputs a moveto after every closepath, so that subsequent
                        // lineto/curveto commands will start at the closed vertex.
                        // We don't want to actually emit a point (a degenerate segment) in that
                        // artificial-moveto case.
                        //
                        // We'll reset to the Initial state so that a subsequent "real" moveto will
                        // be handled as the beginning of a new subpath, or a degenerate point, as
                        // usual.
                        state = SegmentState::Initial;
                    }
                }
            }

            PathCommand::LineTo(x, y) => {
                cur_x = x;
                cur_y = y;

                segments.push(make_line(last_x, last_y, cur_x, cur_y));

                state = SegmentState::InSubpath;
            }

            PathCommand::CurveTo(curve) => {
                let CubicBezierCurve {
                    pt1: (x2, y2),
                    pt2: (x3, y3),
                    to,
                } = curve;
                cur_x = to.0;
                cur_y = to.1;

                segments.push(make_curve(last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

                state = SegmentState::InSubpath;
            }

            PathCommand::Arc(arc) => {
                cur_x = arc.to.0;
                cur_y = arc.to.1;

                match arc.center_parameterization() {
                    ArcParameterization::CenterParameters {
                        center,
                        radii,
                        theta1,
                        delta_theta,
                    } => {
                        let rot = arc.x_axis_rotation;
                        let theta2 = theta1 + delta_theta;
                        let n_segs = (delta_theta / (PI * 0.5 + 0.001)).abs().ceil() as u32;
                        let d_theta = delta_theta / f64::from(n_segs);

                        let segment1 = arc_segment(center, radii, rot, theta1, theta1 + d_theta);
                        let segment2 = arc_segment(center, radii, rot, theta2 - d_theta, theta2);

                        let (x2, y2) = segment1.pt1;
                        let (x3, y3) = segment2.pt2;
                        segments.push(make_curve(last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

                        state = SegmentState::InSubpath;
                    }
                    ArcParameterization::LineTo => {
                        segments.push(make_line(last_x, last_y, cur_x, cur_y));

                        state = SegmentState::InSubpath;
                    }
                    ArcParameterization::Omit => {}
                }
            }

            PathCommand::ClosePath => {
                cur_x = subpath_start_x;
                cur_y = subpath_start_y;

                segments.push(make_line(last_x, last_y, cur_x, cur_y));

                state = SegmentState::ClosedSubpath;
            }
        }
    }

    if let SegmentState::NewSubpath = state {
        // Output a lone point if we started a subpath with a moveto
        // command, but there are no subsequent commands.
        segments.push(make_degenerate(cur_x, cur_y));
    };

    segments
}

fn points_equal(x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    x1.approx_eq_cairo(&x2) && y1.approx_eq_cairo(&y2)
}

// If the segment has directionality, returns two vectors (v1x, v1y, v2x, v2y); otherwise,
// returns None.  The vectors are the tangents at the beginning and at the end of the segment,
// respectively.  A segment does not have directionality if it is degenerate (i.e. a single
// point) or a zero-length segment, i.e. where all four control points are coincident (the first
// and last control points may coincide, but the others may define a loop - thus nonzero length)
fn get_segment_directionalities(segment: &Segment) -> Option<(f64, f64, f64, f64)> {
    match *segment {
        Segment::Degenerate { .. } => None,

        Segment::LineOrCurve {
            x1,
            y1,
            x2,
            y2,
            x3,
            y3,
            x4,
            y4,
        } => {
            let coincide_1_and_2 = points_equal(x1, y1, x2, y2);
            let coincide_1_and_3 = points_equal(x1, y1, x3, y3);
            let coincide_1_and_4 = points_equal(x1, y1, x4, y4);
            let coincide_2_and_3 = points_equal(x2, y2, x3, y3);
            let coincide_2_and_4 = points_equal(x2, y2, x4, y4);
            let coincide_3_and_4 = points_equal(x3, y3, x4, y4);

            if coincide_1_and_2 && coincide_1_and_3 && coincide_1_and_4 {
                None
            } else if coincide_1_and_2 && coincide_1_and_3 {
                Some((x4 - x1, y4 - y1, x4 - x3, y4 - y3))
            } else if coincide_1_and_2 && coincide_3_and_4 {
                Some((x4 - x1, y4 - y1, x4 - x1, y4 - y1))
            } else if coincide_2_and_3 && coincide_2_and_4 {
                Some((x2 - x1, y2 - y1, x4 - x1, y4 - y1))
            } else if coincide_1_and_2 {
                Some((x3 - x1, y3 - y1, x4 - x3, y4 - y3))
            } else if coincide_3_and_4 {
                Some((x2 - x1, y2 - y1, x4 - x2, y4 - y2))
            } else {
                Some((x2 - x1, y2 - y1, x4 - x3, y4 - y3))
            }
        }
    }
}

// The SVG spec 1.1 says http://www.w3.org/TR/SVG/implnote.html#PathElementImplementationNotes
// Certain line-capping and line-joining situations and markers
// require that a path segment have directionality at its start and
// end points. Zero-length path segments have no directionality. In
// these cases, the following algorithm is used to establish
// directionality:  to determine the directionality of the start
// point of a zero-length path segment, go backwards in the path
// data specification within the current subpath until you find a
// segment which has directionality at its end point (e.g., a path
// segment with non-zero length) and use its ending direction;
// otherwise, temporarily consider the start point to lack
// directionality. Similarly, to determine the directionality of the
// end point of a zero-length path segment, go forwards in the path
// data specification within the current subpath until you find a
// segment which has directionality at its start point (e.g., a path
// segment with non-zero length) and use its starting direction;
// otherwise, temporarily consider the end point to lack
// directionality. If the start point has directionality but the end
// point doesn't, then the end point uses the start point's
// directionality. If the end point has directionality but the start
// point doesn't, then the start point uses the end point's
// directionality. Otherwise, set the directionality for the path
// segment's start and end points to align with the positive x-axis
// in user space.
fn find_incoming_directionality_backwards(
    segments: &[Segment],
    start_index: usize,
) -> (bool, f64, f64) {
    // "go backwards ... within the current subpath until ... segment which has directionality
    // at its end point"
    for segment in segments[..start_index + 1].iter().rev() {
        match *segment {
            Segment::Degenerate { .. } => {
                return (false, 0.0, 0.0); // reached the beginning of the subpath as we ran into a standalone point
            }

            Segment::LineOrCurve { .. } => match get_segment_directionalities(segment) {
                Some((_, _, v2x, v2y)) => {
                    return (true, v2x, v2y);
                }
                None => {
                    continue;
                }
            },
        }
    }

    (false, 0.0, 0.0)
}

fn find_outgoing_directionality_forwards(
    segments: &[Segment],
    start_index: usize,
) -> (bool, f64, f64) {
    // "go forwards ... within the current subpath until ... segment which has directionality at
    // its start point"
    for segment in &segments[start_index..] {
        match *segment {
            Segment::Degenerate { .. } => {
                return (false, 0.0, 0.0); // reached the end of a subpath as we ran into a standalone point
            }

            Segment::LineOrCurve { .. } => match get_segment_directionalities(segment) {
                Some((v1x, v1y, _, _)) => {
                    return (true, v1x, v1y);
                }
                None => {
                    continue;
                }
            },
        }
    }

    (false, 0.0, 0.0)
}

// Normalizes an angle to [0.0, 2*PI)
fn normalize_angle(mut angle: f64) -> f64 {
    if angle < 0.0 {
        while angle < 0.0 {
            angle += PI * 2.0;
        }
    } else {
        while angle > PI * 2.0 {
            angle -= PI * 2.0;
        }
    }

    angle
}

fn angle_from_vector(vx: f64, vy: f64) -> f64 {
    let angle = vy.atan2(vx);

    if angle.is_nan() {
        0.0
    } else {
        normalize_angle(angle)
    }
}

fn bisect_angles(incoming: f64, outgoing: f64) -> f64 {
    let half_delta: f64;

    half_delta = (outgoing - incoming) * 0.5;

    if FRAC_PI_2 < half_delta.abs() {
        normalize_angle(incoming + half_delta - PI)
    } else {
        normalize_angle(incoming + half_delta)
    }
}

// From SVG's marker-start, marker-mid, marker-end properties
#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerType {
    Start,
    Middle,
    End,
}

fn emit_marker_by_name(
    draw_ctx: &mut DrawingCtx<'_>,
    name: &str,
    xpos: f64,
    ypos: f64,
    computed_angle: f64,
    line_width: f64,
    clipping: bool,
) -> Result<(), RenderingError> {
    if let Some(acquired) = draw_ctx.get_acquired_node_of_type(Some(name), NodeType::Marker) {
        let node = acquired.get();

        node.with_impl(|marker: &NodeMarker| {
            marker.render(
                &node,
                draw_ctx,
                xpos,
                ypos,
                computed_angle,
                line_width,
                clipping,
            )
        })
    } else {
        rsvg_log!("marker \"{}\" not found", name);
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerEndpoint {
    Start,
    End,
}

fn emit_marker<E>(
    segment: &Segment,
    endpoint: MarkerEndpoint,
    marker_type: MarkerType,
    orient: f64,
    emit_fn: &mut E,
) -> Result<(), RenderingError>
where
    E: FnMut(MarkerType, f64, f64, f64) -> Result<(), RenderingError>,
{
    let (x, y) = match *segment {
        Segment::Degenerate { x, y } => (x, y),

        Segment::LineOrCurve { x1, y1, x4, y4, .. } => match endpoint {
            MarkerEndpoint::Start => (x1, y1),
            MarkerEndpoint::End => (x4, y4),
        },
    };

    emit_fn(marker_type, x, y, orient)
}

pub fn render_markers_for_path_builder(
    builder: &PathBuilder,
    draw_ctx: &mut DrawingCtx<'_>,
    values: &ComputedValues,
    clipping: bool,
) -> Result<(), RenderingError> {
    let line_width = values
        .stroke_width
        .0
        .normalize(values, &draw_ctx.get_view_params());

    if line_width.approx_eq_cairo(&0.0) {
        return Ok(());
    }

    let marker_start = &values.marker_start.0;
    let marker_mid = &values.marker_mid.0;
    let marker_end = &values.marker_end.0;

    match (marker_start, marker_mid, marker_end) {
        (&IRI::None, &IRI::None, &IRI::None) => return Ok(()),
        _ => (),
    }

    emit_markers_for_path_builder(
        builder,
        &mut |marker_type: MarkerType, x: f64, y: f64, computed_angle: f64| {
            if let &IRI::Resource(ref marker) = match marker_type {
                MarkerType::Start => &values.marker_start.0,
                MarkerType::Middle => &values.marker_mid.0,
                MarkerType::End => &values.marker_end.0,
            } {
                emit_marker_by_name(
                    draw_ctx,
                    &marker,
                    x,
                    y,
                    computed_angle,
                    line_width,
                    clipping,
                )
            } else {
                Ok(())
            }
        },
    )
}

fn emit_markers_for_path_builder<E>(
    builder: &PathBuilder,
    emit_fn: &mut E,
) -> Result<(), RenderingError>
where
    E: FnMut(MarkerType, f64, f64, f64) -> Result<(), RenderingError>,
{
    enum SubpathState {
        NoSubpath,
        InSubpath,
    };

    // Convert the path to a list of segments and bare points
    let segments = path_builder_to_segments(builder);

    let mut subpath_state = SubpathState::NoSubpath;

    for (i, segment) in segments.iter().enumerate() {
        match *segment {
            Segment::Degenerate { .. } => {
                if let SubpathState::InSubpath = subpath_state {
                    assert!(i > 0);

                    // Got a lone point after a subpath; render the subpath's end marker first
                    let (_, incoming_vx, incoming_vy) =
                        find_incoming_directionality_backwards(&segments, i - 1);
                    emit_marker(
                        &segments[i - 1],
                        MarkerEndpoint::End,
                        MarkerType::End,
                        angle_from_vector(incoming_vx, incoming_vy),
                        emit_fn,
                    )?;
                }

                // Render marker for the lone point; no directionality
                emit_marker(
                    segment,
                    MarkerEndpoint::Start,
                    MarkerType::Middle,
                    0.0,
                    emit_fn,
                )?;

                subpath_state = SubpathState::NoSubpath;
            }

            Segment::LineOrCurve { .. } => {
                // Not a degenerate segment
                match subpath_state {
                    SubpathState::NoSubpath => {
                        let (_, outgoing_vx, outgoing_vy) =
                            find_outgoing_directionality_forwards(&segments, i);
                        emit_marker(
                            segment,
                            MarkerEndpoint::Start,
                            MarkerType::Start,
                            angle_from_vector(outgoing_vx, outgoing_vy),
                            emit_fn,
                        )?;

                        subpath_state = SubpathState::InSubpath;
                    }

                    SubpathState::InSubpath => {
                        assert!(i > 0);

                        let (has_incoming, incoming_vx, incoming_vy) =
                            find_incoming_directionality_backwards(&segments, i - 1);
                        let (has_outgoing, outgoing_vx, outgoing_vy) =
                            find_outgoing_directionality_forwards(&segments, i);

                        let incoming: f64;
                        let outgoing: f64;

                        incoming = angle_from_vector(incoming_vx, incoming_vy);
                        outgoing = angle_from_vector(outgoing_vx, outgoing_vy);

                        let angle: f64;

                        if has_incoming && has_outgoing {
                            angle = bisect_angles(incoming, outgoing);
                        } else if has_incoming {
                            angle = incoming;
                        } else if has_outgoing {
                            angle = outgoing;
                        } else {
                            angle = 0.0;
                        }

                        emit_marker(
                            segment,
                            MarkerEndpoint::Start,
                            MarkerType::Middle,
                            angle,
                            emit_fn,
                        )?;
                    }
                }
            }
        }
    }

    // Finally, render the last point
    if !segments.is_empty() {
        let segment = &segments[segments.len() - 1];
        if let Segment::LineOrCurve { .. } = *segment {
            let (_, incoming_vx, incoming_vy) =
                find_incoming_directionality_backwards(&segments, segments.len() - 1);

            let angle = {
                if let PathCommand::ClosePath = builder.get_path_commands()[segments.len()] {
                    let (_, outgoing_vx, outgoing_vy) =
                        find_outgoing_directionality_forwards(&segments, 0);
                    bisect_angles(
                        angle_from_vector(incoming_vx, incoming_vy),
                        angle_from_vector(outgoing_vx, outgoing_vy),
                    )
                } else {
                    angle_from_vector(incoming_vx, incoming_vy)
                }
            };

            emit_marker(
                segment,
                MarkerEndpoint::End,
                MarkerType::End,
                angle,
                emit_fn,
            )?;
        }
    }

    Ok(())
}

// ************************************  Tests ************************************
#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parsing_invalid_marker_units_yields_error() {
        assert!(is_parse_error(
            &MarkerUnits::parse_str("", ()).map_err(|e| ValueErrorKind::from(e))
        ));
        assert!(is_parse_error(
            &MarkerUnits::parse_str("foo", ()).map_err(|e| ValueErrorKind::from(e))
        ));
    }

    #[test]
    fn parses_marker_units() {
        assert_eq!(
            MarkerUnits::parse_str("userSpaceOnUse", ()),
            Ok(MarkerUnits::UserSpaceOnUse)
        );
        assert_eq!(
            MarkerUnits::parse_str("strokeWidth", ()),
            Ok(MarkerUnits::StrokeWidth)
        );
    }

    #[test]
    fn parsing_invalid_marker_orient_yields_error() {
        assert!(is_parse_error(
            &MarkerOrient::parse_str("", ()).map_err(|e| ValueErrorKind::from(e))
        ));
        assert!(is_parse_error(
            &MarkerOrient::parse_str("blah", ()).map_err(|e| ValueErrorKind::from(e))
        ));
        assert!(is_parse_error(
            &MarkerOrient::parse_str("45blah", ()).map_err(|e| ValueErrorKind::from(e))
        ));
    }

    #[test]
    fn parses_marker_orient() {
        assert_eq!(MarkerOrient::parse_str("auto", ()), Ok(MarkerOrient::Auto));

        assert_eq!(
            MarkerOrient::parse_str("0", ()),
            Ok(MarkerOrient::Degrees(0.0))
        );
        assert_eq!(
            MarkerOrient::parse_str("180", ()),
            Ok(MarkerOrient::Degrees(180.0))
        );
        assert_eq!(
            MarkerOrient::parse_str("180deg", ()),
            Ok(MarkerOrient::Degrees(180.0))
        );
        assert_eq!(
            MarkerOrient::parse_str("-400grad", ()),
            Ok(MarkerOrient::Degrees(-360.0))
        );
        assert_eq!(
            MarkerOrient::parse_str("1rad", ()),
            Ok(MarkerOrient::Degrees(180.0 / PI))
        );
    }
}

#[cfg(test)]
mod directionality_tests {
    use super::*;
    use float_cmp::ApproxEq;
    use std::f64;

    fn test_bisection_angle(
        expected: f64,
        incoming_vx: f64,
        incoming_vy: f64,
        outgoing_vx: f64,
        outgoing_vy: f64,
    ) {
        let bisected = super::bisect_angles(
            super::angle_from_vector(incoming_vx, incoming_vy),
            super::angle_from_vector(outgoing_vx, outgoing_vy),
        );
        assert!(expected.approx_eq(&bisected, 2.0 * PI * f64::EPSILON, 1));
    }

    #[test]
    fn bisection_angle_is_correct_from_incoming_counterclockwise_to_outgoing() {
        // 1st quadrant
        test_bisection_angle(FRAC_PI_4, 1.0, 0.0, 0.0, 1.0);

        // 2nd quadrant
        test_bisection_angle(FRAC_PI_2 + FRAC_PI_4, 0.0, 1.0, -1.0, 0.0);

        // 3rd quadrant
        test_bisection_angle(PI + FRAC_PI_4, -1.0, 0.0, 0.0, -1.0);

        // 4th quadrant
        test_bisection_angle(PI + FRAC_PI_2 + FRAC_PI_4, 0.0, -1.0, 1.0, 0.0);
    }

    #[test]
    fn bisection_angle_is_correct_from_incoming_clockwise_to_outgoing() {
        // 1st quadrant
        test_bisection_angle(FRAC_PI_4, 0.0, 1.0, 1.0, 0.0);

        // 2nd quadrant
        test_bisection_angle(FRAC_PI_2 + FRAC_PI_4, -1.0, 0.0, 0.0, 1.0);

        // 3rd quadrant
        test_bisection_angle(PI + FRAC_PI_4, 0.0, -1.0, -1.0, 0.0);

        // 4th quadrant
        test_bisection_angle(PI + FRAC_PI_2 + FRAC_PI_4, 1.0, 0.0, 0.0, -1.0);
    }

    #[test]
    fn bisection_angle_is_correct_for_more_than_quarter_turn_angle() {
        test_bisection_angle(0.0, 0.1, -1.0, 0.1, 1.0);

        test_bisection_angle(FRAC_PI_2, 1.0, 0.1, -1.0, 0.1);

        test_bisection_angle(PI, -0.1, 1.0, -0.1, -1.0);

        test_bisection_angle(PI + FRAC_PI_2, -1.0, -0.1, 1.0, -0.1);
    }

    fn degenerate(x: f64, y: f64) -> Segment {
        super::make_degenerate(x, y)
    }

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        super::make_line(x1, y1, x2, y2)
    }

    fn curve(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Segment {
        super::make_curve(x1, y1, x2, y2, x3, y3, x4, y4)
    }

    fn test_path_builder_to_segments(builder: &PathBuilder, expected_segments: Vec<Segment>) {
        let segments = path_builder_to_segments(builder);
        assert_eq!(expected_segments, segments);
    }

    // Single open path; the easy case

    fn setup_open_path() -> PathBuilder {
        let mut builder = PathBuilder::new();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_open_path() {
        let expected_segments: Vec<Segment> =
            vec![line(10.0, 10.0, 20.0, 10.0), line(20.0, 10.0, 20.0, 20.0)];

        test_path_builder_to_segments(&setup_open_path(), expected_segments);
    }

    fn setup_multiple_open_subpaths() -> PathBuilder {
        let mut builder = PathBuilder::new();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);

        builder.move_to(30.0, 30.0);
        builder.line_to(40.0, 30.0);
        builder.curve_to(50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to(80.0, 90.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_multiple_open_subpaths() {
        let expected_segments: Vec<Segment> = vec![
            line(10.0, 10.0, 20.0, 10.0),
            line(20.0, 10.0, 20.0, 20.0),
            line(30.0, 30.0, 40.0, 30.0),
            curve(40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            line(70.0, 70.0, 80.0, 90.0),
        ];

        test_path_builder_to_segments(&setup_multiple_open_subpaths(), expected_segments);
    }

    // Closed subpath; must have a line segment back to the first point
    fn setup_closed_subpath() -> PathBuilder {
        let mut builder = PathBuilder::new();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        builder
    }

    #[test]
    fn path_to_segments_handles_closed_subpath() {
        let expected_segments: Vec<Segment> = vec![
            line(10.0, 10.0, 20.0, 10.0),
            line(20.0, 10.0, 20.0, 20.0),
            line(20.0, 20.0, 10.0, 10.0),
        ];

        test_path_builder_to_segments(&setup_closed_subpath(), expected_segments);
    }

    // Multiple closed subpaths; each must have a line segment back to their
    // initial points, with no degenerate segments between subpaths.
    fn setup_multiple_closed_subpaths() -> PathBuilder {
        let mut builder = PathBuilder::new();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        builder.move_to(30.0, 30.0);
        builder.line_to(40.0, 30.0);
        builder.curve_to(50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to(80.0, 90.0);
        builder.close_path();

        builder
    }

    #[test]
    fn path_to_segments_handles_multiple_closed_subpaths() {
        let expected_segments: Vec<Segment> = vec![
            line(10.0, 10.0, 20.0, 10.0),
            line(20.0, 10.0, 20.0, 20.0),
            line(20.0, 20.0, 10.0, 10.0),
            line(30.0, 30.0, 40.0, 30.0),
            curve(40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            line(70.0, 70.0, 80.0, 90.0),
            line(80.0, 90.0, 30.0, 30.0),
        ];

        test_path_builder_to_segments(&setup_multiple_closed_subpaths(), expected_segments);
    }

    // A lineto follows the first closed subpath, with no moveto to start the second subpath.
    // The lineto must start at the first point of the first subpath.
    fn setup_no_moveto_after_closepath() -> PathBuilder {
        let mut builder = PathBuilder::new();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        builder.line_to(40.0, 30.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_no_moveto_after_closepath() {
        let expected_segments: Vec<Segment> = vec![
            line(10.0, 10.0, 20.0, 10.0),
            line(20.0, 10.0, 20.0, 20.0),
            line(20.0, 20.0, 10.0, 10.0),
            line(10.0, 10.0, 40.0, 30.0),
        ];

        test_path_builder_to_segments(&setup_no_moveto_after_closepath(), expected_segments);
    }

    // Sequence of moveto; should generate degenerate points.
    // This test is not enabled right now!  We create the
    // path fixtures with Cairo, and Cairo compresses
    // sequences of moveto into a single one.  So, we can't
    // really test this, as we don't get the fixture we want.
    //
    // Eventually we'll probably have to switch librsvg to
    // its own internal path representation which should
    // allow for unelided path commands, and which should
    // only build a cairo_path_t for the final rendering step.
    //
    // fn setup_sequence_of_moveto () -> PathBuilder {
    // let mut builder = PathBuilder::new ();
    //
    // builder.move_to (10.0, 10.0);
    // builder.move_to (20.0, 20.0);
    // builder.move_to (30.0, 30.0);
    // builder.move_to (40.0, 40.0);
    //
    // builder
    // }
    //
    // #[test]
    // fn path_to_segments_handles_sequence_of_moveto () {
    // let expected_segments: Vec<Segment> = vec! [
    // degenerate (10.0, 10.0),
    // degenerate (20.0, 20.0),
    // degenerate (30.0, 30.0),
    // degenerate (40.0, 40.0)
    // ];
    //
    // test_path_builder_to_segments (&setup_sequence_of_moveto (), expected_segments);
    // }

    #[test]
    fn degenerate_segment_has_no_directionality() {
        assert!(super::get_segment_directionalities(&degenerate(1.0, 2.0)).is_none());
    }

    #[test]
    fn line_segment_has_directionality() {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&line(1.0, 2.0, 3.0, 4.0)).unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((2.0, 2.0), (v2x, v2y));
    }

    #[test]
    fn line_segment_with_coincident_ends_has_no_directionality() {
        assert!(super::get_segment_directionalities(&line(1.0, 2.0, 1.0, 2.0)).is_none());
    }

    #[test]
    fn curve_has_directionality() {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 20.0, 33.0))
                .unwrap();
        assert_eq!((2.0, 3.0), (v1x, v1y));
        assert_eq!((12.0, 20.0), (v2x, v2y));
    }

    #[test]
    fn curves_with_loops_and_coincident_ends_have_directionality() {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 2.0))
                .unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-4.0, -4.0), (v2x, v2y));

        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0))
                .unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-2.0, -2.0), (v2x, v2y));

        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 1.0, 2.0))
                .unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-2.0, -2.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_coincident_control_points_has_no_directionality() {
        assert!(super::get_segment_directionalities(&curve(
            1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0
        ))
        .is_none());
    }

    #[test]
    fn curve_with_123_coincident_has_directionality() {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 20.0, 40.0))
                .unwrap();
        assert_eq!((20.0, 40.0), (v1x, v1y));
        assert_eq!((20.0, 40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_234_coincident_has_directionality() {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities(&curve(20.0, 40.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                .unwrap();

        assert_eq!((-20.0, -40.0), (v1x, v1y));
        assert_eq!((-20.0, -40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_12_34_coincident_has_directionality() {
        let (v1x, v1y, v2x, v2y) = super::get_segment_directionalities(&curve(
            20.0, 40.0, 20.0, 40.0, 60.0, 70.0, 60.0, 70.0,
        ))
        .unwrap();

        assert_eq!((40.0, 30.0), (v1x, v1y));
        assert_eq!((40.0, 30.0), (v2x, v2y));
    }
}

#[cfg(test)]
mod marker_tests {
    use super::*;

    #[test]
    fn emits_for_open_subpath() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.line_to(1.0, 0.0);
        builder.line_to(1.0, 1.0);
        builder.line_to(0.0, 1.0);

        let mut v = Vec::new();

        assert!(emit_markers_for_path_builder(
            &builder,
            &mut |marker_type: MarkerType,
                  x: f64,
                  y: f64,
                  computed_angle: f64|
             -> Result<(), RenderingError> {
                v.push((marker_type, x, y, computed_angle));
                Ok(())
            }
        )
        .is_ok());

        assert_eq!(
            v,
            vec![
                (MarkerType::Start, 0.0, 0.0, 0.0),
                (MarkerType::Middle, 1.0, 0.0, angle_from_vector(1.0, 1.0)),
                (MarkerType::Middle, 1.0, 1.0, angle_from_vector(-1.0, 1.0)),
                (MarkerType::End, 0.0, 1.0, angle_from_vector(-1.0, 0.0)),
            ]
        );
    }

    #[test]
    fn emits_for_closed_subpath() {
        let mut builder = PathBuilder::new();
        builder.move_to(0.0, 0.0);
        builder.line_to(1.0, 0.0);
        builder.line_to(1.0, 1.0);
        builder.line_to(0.0, 1.0);
        builder.close_path();

        let mut v = Vec::new();

        assert!(emit_markers_for_path_builder(
            &builder,
            &mut |marker_type: MarkerType,
                  x: f64,
                  y: f64,
                  computed_angle: f64|
             -> Result<(), RenderingError> {
                v.push((marker_type, x, y, computed_angle));
                Ok(())
            }
        )
        .is_ok());

        assert_eq!(
            v,
            vec![
                (MarkerType::Start, 0.0, 0.0, 0.0),
                (MarkerType::Middle, 1.0, 0.0, angle_from_vector(1.0, 1.0)),
                (MarkerType::Middle, 1.0, 1.0, angle_from_vector(-1.0, 1.0)),
                (MarkerType::Middle, 0.0, 1.0, angle_from_vector(-1.0, -1.0)),
                (MarkerType::End, 0.0, 0.0, angle_from_vector(1.0, -1.0)),
            ]
        );
    }
}
