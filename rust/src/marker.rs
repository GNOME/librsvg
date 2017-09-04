use ::cairo;
use ::glib::translate::*;
use ::libc;

use std::cell::Cell;
use std::f64::consts::*;
use std::str::FromStr;

use cairo::MatrixTrait;

use aspect_ratio::*;
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use path_builder::*;
use parsers;
use parsers::ParseError;
use property_bag;
use property_bag::*;
use util::*;
use viewbox::*;

// markerUnits attribute: https://www.w3.org/TR/SVG/painting.html#MarkerElement

#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerUnits {
    UserSpaceOnUse,
    StrokeWidth
}

impl Default for MarkerUnits {
    fn default () -> MarkerUnits {
        MarkerUnits::StrokeWidth
    }
}

impl FromStr for MarkerUnits {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result <MarkerUnits, AttributeError> {
        match s {
            "userSpaceOnUse" => Ok (MarkerUnits::UserSpaceOnUse),
            "strokeWidth"    => Ok (MarkerUnits::StrokeWidth),
            _                => Err (AttributeError::Parse (ParseError::new ("expected \"userSpaceOnUse\" or \"strokeWidth\"")))
        }
    }
}

// orient attribute: https://www.w3.org/TR/SVG/painting.html#MarkerElement

#[derive(Debug, Copy, Clone, PartialEq)]
enum MarkerOrient {
    Auto,
    Degrees (f64)
}

impl Default for MarkerOrient {
    fn default () -> MarkerOrient {
        MarkerOrient::Degrees (0.0)
    }
}

impl FromStr for MarkerOrient {
    type Err = AttributeError;

    fn from_str (s: &str) -> Result <MarkerOrient, AttributeError> {
        match s {
            "auto" => Ok (MarkerOrient::Auto),
            _      => parsers::angle_degrees (s)
                .map (|degrees| MarkerOrient::Degrees (degrees) )
                .map_err (|e| AttributeError::Parse (e))
        }
    }
}

// NodeMarker

struct NodeMarker {
    units:  Cell<MarkerUnits>,
    ref_x:  Cell<RsvgLength>,
    ref_y:  Cell<RsvgLength>,
    width:  Cell<RsvgLength>,
    height: Cell<RsvgLength>,
    orient: Cell<MarkerOrient>,
    aspect: Cell<AspectRatio>,
    vbox:   Cell<Option<ViewBox>>
}

impl NodeMarker {
    fn new () -> NodeMarker {
        NodeMarker {
            units:  Cell::new (MarkerUnits::default ()),
            ref_x:  Cell::new (RsvgLength::default ()),
            ref_y:  Cell::new (RsvgLength::default ()),
            width:  Cell::new (NodeMarker::get_default_size ()),
            height: Cell::new (NodeMarker::get_default_size ()),
            orient: Cell::new (MarkerOrient::default ()),
            aspect: Cell::new (AspectRatio::default ()),
            vbox:   Cell::new (None)
        }
    }

    fn get_default_size () -> RsvgLength {
        // per the spec
        RsvgLength::parse ("3", LengthDir::Both).unwrap ()
    }

    fn render (&self,
               node:           &RsvgNode,
               c_node:         *const RsvgNode,
               draw_ctx:       *const RsvgDrawingCtx,
               xpos:           f64,
               ypos:           f64,
               computed_angle: f64,
               line_width:     f64) {
        let marker_width = self.width.get ().normalize (draw_ctx);
        let marker_height = self.height.get ().normalize (draw_ctx);

        let mut affine = cairo::Matrix::identity ();
        affine.translate (xpos, ypos);

        affine = cairo::Matrix::multiply (&affine, &drawing_ctx::get_current_state_affine (draw_ctx));

        let rotation: f64;

        match self.orient.get () {
            MarkerOrient::Auto =>        { rotation = computed_angle; },
            MarkerOrient::Degrees (d) => { rotation = d * PI / 180.0; }
        }

        affine.rotate (rotation);

        if self.units.get () == MarkerUnits::StrokeWidth {
            affine.scale (line_width, line_width);
        }

        if let Some (vbox) = self.vbox.get () {
            let (_, _, w, h) = self.aspect.get ().compute (vbox.0.width, vbox.0.height,
                                                           0.0, 0.0,
                                                           marker_width, marker_height);

            affine.scale (w / vbox.0.width, h / vbox.0.height);

            drawing_ctx::push_view_box (draw_ctx, vbox.0.width, vbox.0.height);
        } else {
            drawing_ctx::push_view_box (draw_ctx, marker_width, marker_height);
        }

        affine.translate (-self.ref_x.get ().normalize (draw_ctx),
                          -self.ref_y.get ().normalize (draw_ctx));

        drawing_ctx::state_push (draw_ctx);

        let state = drawing_ctx::get_current_state (draw_ctx);
        drawing_ctx::state_reinit (state);
        drawing_ctx::state_reconstruct (state, c_node);

        drawing_ctx::set_current_state_affine (draw_ctx, affine);

        drawing_ctx::push_discrete_layer (draw_ctx);

        let state = drawing_ctx::get_current_state (draw_ctx);

        if !drawing_ctx::state_is_overflow (state) {
            if let Some (vbox) = self.vbox.get () {
                drawing_ctx::add_clipping_rect (draw_ctx,
                                                vbox.0.x,
                                                vbox.0.y,
                                                vbox.0.width,
                                                vbox.0.height);
            } else {
                drawing_ctx::add_clipping_rect (draw_ctx,
                                                0.0,
                                                0.0,
                                                marker_width,
                                                marker_height);
            }
        }

        node.draw_children (draw_ctx, -1); // dominate=-1 so it won't reinherit state / push a layer

        drawing_ctx::state_pop (draw_ctx);
        drawing_ctx::pop_discrete_layer (draw_ctx);
        drawing_ctx::pop_view_box (draw_ctx);
    }
}

impl NodeTrait for NodeMarker {
    fn set_atts (&self, _: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        self.units.set (property_bag::parse_or_default (pbag, "markerUnits")?);

        self.ref_x.set (property_bag::length_or_default (pbag, "refX", LengthDir::Horizontal)?);
        self.ref_y.set (property_bag::length_or_default (pbag, "refY", LengthDir::Vertical)?);

        self.width.set (property_bag::lookup (pbag, "markerWidth").map_or (NodeMarker::get_default_size (),
                                                                           |v| RsvgLength::parse (&v, LengthDir::Horizontal).unwrap_or (NodeMarker::get_default_size ())));
        self.height.set (property_bag::lookup (pbag, "markerHeight").map_or (NodeMarker::get_default_size (),
                                                                             |v| RsvgLength::parse (&v, LengthDir::Vertical).unwrap_or (NodeMarker::get_default_size ())));

        self.orient.set (property_bag::parse_or_default (pbag, "orient")?);
        self.aspect.set (property_bag::parse_or_default (pbag, "preserveAspectRatio")?);
        self.vbox.set   (property_bag::parse_or_none (pbag, "viewBox")?);
        self.aspect.set (property_bag::parse_or_default (pbag, "preserveAspectRatio")?);

        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; markers are drawn by their referencing shapes
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

#[no_mangle]
pub extern fn rsvg_node_marker_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Marker,
                    raw_parent,
                    Box::new (NodeMarker::new ()))
}

// Machinery to figure out marker orientations

#[derive(Debug, PartialEq)]
pub enum Segment {
    Degenerate {            // A single lone point
        x: f64,
        y: f64
    },

    LineOrCurve {
        x1: f64, y1: f64,
        x2: f64, y2: f64,
        x3: f64, y3: f64,
        x4: f64, y4: f64
    },
}

enum SegmentState {
    Initial,
    NewSubpath,
    InSubpath,
    ClosedSubpath
}

/* This converts a cairo_path_t into a list of curveto-like segments.  Each segment can be:
 *
 * 1. Segment::Degenerate => the segment is actually a single point (x, y)
 *
 * 2. Segment::LineOrCurve => either a lineto or a curveto (or the effective lineto that results from a closepath).
 *    We have the following points:
 *       P1 = (x1, y1)
 *       P2 = (x2, y2)
 *       P3 = (x3, y3)
 *       P4 = (x4, y4)
 *
 *    The start and end points are P1 and P4, respectively.
 *    The tangent at the start point is given by the vector (P2 - P1).
 *    The tangent at the end point is given by the vector (P4 - P3).
 *    The tangents also work if the segment refers to a lineto (they will both just point in the same direction).
 */

fn make_degenerate (x: f64, y: f64) -> Segment {
    Segment::Degenerate { x: x, y: y }
}

fn make_curve (x1: f64, y1: f64,
               x2: f64, y2: f64,
               x3: f64, y3: f64,
               x4: f64, y4: f64) -> Segment {
    Segment::LineOrCurve {
        x1: x1, y1: y1,
        x2: x2, y2: y2,
        x3: x3, y3: y3,
        x4: x4, y4: y4
    }
}

fn make_line (x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
    make_curve (x1, y1, x2, y2, x1, y1, x2, y2)
}


pub fn path_builder_to_segments (builder: &RsvgPathBuilder) -> Vec<Segment> {
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

    segments = Vec::new ();
    state = SegmentState::Initial;

    for path_command in builder.get_path_commands () {
        last_x = cur_x;
        last_y = cur_y;

        match *path_command {
            PathCommand::MoveTo (x, y) => {
                cur_x = x;
                cur_y = y;

                subpath_start_x = cur_x;
                subpath_start_y = cur_y;

                match state {
                    SegmentState::Initial |
                    SegmentState::InSubpath => {
                        /* Ignore the very first moveto in a sequence (Initial state), or if we were
                         * already drawing within a subpath, start a new subpath.
                         */
                        state = SegmentState::NewSubpath;
                    },

                    SegmentState::NewSubpath => {
                        /* We had just begun a new subpath (i.e. from a moveto) and we got another
                         * moveto?  Output a stray point for the previous moveto.
                         */
                        segments.push (make_degenerate (last_x, last_y));
                        state = SegmentState::NewSubpath;
                    }

                    SegmentState::ClosedSubpath => {
                        /* Cairo outputs a moveto after every closepath, so that subsequent
                         * lineto/curveto commands will start at the closed vertex.
                         *
                         * We don't want to actually emit a point (a degenerate segment) in that
                         * artificial-moveto case.
                         *
                         * We'll reset to the Initial state so that a subsequent "real" moveto will
                         * be handled as the beginning of a new subpath, or a degenerate point, as
                         * usual.
                         */
                        state = SegmentState::Initial;
                    }
                }
            },

            PathCommand::LineTo (x, y) => {
                cur_x = x;
                cur_y = y;

                segments.push (make_line (last_x, last_y, cur_x, cur_y));

                state = SegmentState::InSubpath;
            },

            PathCommand::CurveTo ((x2, y2), (x3, y3), (x4, y4)) => {
                cur_x = x4;
                cur_y = y4;

                segments.push (make_curve (last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

                state = SegmentState::InSubpath;
            },

            PathCommand::ClosePath => {
                cur_x = subpath_start_x;
                cur_y = subpath_start_y;

                segments.push (make_line (last_x, last_y, cur_x, cur_y));

                state = SegmentState::ClosedSubpath;
            }
        }
    }

    match state {
        SegmentState::NewSubpath => {
            /* Output a lone point if we started a subpath with a
             * moveto command, but there are no subsequent commands.
             */
            segments.push (make_degenerate (cur_x, cur_y));
        },

        _ => {
        }
    }

    segments
}

fn points_equal (x1: f64, y1: f64, x2: f64, y2: f64) -> bool {
    double_equals (x1, x2) && double_equals (y1, y2)
}

/* If the segment has directionality, returns two vectors (v1x, v1y, v2x, v2y); otherwise, returns None.  The
 * vectors are the tangents at the beginning and at the end of the segment, respectively.  A segment does not have
 * directionality if it is degenerate (i.e. a single point) or a zero-length segment, i.e. where all four control
 * points are coincident (the first and last control points may coincide, but the others may define a loop - thus
 * nonzero length)
 */
fn get_segment_directionalities (segment: &Segment) -> Option <(f64, f64, f64, f64)> {
    match *segment {
        Segment::Degenerate { .. } => { None },

        Segment::LineOrCurve { x1, y1, x2, y2, x3, y3, x4, y4 } => {
            let coincide_1_and_2 = points_equal (x1, y1, x2, y2);
            let coincide_1_and_3 = points_equal (x1, y1, x3, y3);
            let coincide_1_and_4 = points_equal (x1, y1, x4, y4);
            let coincide_2_and_3 = points_equal (x2, y2, x3, y3);
            let coincide_2_and_4 = points_equal (x2, y2, x4, y4);
            let coincide_3_and_4 = points_equal (x3, y3, x4, y4);

            if coincide_1_and_2 && coincide_1_and_3 && coincide_1_and_4 {

                None

            } else if coincide_1_and_2 && coincide_1_and_3 {

                Some ((x4 - x1, y4 - y1, x4 - x3, y4 - y3))

            } else if coincide_1_and_2 && coincide_3_and_4 {

                Some ((x4 - x1, y4 - y1, x4 - x1, y4 - y1))

            } else if coincide_2_and_3 && coincide_2_and_4 {

                Some ((x2 - x1, y2 - y1, x4 - x1, y4 - y1))

            } else if coincide_1_and_2 {

                Some  ((x3 - x1, y3 - y1, x4 - x3, y4 - y3))

            } else if coincide_3_and_4 {

                Some ((x2 - x1, y2 - y1, x4 - x2, y4 - y2))

            } else {

                Some ((x2 - x1, y2 - y1, x4 - x3, y4 - y3))

            }
        }
    }
}

/* The SVG spec 1.1 says http://www.w3.org/TR/SVG/implnote.html#PathElementImplementationNotes
 *
 * Certain line-capping and line-joining situations and markers
 * require that a path segment have directionality at its start and
 * end points. Zero-length path segments have no directionality. In
 * these cases, the following algorithm is used to establish
 * directionality:  to determine the directionality of the start
 * point of a zero-length path segment, go backwards in the path
 * data specification within the current subpath until you find a
 * segment which has directionality at its end point (e.g., a path
 * segment with non-zero length) and use its ending direction;
 * otherwise, temporarily consider the start point to lack
 * directionality. Similarly, to determine the directionality of the
 * end point of a zero-length path segment, go forwards in the path
 * data specification within the current subpath until you find a
 * segment which has directionality at its start point (e.g., a path
 * segment with non-zero length) and use its starting direction;
 * otherwise, temporarily consider the end point to lack
 * directionality. If the start point has directionality but the end
 * point doesn't, then the end point uses the start point's
 * directionality. If the end point has directionality but the start
 * point doesn't, then the start point uses the end point's
 * directionality. Otherwise, set the directionality for the path
 * segment's start and end points to align with the positive x-axis
 * in user space.
 */
fn find_incoming_directionality_backwards (segments: &[Segment], start_index: usize) -> (bool, f64, f64) {
    /* "go backwards ... within the current subpath until ... segment which has directionality at its end point" */

    for segment in segments[.. start_index + 1].iter ().rev () {
        match *segment {
            Segment::Degenerate { .. } => {
                return (false, 0.0, 0.0); /* reached the beginning of the subpath as we ran into a standalone point */
            },

            Segment::LineOrCurve { .. } => {
                match get_segment_directionalities (segment) {
                    Some ((_, _, v2x, v2y)) => { return (true, v2x, v2y); }
                    None => { continue; }
                }
            }
        }
    }

    (false, 0.0, 0.0)
}

fn find_outgoing_directionality_forwards (segments: &[Segment], start_index: usize) -> (bool, f64, f64) {
    /* "go forwards ... within the current subpath until ... segment which has directionality at its start point" */

    for segment in &segments[start_index .. ] {
        match *segment {
            Segment::Degenerate { .. } => {
                return (false, 0.0, 0.0);  /* reached the end of a subpath as we ran into a standalone point */
            },

            Segment::LineOrCurve { .. } => {
                match get_segment_directionalities (segment) {
                    Some ((v1x, v1y, _, _)) => { return (true, v1x, v1y); }
                    None => { continue; }
                }
            }
        }
    }

    (false, 0.0, 0.0)
}

// Normalizes an angle to [0.0, 2*PI)
fn normalize_angle (mut angle: f64) -> f64 {
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

fn angle_from_vector (vx: f64, vy: f64) -> f64 {
    let angle = vy.atan2 (vx);

    if angle.is_nan () {
        0.0
    } else {
        normalize_angle (angle)
    }
}

fn bisect_angles (incoming: f64, outgoing: f64) -> f64 {
    let half_delta: f64;

    half_delta = (outgoing - incoming) * 0.5;

    if FRAC_PI_2 < half_delta.abs () {
        normalize_angle (incoming + half_delta - PI)
    } else {
        normalize_angle (incoming + half_delta)
    }
}

// From SVG's marker-start, marker-mid, marker-end properties
#[derive(Debug, PartialEq)]
enum MarkerType {
    Start,
    Middle,
    End
}

fn emit_marker_by_name (draw_ctx:       *const RsvgDrawingCtx,
                        marker_name:    *const libc::c_char,
                        xpos:           f64,
                        ypos:           f64,
                        computed_angle: f64,
                        line_width:     f64) {
    if marker_name.is_null () {
        return;
    }

    let name = unsafe { String::from_glib_none (marker_name) };

    let c_node = drawing_ctx::acquire_node_of_type (draw_ctx, &name, NodeType::Marker);

    if c_node.is_null () {
        return;
    }

    let node: &RsvgNode = unsafe { & *c_node };

    node.with_impl (|marker: &NodeMarker| marker.render (node, c_node, draw_ctx, xpos, ypos, computed_angle, line_width));

    drawing_ctx::release_node (draw_ctx, c_node);
}

fn get_marker_name_from_drawing_ctx (draw_ctx:    *const RsvgDrawingCtx,
                                     marker_type: MarkerType) -> *const libc::c_char {
   match marker_type {
        MarkerType::Start  => unsafe { rsvg_get_start_marker (draw_ctx) },
        MarkerType::Middle => unsafe { rsvg_get_middle_marker (draw_ctx) },
        MarkerType::End    => unsafe { rsvg_get_end_marker (draw_ctx) }
    }
}

enum MarkerEndpoint {
    Start,
    End
}

fn emit_marker<E> (segment:     &Segment,
                   endpoint:    MarkerEndpoint,
                   marker_type: MarkerType,
                   orient:      f64,
                   emit_fn:     &mut E) where E: FnMut(MarkerType, f64, f64, f64) {
    let (x, y) = match *segment {
        Segment::Degenerate  { x, y } => (x, y),

        Segment::LineOrCurve { x1, y1, x4, y4, .. } => match endpoint {
            MarkerEndpoint::Start => (x1, y1),
            MarkerEndpoint::End   => (x4, y4)
        }
    };

    emit_fn (marker_type, x, y, orient);
}

extern "C" {
    fn rsvg_get_normalized_stroke_width (draw_ctx: *const RsvgDrawingCtx) -> f64;

    fn rsvg_get_start_marker (draw_ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
    fn rsvg_get_middle_marker (draw_ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
    fn rsvg_get_end_marker (draw_ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
}

fn drawing_ctx_has_markers (draw_ctx: *const RsvgDrawingCtx) -> bool {
    (!get_marker_name_from_drawing_ctx (draw_ctx, MarkerType::Start).is_null ()
     || !get_marker_name_from_drawing_ctx (draw_ctx, MarkerType::Middle).is_null ()
     || !get_marker_name_from_drawing_ctx (draw_ctx, MarkerType::End).is_null ())
}

pub fn render_markers_for_path_builder (builder:  &RsvgPathBuilder,
                                        draw_ctx: *const RsvgDrawingCtx) {
    let linewidth: f64 = unsafe { rsvg_get_normalized_stroke_width (draw_ctx) };

    if linewidth == 0.0 {
        return;
    }

    if !drawing_ctx_has_markers (draw_ctx) {
        return;
    }

    emit_markers_for_path_builder (builder,
                                   &mut |marker_type: MarkerType, x: f64, y: f64, computed_angle: f64| {
                                       emit_marker_by_name (draw_ctx,
                                                            get_marker_name_from_drawing_ctx (draw_ctx, marker_type),
                                                            x,
                                                            y,
                                                            computed_angle,
                                                            linewidth);
                                   });
}

fn emit_markers_for_path_builder<E> (builder: &RsvgPathBuilder,
                                     emit_fn: &mut E) where E: FnMut(MarkerType, f64, f64, f64) {
    enum SubpathState {
        NoSubpath,
        InSubpath
    };

    /* Convert the path to a list of segments and bare points */
    let segments = path_builder_to_segments (builder);

    let mut subpath_state = SubpathState::NoSubpath;

    for (i, segment) in segments.iter ().enumerate () {
        match *segment {
            Segment::Degenerate { .. } => {
                match subpath_state {
                    SubpathState::InSubpath => {
                        assert! (i > 0);

                        /* Got a lone point after a subpath; render the subpath's end marker first */

                        let (_, incoming_vx, incoming_vy) = find_incoming_directionality_backwards (&segments, i - 1);
                        emit_marker (&segments[i - 1],
                                     MarkerEndpoint::End,
                                     MarkerType::End,
                                     angle_from_vector (incoming_vx, incoming_vy),
                                     emit_fn);
                    },

                    _ => { }
                }

                /* Render marker for the lone point; no directionality */
                emit_marker (segment, MarkerEndpoint::Start, MarkerType::Middle, 0.0, emit_fn);

                subpath_state = SubpathState::NoSubpath;
            },

            Segment::LineOrCurve { .. } => {
                /* Not a degenerate segment */

                match subpath_state {
                    SubpathState::NoSubpath => {
                        let (_, outgoing_vx, outgoing_vy) = find_outgoing_directionality_forwards (&segments, i);
                        emit_marker (segment,
                                     MarkerEndpoint::Start,
                                     MarkerType::Start,
                                     angle_from_vector (outgoing_vx, outgoing_vy),
                                     emit_fn);

                        subpath_state = SubpathState::InSubpath;
                    },

                    SubpathState::InSubpath => {
                        assert! (i > 0);

                        let (has_incoming, incoming_vx, incoming_vy) = find_incoming_directionality_backwards (&segments, i - 1);
                        let (has_outgoing, outgoing_vx, outgoing_vy) = find_outgoing_directionality_forwards (&segments, i);

                        let incoming: f64;
                        let outgoing: f64;

                        incoming = angle_from_vector (incoming_vx, incoming_vy);
                        outgoing = angle_from_vector (outgoing_vx, outgoing_vy);

                        let angle: f64;

                        if has_incoming && has_outgoing {
                            angle = bisect_angles (incoming, outgoing);
                        } else if has_incoming {
                            angle = incoming;
                        } else if has_outgoing {
                            angle = outgoing;
                        } else {
                            angle = 0.0;
                        }

                        emit_marker (segment, MarkerEndpoint::Start, MarkerType::Middle, angle, emit_fn);
                    }
                }
            }
        }
    }

    /* Finally, render the last point */

    if segments.len() > 0 {
        let segment = &segments[segments.len() - 1];
        match *segment {
            Segment::LineOrCurve { .. } => {
                let (_, incoming_vx, incoming_vy) = find_incoming_directionality_backwards (&segments, segments.len () - 1);

                emit_marker (&segment, MarkerEndpoint::End, MarkerType::End, angle_from_vector (incoming_vx, incoming_vy), emit_fn);
            },

            _ => { }
        }
    }
}

/******************** Tests ********************/

#[cfg(test)]
mod parser_tests {
    use super::*;

    #[test]
    fn parsing_invalid_marker_units_yields_error () {
        assert! (is_parse_error (&MarkerUnits::from_str ("").map_err (|e| AttributeError::from (e))));
        assert! (is_parse_error (&MarkerUnits::from_str ("foo").map_err (|e| AttributeError::from (e))));
    }

    #[test]
    fn parses_marker_units () {
        assert_eq! (MarkerUnits::from_str ("userSpaceOnUse"), Ok (MarkerUnits::UserSpaceOnUse));
        assert_eq! (MarkerUnits::from_str ("strokeWidth"),    Ok (MarkerUnits::StrokeWidth));
    }

    #[test]
    fn parsing_invalid_marker_orient_yields_error () {
        assert! (is_parse_error (&MarkerOrient::from_str ("").map_err (|e| AttributeError::from (e))));
        assert! (is_parse_error (&MarkerOrient::from_str ("blah").map_err (|e| AttributeError::from (e))));
        assert! (is_parse_error (&MarkerOrient::from_str ("45blah").map_err (|e| AttributeError::from (e))));
    }

    #[test]
    fn parses_marker_orient () {
        assert_eq! (MarkerOrient::from_str ("auto"), Ok (MarkerOrient::Auto));

        assert_eq! (MarkerOrient::from_str ("0"), Ok (MarkerOrient::Degrees (0.0)));
        assert_eq! (MarkerOrient::from_str ("180"), Ok (MarkerOrient::Degrees (180.0)));
        assert_eq! (MarkerOrient::from_str ("180deg"), Ok (MarkerOrient::Degrees (180.0)));
        assert_eq! (MarkerOrient::from_str ("-400grad"), Ok (MarkerOrient::Degrees (-360.0)));
        assert_eq! (MarkerOrient::from_str ("1rad"), Ok (MarkerOrient::Degrees (180.0 / PI)));
    }
}

#[cfg(test)]
mod directionality_tests {
    use std::f64::consts::*;
    use super::*;
    extern crate cairo;

    fn test_bisection_angle (expected: f64,
                             incoming_vx: f64,
                             incoming_vy: f64,
                             outgoing_vx: f64,
                             outgoing_vy: f64) {
        let bisected = super::bisect_angles (super::angle_from_vector (incoming_vx, incoming_vy),
                                             super::angle_from_vector (outgoing_vx, outgoing_vy));
        assert! (double_equals (expected, bisected));
    }

    #[test]
    fn bisection_angle_is_correct_from_incoming_counterclockwise_to_outgoing () {
        // 1st quadrant
        test_bisection_angle (FRAC_PI_4,
                              1.0, 0.0,
                              0.0, 1.0);

        // 2nd quadrant
        test_bisection_angle (FRAC_PI_2 + FRAC_PI_4,
                              0.0, 1.0,
                              -1.0, 0.0);

        // 3rd quadrant
        test_bisection_angle (PI + FRAC_PI_4,
                              -1.0, 0.0,
                              0.0, -1.0);

        // 4th quadrant
        test_bisection_angle (PI + FRAC_PI_2 + FRAC_PI_4,
                              0.0, -1.0,
                              1.0, 0.0);
    }

    #[test]
    fn bisection_angle_is_correct_from_incoming_clockwise_to_outgoing () {
        // 1st quadrant
        test_bisection_angle (FRAC_PI_4,
                              0.0, 1.0,
                              1.0, 0.0);

        // 2nd quadrant
        test_bisection_angle (FRAC_PI_2 + FRAC_PI_4,
                              -1.0, 0.0,
                              0.0, 1.0);

        // 3rd quadrant
        test_bisection_angle (PI + FRAC_PI_4,
                              0.0, -1.0,
                              -1.0, 0.0);

        // 4th quadrant
        test_bisection_angle (PI + FRAC_PI_2 + FRAC_PI_4,
                              1.0, 0.0,
                              0.0, -1.0);
    }

    #[test]
    fn bisection_angle_is_correct_for_more_than_quarter_turn_angle () {
        test_bisection_angle (0.0,
                              0.1, -1.0,
                              0.1, 1.0);

        test_bisection_angle (FRAC_PI_2,
                              1.0, 0.1,
                              -1.0, 0.1);

        test_bisection_angle (PI,
                              -0.1, 1.0,
                              -0.1, -1.0);

        test_bisection_angle (PI + FRAC_PI_2,
                              -1.0, -0.1,
                              1.0, -0.1);
    }

    fn degenerate (x: f64, y: f64) -> Segment {
        super::make_degenerate (x, y)
    }

    fn line (x1: f64, y1: f64, x2: f64, y2: f64) -> Segment {
        super::make_line (x1, y1, x2, y2)
    }

    fn curve (x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64, x4: f64, y4: f64) -> Segment {
        super::make_curve (x1, y1, x2, y2, x3, y3, x4, y4)
    }

    fn test_path_builder_to_segments (builder: &RsvgPathBuilder, expected_segments: Vec<Segment>) {
        let segments = path_builder_to_segments (builder);
        assert_eq! (expected_segments, segments);
    }

    /* Single open path; the easy case */

    fn setup_open_path () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.line_to (20.0, 10.0);
        builder.line_to (20.0, 20.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_open_path () {
        let expected_segments: Vec<Segment> = vec![
            line (10.0, 10.0, 20.0, 10.0),
            line (20.0, 10.0, 20.0, 20.0)
        ];

        test_path_builder_to_segments (&setup_open_path (), expected_segments);
    }

    /* Multiple open subpaths */

    fn setup_multiple_open_subpaths () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.line_to (20.0, 10.0);
        builder.line_to (20.0, 20.0);

        builder.move_to (30.0, 30.0);
        builder.line_to (40.0, 30.0);
        builder.curve_to (50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to (80.0, 90.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_multiple_open_subpaths () {
        let expected_segments: Vec<Segment> = vec![
            line  (10.0, 10.0, 20.0, 10.0),
            line  (20.0, 10.0, 20.0, 20.0),

            line  (30.0, 30.0, 40.0, 30.0),
            curve (40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            line  (70.0, 70.0, 80.0, 90.0)
        ];

        test_path_builder_to_segments (&setup_multiple_open_subpaths (), expected_segments);
    }

    /* Closed subpath; must have a line segment back to the first point */

    fn setup_closed_subpath () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.line_to (20.0, 10.0);
        builder.line_to (20.0, 20.0);
        builder.close_path ();

        builder
    }

    #[test]
    fn path_to_segments_handles_closed_subpath () {
        let expected_segments: Vec<Segment> = vec![
            line (10.0, 10.0, 20.0, 10.0),
            line (20.0, 10.0, 20.0, 20.0),
            line (20.0, 20.0, 10.0, 10.0)
        ];

        test_path_builder_to_segments (&setup_closed_subpath (), expected_segments);
    }

    /* Multiple closed subpaths; each must have a line segment back to their
     * initial points, with no degenerate segments between subpaths.
     */

    fn setup_multiple_closed_subpaths () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.line_to (20.0, 10.0);
        builder.line_to (20.0, 20.0);
        builder.close_path ();

        builder.move_to (30.0, 30.0);
        builder.line_to (40.0, 30.0);
        builder.curve_to (50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        builder.line_to (80.0, 90.0);
        builder.close_path ();

        builder
    }

    #[test]
    fn path_to_segments_handles_multiple_closed_subpaths () {
        let expected_segments: Vec<Segment> = vec![
            line  (10.0, 10.0, 20.0, 10.0),
            line  (20.0, 10.0, 20.0, 20.0),
            line  (20.0, 20.0, 10.0, 10.0),

            line  (30.0, 30.0, 40.0, 30.0),
            curve (40.0, 30.0, 50.0, 35.0, 60.0, 60.0, 70.0, 70.0),
            line  (70.0, 70.0, 80.0, 90.0),
            line  (80.0, 90.0, 30.0, 30.0)
        ];

        test_path_builder_to_segments (&setup_multiple_closed_subpaths (), expected_segments);
    }

    /* A lineto follows the first closed subpath, with no moveto to start the second subpath.  The
     * lineto must start at the first point of the first subpath.
     */

    fn setup_no_moveto_after_closepath () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.line_to (20.0, 10.0);
        builder.line_to (20.0, 20.0);
        builder.close_path ();

        builder.line_to (40.0, 30.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_no_moveto_after_closepath () {
        let expected_segments: Vec<Segment> = vec![
            line  (10.0, 10.0, 20.0, 10.0),
            line  (20.0, 10.0, 20.0, 20.0),
            line  (20.0, 20.0, 10.0, 10.0),

            line  (10.0, 10.0, 40.0, 30.0)
        ];

        test_path_builder_to_segments (&setup_no_moveto_after_closepath (), expected_segments);
    }

    /* Sequence of moveto; should generate degenerate points.
     *
     * This test is not enabled right now!  We create the
     * path fixtures with Cairo, and Cairo compresses
     * sequences of moveto into a single one.  So, we can't
     * really test this, as we don't get the fixture we want.
     *
     * Eventually we'll probably have to switch librsvg to
     * its own internal path representation which should
     * allow for unelided path commands, and which should
     * only build a cairo_path_t for the final rendering step.

    fn setup_sequence_of_moveto () -> RsvgPathBuilder {
        let mut builder = RsvgPathBuilder::new ();

        builder.move_to (10.0, 10.0);
        builder.move_to (20.0, 20.0);
        builder.move_to (30.0, 30.0);
        builder.move_to (40.0, 40.0);

        builder
    }

    #[test]
    fn path_to_segments_handles_sequence_of_moveto () {
        let expected_segments: Vec<Segment> = vec! [
            degenerate (10.0, 10.0),
            degenerate (20.0, 20.0),
            degenerate (30.0, 30.0),
            degenerate (40.0, 40.0)
        ];

        test_path_builder_to_segments (&setup_sequence_of_moveto (), expected_segments);
    }
     */

    #[test]
    fn degenerate_segment_has_no_directionality () {
        assert! (super::get_segment_directionalities (&degenerate (1.0, 2.0)).is_none ());
    }

    #[test]
    fn line_segment_has_directionality () {
        let (v1x, v1y, v2x, v2y) = super::get_segment_directionalities (&line (1.0, 2.0, 3.0, 4.0)).unwrap ();
        assert_eq! ((2.0, 2.0), (v1x, v1y));
        assert_eq! ((2.0, 2.0), (v2x, v2y));
    }

    #[test]
    fn line_segment_with_coincident_ends_has_no_directionality () {
        assert! (super::get_segment_directionalities (&line (1.0, 2.0, 1.0, 2.0)).is_none ());
    }

    #[test]
    fn curve_has_directionality () {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (1.0, 2.0, 3.0, 5.0, 8.0, 13.0, 20.0, 33.0)).unwrap ();
        assert_eq! ((2.0, 3.0), (v1x, v1y));
        assert_eq! ((12.0, 20.0), (v2x, v2y));
    }

    #[test]
    fn curves_with_loops_and_coincident_ends_have_directionality () {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 2.0)).unwrap ();
        assert_eq! ((2.0, 2.0), (v1x, v1y));
        assert_eq! ((-4.0, -4.0), (v2x, v2y));

        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0)).unwrap ();
        assert_eq! ((2.0, 2.0), (v1x, v1y));
        assert_eq! ((-2.0, -2.0), (v2x, v2y));

        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 1.0, 2.0)).unwrap ();
        assert_eq! ((2.0, 2.0), (v1x, v1y));
        assert_eq! ((-2.0, -2.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_coincident_control_points_has_no_directionality () {
        assert! (super::get_segment_directionalities (&curve (1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0)).is_none ());
    }

    #[test]
    fn curve_with_123_coincident_has_directionality () {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 20.0, 40.0)).unwrap ();
        assert_eq! ((20.0, 40.0), (v1x, v1y));
        assert_eq! ((20.0, 40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_234_coincident_has_directionality () {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (20.0, 40.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0)).unwrap ();

        assert_eq! ((-20.0, -40.0), (v1x, v1y));
        assert_eq! ((-20.0, -40.0), (v2x, v2y));
    }

    #[test]
    fn curve_with_12_34_coincident_has_directionality () {
        let (v1x, v1y, v2x, v2y) =
            super::get_segment_directionalities (&curve (20.0, 40.0, 20.0, 40.0, 60.0, 70.0, 60.0, 70.0)).unwrap ();

        assert_eq! ((40.0, 30.0), (v1x, v1y));
        assert_eq! ((40.0, 30.0), (v2x, v2y));
    }
}

#[cfg(test)]
mod marker_tests {
    use super::*;

    #[test]
    fn emits_for_open_subpath () {
        let mut builder = RsvgPathBuilder::new ();
        builder.move_to (0.0, 0.0);
        builder.line_to (1.0, 0.0);
        builder.line_to (1.0, 1.0);
        builder.line_to (0.0, 1.0);

        let mut v = Vec::new ();

        emit_markers_for_path_builder (&builder,
                                       &mut |marker_type: MarkerType, x: f64, y: f64, computed_angle: f64| {
                                           v.push ((marker_type, x, y, computed_angle));
                                       });

        assert_eq! (v, vec! [(MarkerType::Start,  0.0, 0.0, 0.0),
                             (MarkerType::Middle, 1.0, 0.0, angle_from_vector (1.0, 1.0)),
                             (MarkerType::Middle, 1.0, 1.0, angle_from_vector (-1.0, 1.0)),
                             (MarkerType::End,    0.0, 1.0, angle_from_vector (-1.0, 0.0))]);
    }

    #[test]
    #[ignore]
    // https://bugzilla.gnome.org/show_bug.cgi?id=777854
    fn emits_for_closed_subpath () {
        let mut builder = RsvgPathBuilder::new ();
        builder.move_to (0.0, 0.0);
        builder.line_to (1.0, 0.0);
        builder.line_to (1.0, 1.0);
        builder.line_to (0.0, 1.0);
        builder.close_path ();

        let mut v = Vec::new ();

        emit_markers_for_path_builder (&builder,
                                       &mut |marker_type: MarkerType, x: f64, y: f64, computed_angle: f64| {
                                           v.push ((marker_type, x, y, computed_angle));
                                       });

        assert_eq! (v, vec! [(MarkerType::Start,  0.0, 0.0, 0.0),
                             (MarkerType::Middle, 1.0, 0.0, angle_from_vector (1.0, 1.0)),
                             (MarkerType::Middle, 1.0, 1.0, angle_from_vector (-1.0, 1.0)),
                             (MarkerType::Middle, 0.0, 1.0, angle_from_vector (-1.0, 0.0)),
                             (MarkerType::End,    0.0, 0.0, angle_from_vector (1.0, -1.0))]);
    }
}
