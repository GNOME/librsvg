extern crate libc;
extern crate cairo;
extern crate cairo_sys;

use std::f64;
use path_builder::*;
use drawing_ctx::RsvgDrawingCtx;

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

const EPSILON: f64 = 1e-10;

fn double_equals (a: f64, b: f64) -> bool {
    (a - b).abs () < EPSILON
}

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

    for cairo_segment in builder.get_path_segments () {
        last_x = cur_x;
        last_y = cur_y;

        match *cairo_segment {
            cairo::PathSegment::MoveTo ((x, y)) => {
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

            cairo::PathSegment::LineTo ((x, y)) => {
                cur_x = x;
                cur_y = y;

                segments.push (make_line (last_x, last_y, cur_x, cur_y));

                state = SegmentState::InSubpath;
            },

            cairo::PathSegment::CurveTo ((x2, y2), (x3, y3), (x4, y4)) => {
                cur_x = x4;
                cur_y = y4;

                segments.push (make_curve (last_x, last_y, x2, y2, x3, y3, cur_x, cur_y));

                state = SegmentState::InSubpath;
            },

            cairo::PathSegment::ClosePath => {
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

fn angle_from_vector (vx: f64, vy: f64) -> f64 {
    let mut angle = vy.atan2 (vx);

    if angle.is_nan () {
        0.0
    } else {
        while angle < 0.0 {
            angle += f64::consts::PI * 2.0;
        }

        angle
    }
}

extern "C" {
    fn rsvg_marker_render (marker_name: *const libc::c_char,
                           xpos: f64,
                           ypos: f64,
                           orient: f64,
                           linewidth: f64,
                           ctx: *mut RsvgDrawingCtx);
    fn rsvg_get_normalized_stroke_width (ctx: *const RsvgDrawingCtx) -> f64;

    fn rsvg_get_start_marker (ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
    fn rsvg_get_middle_marker (ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
    fn rsvg_get_end_marker (ctx: *const RsvgDrawingCtx) -> *const libc::c_char;
}

enum SubpathState {
    NoSubpath,
    InSubpath
}

fn render_marker_at_start_of_segment (segment: &Segment,
                                      marker_name: *const libc::c_char,
                                      orient: f64,
                                      linewidth: f64,
                                      ctx: *mut RsvgDrawingCtx) {
    let xpos: f64;
    let ypos: f64;

    match *segment {
        Segment::Degenerate { x, y } => {
            xpos = x;
            ypos = y;
        },

        Segment::LineOrCurve { x1, y1, .. } => {
            xpos = x1;
            ypos = y1;
        }
    }

    unsafe { rsvg_marker_render (marker_name, xpos, ypos, orient, linewidth, ctx); }
}

fn render_marker_at_end_of_segment (segment: &Segment,
                                    marker_name: *const libc::c_char,
                                    orient: f64,
                                    linewidth: f64,
                                    ctx: *mut RsvgDrawingCtx) {
    let xpos: f64;
    let ypos: f64;

    match *segment {
        Segment::Degenerate { x, y } => {
            xpos = x;
            ypos = y;
        },

        Segment::LineOrCurve { x4, y4, .. } => {
            xpos = x4;
            ypos = y4;
        }
    }

    unsafe { rsvg_marker_render (marker_name, xpos, ypos, orient, linewidth, ctx); }
}

#[no_mangle]
pub extern fn rsvg_render_markers (ctx: *mut RsvgDrawingCtx,
                                   raw_builder: *mut RsvgPathBuilder) {
    let linewidth: f64 = unsafe { rsvg_get_normalized_stroke_width (ctx) };

    if linewidth == 0.0 {
        return;
    }

    let start_marker = unsafe { rsvg_get_start_marker (ctx) };
    let middle_marker = unsafe { rsvg_get_middle_marker (ctx) };
    let end_marker = unsafe { rsvg_get_end_marker (ctx) };

    if start_marker.is_null () && middle_marker.is_null () && end_marker.is_null () {
        return;
    }

    assert! (!raw_builder.is_null ());
    let builder: &mut RsvgPathBuilder = unsafe { &mut (*raw_builder) };

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
                        render_marker_at_end_of_segment (&segments[i - 1], end_marker, angle_from_vector (incoming_vx, incoming_vy), linewidth, ctx);
                    },

                    _ => { }
                }

                /* Render marker for the lone point; no directionality */
                render_marker_at_start_of_segment (segment, middle_marker, 0.0, linewidth, ctx);

                subpath_state = SubpathState::NoSubpath;
            },

            Segment::LineOrCurve { .. } => {
                /* Not a degenerate segment */

                match subpath_state {
                    SubpathState::NoSubpath => {
                        let (_, outgoing_vx, outgoing_vy) = find_outgoing_directionality_forwards (&segments, i);
                        render_marker_at_start_of_segment (segment, start_marker, angle_from_vector (outgoing_vx, outgoing_vy), linewidth, ctx);

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
                            angle = (incoming + outgoing) / 2.0;
                        } else if has_incoming {
                            angle = incoming;
                        } else if has_outgoing {
                            angle = outgoing;
                        } else {
                            angle = 0.0;
                        }

                        render_marker_at_start_of_segment (segment, middle_marker, angle, linewidth, ctx);
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

                render_marker_at_end_of_segment (&segment, end_marker, angle_from_vector (incoming_vx, incoming_vy), linewidth, ctx);
            },

            _ => { }
        }
    }
}


/******************** Tests ********************/

#[cfg(test)]
mod tests {
    use super::*;
    use path_builder::*;
    extern crate cairo;

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
