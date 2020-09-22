//! The `marker` element, and geometry computations for markers.

use std::f64::consts::*;
use std::ops::Deref;

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::Fragment;
use crate::angle::Angle;
use crate::aspect_ratio::*;
use crate::attributes::Attributes;
use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::float_eq_cairo::ApproxEqCairo;
use crate::iri::IRI;
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::parsers::{Parse, ParseValue};
use crate::path_builder::{arc_segment, ArcParameterization, CubicBezierCurve, Path, PathCommand};
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::transform::Transform;
use crate::viewbox::*;

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
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<MarkerUnits, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "userSpaceOnUse" => MarkerUnits::UserSpaceOnUse,
            "strokeWidth" => MarkerUnits::StrokeWidth,
        )?)
    }
}

// orient attribute: https://www.w3.org/TR/SVG/painting.html#MarkerElement
#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerOrient {
    Auto,
    Angle(Angle),
}

impl Default for MarkerOrient {
    fn default() -> MarkerOrient {
        MarkerOrient::Angle(Angle::new(0.0))
    }
}

impl Parse for MarkerOrient {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<MarkerOrient, ParseError<'i>> {
        parser
            .try_parse(|p| p.expect_ident_matching("auto").map(|_| MarkerOrient::Auto))
            .or_else(|_| Angle::parse(parser).map(MarkerOrient::Angle))
    }
}

pub struct Marker {
    units: MarkerUnits,
    ref_x: Length<Horizontal>,
    ref_y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,
    orient: MarkerOrient,
    aspect: AspectRatio,
    vbox: Option<ViewBox>,
}

impl Default for Marker {
    fn default() -> Marker {
        Marker {
            units: MarkerUnits::default(),
            ref_x: Default::default(),
            ref_y: Default::default(),
            // the following two are per the spec
            width: Length::<Horizontal>::parse_str("3").unwrap(),
            height: Length::<Vertical>::parse_str("3").unwrap(),
            orient: MarkerOrient::default(),
            aspect: AspectRatio::default(),
            vbox: None,
        }
    }
}

impl Marker {
    fn render(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        draw_ctx: &mut DrawingCtx,
        xpos: f64,
        ypos: f64,
        computed_angle: Angle,
        line_width: f64,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let cascaded = CascadedValues::new_from_node(&node);
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let marker_width = self.width.normalize(&values, &params);
        let marker_height = self.height.normalize(&values, &params);

        if marker_width.approx_eq_cairo(0.0) || marker_height.approx_eq_cairo(0.0) {
            // markerWidth or markerHeight set to 0 disables rendering of the element
            // https://www.w3.org/TR/SVG/painting.html#MarkerWidthAttribute
            return Ok(draw_ctx.empty_bbox());
        }

        let rotation = match self.orient {
            MarkerOrient::Auto => computed_angle,
            MarkerOrient::Angle(a) => a,
        };

        let mut transform = Transform::new_translate(xpos, ypos).pre_rotate(rotation);

        if self.units == MarkerUnits::StrokeWidth {
            transform = transform.pre_scale(line_width, line_width);
        }

        let params = if let Some(vbox) = self.vbox {
            if vbox.is_empty() {
                return Ok(draw_ctx.empty_bbox());
            }

            let r = self
                .aspect
                .compute(&vbox, &Rect::from_size(marker_width, marker_height));

            let (vb_width, vb_height) = vbox.size();
            transform = transform.pre_scale(r.width() / vb_width, r.height() / vb_height);

            draw_ctx.push_view_box(vb_width, vb_height)
        } else {
            draw_ctx.push_view_box(marker_width, marker_height)
        };

        transform = transform.pre_translate(
            -self.ref_x.normalize(&values, &params),
            -self.ref_y.normalize(&values, &params),
        );

        let clip = if values.is_overflow() {
            None
        } else {
            Some(
                self.vbox
                    .map_or_else(|| Rect::from_size(marker_width, marker_height), |vb| *vb),
            )
        };

        draw_ctx.with_saved_transform(Some(transform), &mut |dc| {
            dc.with_clip_rect(clip, &mut |dc| {
                dc.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
                    node.draw_children(an, &cascaded, dc, clipping)
                })
            })
        })
    }
}

impl SetAttributes for Marker {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "markerUnits") => self.units = attr.parse(value)?,
                expanded_name!("", "refX") => self.ref_x = attr.parse(value)?,
                expanded_name!("", "refY") => self.ref_y = attr.parse(value)?,
                expanded_name!("", "markerWidth") => {
                    self.width =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!("", "markerHeight") => {
                    self.height =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
                expanded_name!("", "orient") => self.orient = attr.parse(value)?,
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,
                expanded_name!("", "viewBox") => self.vbox = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Marker {}

// Machinery to figure out marker orientations
#[derive(Debug, PartialEq)]
enum Segment {
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

impl Segment {
    fn degenerate(x: f64, y: f64) -> Segment {
        Segment::Degenerate { x, y }
    }

    fn curve(x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Segment {
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

    fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        Segment::curve(x1, y1, x2, y2, x1, y1, x2, y2)
    }

    // If the segment has directionality, returns two vectors (v1x, v1y, v2x, v2y); otherwise,
    // returns None.  The vectors are the tangents at the beginning and at the end of the segment,
    // respectively.  A segment does not have directionality if it is degenerate (i.e. a single
    // point) or a zero-length segment, i.e. where all four control points are coincident (the first
    // and last control points may coincide, but the others may define a loop - thus nonzero length)
    fn get_directionalities(&self) -> Option<(f64, f64, f64, f64)> {
        match *self {
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
}

fn points_equal(x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    x1.approx_eq_cairo(x2) && y1.approx_eq_cairo(y2)
}

enum SegmentState {
    Initial,
    NewSubpath,
    InSubpath,
    ClosedSubpath,
}

#[derive(Debug, PartialEq)]
struct Segments(Vec<Segment>);

impl Deref for Segments {
    type Target = [Segment];

    fn deref(&self) -> &[Segment] {
        &self.0
    }
}

// This converts a path builder into a vector of curveto-like segments.
// Each segment can be:
//
// 1. Segment::Degenerate => the segment is actually a single point (x, y)
//
// 2. Segment::LineOrCurve => either a lineto or a curveto (or the effective
// lineto that results from a closepath).
// We have the following points:
//       P1 = (x1, y1)
//       P2 = (x2, y2)
//       P3 = (x3, y3)
//       P4 = (x4, y4)
//
// The start and end points are P1 and P4, respectively.
// The tangent at the start point is given by the vector (P2 - P1).
// The tangent at the end point is given by the vector (P4 - P3).
// The tangents also work if the segment refers to a lineto (they will
// both just point in the same direction).
impl From<&Path> for Segments {
    fn from(path: &Path) -> Segments {
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

        for path_command in path.iter() {
            last_x = cur_x;
            last_y = cur_y;

            match path_command {
                PathCommand::MoveTo(x, y) => {
                    cur_x = x;
                    cur_y = y;

                    subpath_start_x = cur_x;
                    subpath_start_y = cur_y;

                    match state {
                        SegmentState::Initial | SegmentState::InSubpath => {
                            // Ignore the very first moveto in a sequence (Initial state),
                            // or if we were already drawing within a subpath, start
                            // a new subpath.
                            state = SegmentState::NewSubpath;
                        }

                        SegmentState::NewSubpath => {
                            // We had just begun a new subpath (i.e. from a moveto) and we got
                            // another moveto?  Output a stray point for the
                            // previous moveto.
                            segments.push(Segment::degenerate(last_x, last_y));
                            state = SegmentState::NewSubpath;
                        }

                        SegmentState::ClosedSubpath => {
                            // Cairo outputs a moveto after every closepath, so that subsequent
                            // lineto/curveto commands will start at the closed vertex.
                            // We don't want to actually emit a point (a degenerate segment) in
                            // that artificial-moveto case.
                            //
                            // We'll reset to the Initial state so that a subsequent "real"
                            // moveto will be handled as the beginning of a new subpath, or a
                            // degenerate point, as usual.
                            state = SegmentState::Initial;
                        }
                    }
                }

                PathCommand::LineTo(x, y) => {
                    cur_x = x;
                    cur_y = y;

                    segments.push(Segment::line(last_x, last_y, cur_x, cur_y));

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

                    segments.push(Segment::curve(last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

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

                            let segment1 =
                                arc_segment(center, radii, rot, theta1, theta1 + d_theta);
                            let segment2 =
                                arc_segment(center, radii, rot, theta2 - d_theta, theta2);

                            let (x2, y2) = segment1.pt1;
                            let (x3, y3) = segment2.pt2;
                            segments
                                .push(Segment::curve(last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

                            state = SegmentState::InSubpath;
                        }
                        ArcParameterization::LineTo => {
                            segments.push(Segment::line(last_x, last_y, cur_x, cur_y));

                            state = SegmentState::InSubpath;
                        }
                        ArcParameterization::Omit => {}
                    }
                }

                PathCommand::ClosePath => {
                    cur_x = subpath_start_x;
                    cur_y = subpath_start_y;

                    segments.push(Segment::line(last_x, last_y, cur_x, cur_y));

                    state = SegmentState::ClosedSubpath;
                }
            }
        }

        if let SegmentState::NewSubpath = state {
            // Output a lone point if we started a subpath with a moveto
            // command, but there are no subsequent commands.
            segments.push(Segment::degenerate(cur_x, cur_y));
        };

        Segments(segments)
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
impl Segments {
    fn find_incoming_angle_backwards(&self, start_index: usize) -> Option<Angle> {
        // "go backwards ... within the current subpath until ... segment which has directionality
        // at its end point"
        for segment in self[..=start_index].iter().rev() {
            match *segment {
                Segment::Degenerate { .. } => {
                    return None; // reached the beginning of the subpath as we ran into a standalone point
                }

                Segment::LineOrCurve { .. } => match segment.get_directionalities() {
                    Some((_, _, v2x, v2y)) => {
                        return Some(Angle::from_vector(v2x, v2y));
                    }
                    None => {
                        continue;
                    }
                },
            }
        }

        None
    }

    fn find_outgoing_angle_forwards(&self, start_index: usize) -> Option<Angle> {
        // "go forwards ... within the current subpath until ... segment which has directionality at
        // its start point"
        for segment in &self[start_index..] {
            match *segment {
                Segment::Degenerate { .. } => {
                    return None; // reached the end of a subpath as we ran into a standalone point
                }

                Segment::LineOrCurve { .. } => match segment.get_directionalities() {
                    Some((v1x, v1y, _, _)) => {
                        return Some(Angle::from_vector(v1x, v1y));
                    }
                    None => {
                        continue;
                    }
                },
            }
        }

        None
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
    draw_ctx: &mut DrawingCtx,
    acquired_nodes: &mut AcquiredNodes,
    name: &Fragment,
    xpos: f64,
    ypos: f64,
    computed_angle: Angle,
    line_width: f64,
    clipping: bool,
) -> Result<BoundingBox, RenderingError> {
    if let Ok(acquired) = acquired_nodes.acquire(name) {
        let node = acquired.get();

        match *node.borrow_element() {
            Element::Marker(ref m) => m.render(
                &node,
                acquired_nodes,
                draw_ctx,
                xpos,
                ypos,
                computed_angle,
                line_width,
                clipping,
            ),
            _ => {
                rsvg_log!("\"{}\" is not marker", name);
                Ok(draw_ctx.empty_bbox())
            }
        }
    } else {
        rsvg_log!("marker \"{}\" not found", name);
        Ok(draw_ctx.empty_bbox())
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
    orient: Angle,
    emit_fn: &mut E,
) -> Result<BoundingBox, RenderingError>
where
    E: FnMut(MarkerType, f64, f64, Angle) -> Result<BoundingBox, RenderingError>,
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

pub fn render_markers_for_path(
    path: &Path,
    draw_ctx: &mut DrawingCtx,
    acquired_nodes: &mut AcquiredNodes,
    values: &ComputedValues,
    clipping: bool,
) -> Result<BoundingBox, RenderingError> {
    let line_width = values
        .stroke_width()
        .0
        .normalize(values, &draw_ctx.get_view_params());

    if line_width.approx_eq_cairo(0.0) {
        return Ok(draw_ctx.empty_bbox());
    }

    let marker_start = values.marker_start().0;
    let marker_mid = values.marker_mid().0;
    let marker_end = values.marker_end().0;

    if let (&IRI::None, &IRI::None, &IRI::None) = (&marker_start, &marker_mid, &marker_end) {
        return Ok(draw_ctx.empty_bbox());
    }

    emit_markers_for_path(
        path,
        draw_ctx.empty_bbox(),
        &mut |marker_type: MarkerType, x: f64, y: f64, computed_angle: Angle| {
            if let IRI::Resource(ref marker) = match marker_type {
                MarkerType::Start => &marker_start,
                MarkerType::Middle => &marker_mid,
                MarkerType::End => &marker_end,
            } {
                emit_marker_by_name(
                    draw_ctx,
                    acquired_nodes,
                    marker,
                    x,
                    y,
                    computed_angle,
                    line_width,
                    clipping,
                )
            } else {
                Ok(draw_ctx.empty_bbox())
            }
        },
    )
}

fn emit_markers_for_path<E>(
    path: &Path,
    empty_bbox: BoundingBox,
    emit_fn: &mut E,
) -> Result<BoundingBox, RenderingError>
where
    E: FnMut(MarkerType, f64, f64, Angle) -> Result<BoundingBox, RenderingError>,
{
    enum SubpathState {
        NoSubpath,
        InSubpath,
    };

    let mut bbox = empty_bbox;

    // Convert the path to a list of segments and bare points
    let segments = Segments::from(path);

    let mut subpath_state = SubpathState::NoSubpath;

    for (i, segment) in segments.iter().enumerate() {
        match *segment {
            Segment::Degenerate { .. } => {
                if let SubpathState::InSubpath = subpath_state {
                    assert!(i > 0);

                    // Got a lone point after a subpath; render the subpath's end marker first
                    let angle = segments
                        .find_incoming_angle_backwards(i - 1)
                        .unwrap_or_else(|| Angle::new(0.0));
                    let marker_bbox = emit_marker(
                        &segments[i - 1],
                        MarkerEndpoint::End,
                        MarkerType::End,
                        angle,
                        emit_fn,
                    )?;
                    bbox.insert(&marker_bbox);
                }

                // Render marker for the lone point; no directionality
                let marker_bbox = emit_marker(
                    segment,
                    MarkerEndpoint::Start,
                    MarkerType::Middle,
                    Angle::new(0.0),
                    emit_fn,
                )?;
                bbox.insert(&marker_bbox);

                subpath_state = SubpathState::NoSubpath;
            }

            Segment::LineOrCurve { .. } => {
                // Not a degenerate segment
                match subpath_state {
                    SubpathState::NoSubpath => {
                        let angle = segments
                            .find_outgoing_angle_forwards(i)
                            .unwrap_or_else(|| Angle::new(0.0));
                        let marker_bbox = emit_marker(
                            segment,
                            MarkerEndpoint::Start,
                            MarkerType::Start,
                            angle,
                            emit_fn,
                        )?;
                        bbox.insert(&marker_bbox);

                        subpath_state = SubpathState::InSubpath;
                    }

                    SubpathState::InSubpath => {
                        assert!(i > 0);

                        let incoming = segments.find_incoming_angle_backwards(i - 1);
                        let outgoing = segments.find_outgoing_angle_forwards(i);

                        let angle = match (incoming, outgoing) {
                            (Some(incoming), Some(outgoing)) => incoming.bisect(outgoing),
                            (Some(incoming), _) => incoming,
                            (_, Some(outgoing)) => outgoing,
                            _ => Angle::new(0.0),
                        };

                        let marker_bbox = emit_marker(
                            segment,
                            MarkerEndpoint::Start,
                            MarkerType::Middle,
                            angle,
                            emit_fn,
                        )?;
                        bbox.insert(&marker_bbox);
                    }
                }
            }
        }
    }

    // Finally, render the last point
    if !segments.is_empty() {
        let segment = &segments[segments.len() - 1];
        if let Segment::LineOrCurve { .. } = *segment {
            let incoming = segments
                .find_incoming_angle_backwards(segments.len() - 1)
                .unwrap_or_else(|| Angle::new(0.0));

            let angle = {
                if let PathCommand::ClosePath = path.iter().nth(segments.len()).unwrap() {
                    let outgoing = segments
                        .find_outgoing_angle_forwards(0)
                        .unwrap_or_else(|| Angle::new(0.0));
                    incoming.bisect(outgoing)
                } else {
                    incoming
                }
            };

            let marker_bbox = emit_marker(
                segment,
                MarkerEndpoint::End,
                MarkerType::End,
                angle,
                emit_fn,
            )?;
            bbox.insert(&marker_bbox);
        }
    }

    Ok(bbox)
}

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parsing_invalid_marker_units_yields_error() {
        assert!(MarkerUnits::parse_str("").is_err());
        assert!(MarkerUnits::parse_str("foo").is_err());
    }

    #[test]
    fn parses_marker_units() {
        assert_eq!(
            MarkerUnits::parse_str("userSpaceOnUse"),
            Ok(MarkerUnits::UserSpaceOnUse)
        );
        assert_eq!(
            MarkerUnits::parse_str("strokeWidth"),
            Ok(MarkerUnits::StrokeWidth)
        );
    }

    #[test]
    fn parsing_invalid_marker_orient_yields_error() {
        assert!(MarkerOrient::parse_str("").is_err());
        assert!(MarkerOrient::parse_str("blah").is_err());
        assert!(MarkerOrient::parse_str("45blah").is_err());
    }

    #[test]
    fn parses_marker_orient() {
        assert_eq!(MarkerOrient::parse_str("auto"), Ok(MarkerOrient::Auto));

        assert_eq!(
            MarkerOrient::parse_str("0"),
            Ok(MarkerOrient::Angle(Angle::new(0.0)))
        );
        assert_eq!(
            MarkerOrient::parse_str("180"),
            Ok(MarkerOrient::Angle(Angle::from_degrees(180.0)))
        );
        assert_eq!(
            MarkerOrient::parse_str("180deg"),
            Ok(MarkerOrient::Angle(Angle::from_degrees(180.0)))
        );
        assert_eq!(
            MarkerOrient::parse_str("-400grad"),
            Ok(MarkerOrient::Angle(Angle::from_degrees(-360.0)))
        );
        assert_eq!(
            MarkerOrient::parse_str("1rad"),
            Ok(MarkerOrient::Angle(Angle::new(1.0)))
        );
    }
}

#[cfg(test)]
mod directionality_tests {
    use super::*;
    use crate::path_builder::PathBuilder;

    // Single open path; the easy case
    fn setup_open_path() -> Segments {
        let mut builder = PathBuilder::default();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);

        Segments::from(&builder.into_path())
    }

    #[test]
    fn path_to_segments_handles_open_path() {
        let expected_segments: Segments = Segments(vec![
            Segment::line(10.0, 10.0, 20.0, 10.0),
            Segment::line(20.0, 10.0, 20.0, 20.0),
        ]);

        assert_eq!(setup_open_path(), expected_segments);
    }

    fn setup_multiple_open_subpaths() -> Segments {
        let mut builder = PathBuilder::default();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);

        builder.move_to(30.0, 30.0);
        builder.line_to(40.0, 30.0);
        builder.curve_to(50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to(80.0, 90.0);

        Segments::from(&builder.into_path())
    }

    #[test]
    fn path_to_segments_handles_multiple_open_subpaths() {
        let expected_segments: Segments = Segments(vec![
            Segment::line(10.0, 10.0, 20.0, 10.0),
            Segment::line(20.0, 10.0, 20.0, 20.0),
            Segment::line(30.0, 30.0, 40.0, 30.0),
            Segment::curve(40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            Segment::line(70.0, 70.0, 80.0, 90.0),
        ]);

        assert_eq!(setup_multiple_open_subpaths(), expected_segments);
    }

    // Closed subpath; must have a line segment back to the first point
    fn setup_closed_subpath() -> Segments {
        let mut builder = PathBuilder::default();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        Segments::from(&builder.into_path())
    }

    #[test]
    fn path_to_segments_handles_closed_subpath() {
        let expected_segments: Segments = Segments(vec![
            Segment::line(10.0, 10.0, 20.0, 10.0),
            Segment::line(20.0, 10.0, 20.0, 20.0),
            Segment::line(20.0, 20.0, 10.0, 10.0),
        ]);

        assert_eq!(setup_closed_subpath(), expected_segments);
    }

    // Multiple closed subpaths; each must have a line segment back to their
    // initial points, with no degenerate segments between subpaths.
    fn setup_multiple_closed_subpaths() -> Segments {
        let mut builder = PathBuilder::default();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        builder.move_to(30.0, 30.0);
        builder.line_to(40.0, 30.0);
        builder.curve_to(50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to(80.0, 90.0);
        builder.close_path();

        Segments::from(&builder.into_path())
    }

    #[test]
    fn path_to_segments_handles_multiple_closed_subpaths() {
        let expected_segments: Segments = Segments(vec![
            Segment::line(10.0, 10.0, 20.0, 10.0),
            Segment::line(20.0, 10.0, 20.0, 20.0),
            Segment::line(20.0, 20.0, 10.0, 10.0),
            Segment::line(30.0, 30.0, 40.0, 30.0),
            Segment::curve(40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            Segment::line(70.0, 70.0, 80.0, 90.0),
            Segment::line(80.0, 90.0, 30.0, 30.0),
        ]);

        assert_eq!(setup_multiple_closed_subpaths(), expected_segments);
    }

    // A lineto follows the first closed subpath, with no moveto to start the second subpath.
    // The lineto must start at the first point of the first subpath.
    fn setup_no_moveto_after_closepath() -> Segments {
        let mut builder = PathBuilder::default();

        builder.move_to(10.0, 10.0);
        builder.line_to(20.0, 10.0);
        builder.line_to(20.0, 20.0);
        builder.close_path();

        builder.line_to(40.0, 30.0);

        Segments::from(&builder.into_path())
    }

    #[test]
    fn path_to_segments_handles_no_moveto_after_closepath() {
        let expected_segments: Segments = Segments(vec![
            Segment::line(10.0, 10.0, 20.0, 10.0),
            Segment::line(20.0, 10.0, 20.0, 20.0),
            Segment::line(20.0, 20.0, 10.0, 10.0),
            Segment::line(10.0, 10.0, 40.0, 30.0),
        ]);

        assert_eq!(setup_no_moveto_after_closepath(), expected_segments);
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
    // fn setup_sequence_of_moveto () -> Segments {
    // let mut builder = PathBuilder::default ();
    //
    // builder.move_to (10.0, 10.0);
    // builder.move_to (20.0, 20.0);
    // builder.move_to (30.0, 30.0);
    // builder.move_to (40.0, 40.0);
    //
    // Segments::from(&builder.into_path())
    // }
    //
    // #[test]
    // fn path_to_segments_handles_sequence_of_moveto () {
    // let expected_segments: Segments = Segments(vec! [
    // Segment::degenerate(10.0, 10.0),
    // Segment::degenerate(20.0, 20.0),
    // Segment::degenerate(30.0, 30.0),
    // Segment::degenerate(40.0, 40.0),
    // ]);
    //
    // assert_eq!(setup_sequence_of_moveto(), expected_segments);
    // }

    #[test]
    fn degenerate_segment_has_no_directionality() {
        let s = Segment::degenerate(1.0, 2.0);
        assert!(s.get_directionalities().is_none());
    }

    #[test]
    fn line_segment_has_directionality() {
        let s = Segment::line(1.0, 2.0, 3.0, 4.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((2.0, 2.0), (v2x, v2y));
    }

    #[test]
    fn line_segment_with_coincident_ends_has_no_directionality() {
        let s = Segment::line(1.0, 2.0, 1.0, 2.0);
        assert!(s.get_directionalities().is_none());
    }

    #[test]
    fn curve_has_directionality() {
        let s = Segment::curve(1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 20.0, 33.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((2.0, 3.0), (v1x, v1y));
        assert_eq!((12.0, 20.0), (v2x, v2y));
    }

    #[test]
    fn curves_with_loops_and_coincident_ends_have_directionality() {
        let s = Segment::curve(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 2.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-4.0, -4.0), (v2x, v2y));

        let s = Segment::curve(1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-2.0, -2.0), (v2x, v2y));

        let s = Segment::curve(1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 1.0, 2.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((2.0, 2.0), (v1x, v1y));
        assert_eq!((-2.0, -2.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_coincident_control_points_has_no_directionality() {
        let s = Segment::curve(1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0);
        assert!(s.get_directionalities().is_none());
    }

    #[test]
    fn curve_with_123_coincident_has_directionality() {
        let s = Segment::curve(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 20.0, 40.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((20.0, 40.0), (v1x, v1y));
        assert_eq!((20.0, 40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_234_coincident_has_directionality() {
        let s = Segment::curve(20.0, 40.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((-20.0, -40.0), (v1x, v1y));
        assert_eq!((-20.0, -40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_12_34_coincident_has_directionality() {
        let s = Segment::curve(20.0, 40.0, 20.0, 40.0, 60.0, 70.0, 60.0, 70.0);
        let (v1x, v1y, v2x, v2y) = s.get_directionalities().unwrap();
        assert_eq!((40.0, 30.0), (v1x, v1y));
        assert_eq!((40.0, 30.0), (v2x, v2y));
    }
}

#[cfg(test)]
mod marker_tests {
    use super::*;
    use crate::path_builder::PathBuilder;

    #[test]
    fn emits_for_open_subpath() {
        let mut builder = PathBuilder::default();
        builder.move_to(0.0, 0.0);
        builder.line_to(1.0, 0.0);
        builder.line_to(1.0, 1.0);
        builder.line_to(0.0, 1.0);

        let mut v = Vec::new();

        assert!(emit_markers_for_path(
            &builder.into_path(),
            BoundingBox::new(),
            &mut |marker_type: MarkerType,
                  x: f64,
                  y: f64,
                  computed_angle: Angle|
             -> Result<BoundingBox, RenderingError> {
                v.push((marker_type, x, y, computed_angle));
                Ok(BoundingBox::new())
            }
        )
        .is_ok());

        assert_eq!(
            v,
            vec![
                (MarkerType::Start, 0.0, 0.0, Angle::new(0.0)),
                (MarkerType::Middle, 1.0, 0.0, Angle::from_vector(1.0, 1.0)),
                (MarkerType::Middle, 1.0, 1.0, Angle::from_vector(-1.0, 1.0)),
                (MarkerType::End, 0.0, 1.0, Angle::from_vector(-1.0, 0.0)),
            ]
        );
    }

    #[test]
    fn emits_for_closed_subpath() {
        let mut builder = PathBuilder::default();
        builder.move_to(0.0, 0.0);
        builder.line_to(1.0, 0.0);
        builder.line_to(1.0, 1.0);
        builder.line_to(0.0, 1.0);
        builder.close_path();

        let mut v = Vec::new();

        assert!(emit_markers_for_path(
            &builder.into_path(),
            BoundingBox::new(),
            &mut |marker_type: MarkerType,
                  x: f64,
                  y: f64,
                  computed_angle: Angle|
             -> Result<BoundingBox, RenderingError> {
                v.push((marker_type, x, y, computed_angle));
                Ok(BoundingBox::new())
            }
        )
        .is_ok());

        assert_eq!(
            v,
            vec![
                (MarkerType::Start, 0.0, 0.0, Angle::new(0.0)),
                (MarkerType::Middle, 1.0, 0.0, Angle::from_vector(1.0, 1.0)),
                (MarkerType::Middle, 1.0, 1.0, Angle::from_vector(-1.0, 1.0)),
                (MarkerType::Middle, 0.0, 1.0, Angle::from_vector(-1.0, -1.0)),
                (MarkerType::End, 0.0, 0.0, Angle::from_vector(1.0, -1.0)),
            ]
        );
    }
}
