extern crate cairo;

#[derive(Debug)]
#[derive(PartialEq)]
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
    Segment::Degenerate { x: x, y: y}
}

fn make_curve (x1: f64, y1: f64,
               mut x2: f64, mut y2: f64,
               mut x3: f64, mut y3: f64,
               x4: f64, y4: f64) -> Segment {
    /* Fix the tangents for when the middle control points coincide with their respective endpoints */

    if double_equals (x2, x1) && double_equals (y2, y1) {
        x2 = x3;
        y2 = y3;
    }

    if double_equals (x3, x4) && double_equals (y3, y4) {
        x3 = x2;
        y3 = y2;
    }

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


pub fn path_to_segments (path: cairo::Path) -> Vec<Segment> {
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

    for cairo_segment in path.iter () {
        last_x = cur_x;
        last_y = cur_y;

        match cairo_segment {
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

/* A segment is zero length if it is degenerate, or if all four control points
 * coincide (the first and last control points may coincide, but the others may
 * define a loop - thus nonzero length)
 */
fn is_zero_length_segment (segment: Segment) -> bool {
    match segment {
        Segment::Degenerate { .. } => { true },

        Segment::LineOrCurve { x1, y1, x2, y2, x3, y3, x4, y4 } => {
            (points_equal (x1, y1, x2, y2)
             && points_equal (x1, y1, x3, y3)
             && points_equal (x1, y1, x4, y4))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate cairo;

    fn create_cr () -> cairo::Context {
        let surf = cairo::ImageSurface::create (cairo::Format::Rgb24, 1, 1);
        let cr = cairo::Context::new (&surf);

        cr
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

    fn test_path_to_segments (path: cairo::Path, expected_segments: Vec<Segment>) {
        let segments = path_to_segments (path);
        assert_eq! (expected_segments, segments);
    }

    /* Single open path; the easy case */

    fn setup_open_path () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);

        cr.copy_path ()
    }

    #[test]
    fn path_to_segments_handles_open_path () {
        let expected_segments: Vec<Segment> = vec![
            line (10.0, 10.0, 20.0, 10.0),
            line (20.0, 10.0, 20.0, 20.0)
        ];

        test_path_to_segments (setup_open_path(), expected_segments);
    }

    /* Multiple open subpaths */

    fn setup_multiple_open_subpaths () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);

        cr.move_to (30.0, 30.0);
        cr.line_to (40.0, 30.0);
        cr.curve_to (50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        cr.line_to (80.0, 90.0);

        cr.copy_path ()
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

        test_path_to_segments (setup_multiple_open_subpaths (), expected_segments);
    }

    /* Closed subpath; must have a line segment back to the first point */

    fn setup_closed_subpath () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);
        cr.close_path ();

        cr.copy_path ()
    }

    #[test]
    fn path_to_segments_handles_closed_subpath () {
        let expected_segments: Vec<Segment> = vec![
            line (10.0, 10.0, 20.0, 10.0),
            line (20.0, 10.0, 20.0, 20.0),
            line (20.0, 20.0, 10.0, 10.0)
        ];

        test_path_to_segments (setup_closed_subpath (), expected_segments);
    }

    /* Multiple closed subpaths; each must have a line segment back to their
     * initial points, with no degenerate segments between subpaths.
     */

    fn setup_multiple_closed_subpaths () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);
        cr.close_path ();

        cr.move_to (30.0, 30.0);
        cr.line_to (40.0, 30.0);
        cr.curve_to (50.0, 35.0, 60.0, 60.0, 70.0, 70.0);
        cr.line_to (80.0, 90.0);
        cr.close_path ();

        cr.copy_path ()
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

        test_path_to_segments (setup_multiple_closed_subpaths (), expected_segments);
    }

    /* A lineto follows the first closed subpath, with no moveto to start the second subpath.  The
     * lineto must start at the first point of the first subpath.
     */

    fn setup_no_moveto_after_closepath () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);
        cr.close_path ();

        cr.line_to (40.0, 30.0);

        cr.copy_path ()
    }

    #[test]
    fn path_to_segments_handles_no_moveto_after_closepath () {
        let expected_segments: Vec<Segment> = vec![
            line  (10.0, 10.0, 20.0, 10.0),
            line  (20.0, 10.0, 20.0, 20.0),
            line  (20.0, 20.0, 10.0, 10.0),

            line  (10.0, 10.0, 40.0, 30.0)
        ];

        test_path_to_segments (setup_no_moveto_after_closepath (), expected_segments);
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

    fn setup_sequence_of_moveto () -> cairo::Path {
        let cr = create_cr ();

        cr.move_to (10.0, 10.0);
        cr.move_to (20.0, 20.0);
        cr.move_to (30.0, 30.0);
        cr.move_to (40.0, 40.0);

        let path = cr.copy_path ();
        path
    }

    #[test]
    fn path_to_segments_handles_sequence_of_moveto () {
        let expected_segments: Vec<Segment> = vec! [
            degenerate (10.0, 10.0),
            degenerate (20.0, 20.0),
            degenerate (30.0, 30.0),
            degenerate (40.0, 40.0)
        ];

        test_path_to_segments (setup_sequence_of_moveto (), expected_segments);
    }
     */

    #[test]
    fn degenerate_segment_is_zero_length () {
        assert! (super::is_zero_length_segment (degenerate (1.0, 2.0)));
    }

    #[test]
    fn line_segment_is_nonzero_length () {
        assert! (!super::is_zero_length_segment (line (1.0, 2.0, 3.0, 4.0)));
    }

    #[test]
    fn line_segment_with_coincident_ends_is_zero_length () {
        assert! (super::is_zero_length_segment (line (1.0, 2.0, 1.0, 2.0)));
    }

    #[test]
    fn curves_with_loops_and_coincident_ends_are_nonzero_length () {
        assert! (!super::is_zero_length_segment (curve (1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 1.0, 2.0)));
        assert! (!super::is_zero_length_segment (curve (1.0, 2.0, 1.0, 2.0, 3.0, 4.0, 1.0, 2.0)));
        assert! (!super::is_zero_length_segment (curve (1.0, 2.0, 3.0, 4.0, 1.0, 2.0, 1.0, 2.0)));
    }

    #[test]
    fn curve_with_coincident_control_points_is_zero_length () {
        assert! (super::is_zero_length_segment (curve (1.0, 2.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0)));
    }
}
