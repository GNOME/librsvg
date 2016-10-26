extern crate cairo;

pub struct Segment {
    is_degenerate: bool, /* If true, only (p1x, p1y) are valid.  If false, all are valid */
    p1x: f64, p1y: f64,
    p2x: f64, p2y: f64,
    p3x: f64, p3y: f64,
    p4x: f64, p4y: f64
}

enum SegmentState {
    Start,
    End
}

/* This converts a cairo_path_t into a list of curveto-like segments.  Each segment can be:
 *
 * 1. segment.is_degenerate = TRUE => the segment is actually a single point (segment.p1x, segment.p1y)
 *
 * 2. segment.is_degenerate = FALSE => either a lineto or a curveto (or the effective lineto that results from a closepath).
 *    We have the following points:
 *       P1 = (p1x, p1y)
 *       P2 = (p2x, p2y)
 *       P3 = (p3x, p3y)
 *       P4 = (p4x, p4y)
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
    state = SegmentState::End;

    for cairo_segment in path.iter () {
        last_x = cur_x;
        last_y = cur_y;

        match cairo_segment {
            cairo::PathSegment::MoveTo ((x, y)) => {
                cur_x = x;
                cur_y = y;

                subpath_start_x = cur_x;
                subpath_start_y = cur_y;

                let seg = Segment {
                    is_degenerate: true,
                    p1x: cur_x,
                    p1y: cur_y,
                    p2x: 0.0, p2y: 0.0, p3x: 0.0, p3y: 0.0, p4x: 0.0, p4y: 0.0 // these are set in the next iteration
                };

                segments.push (seg);

                state = SegmentState::Start;
            },

            cairo::PathSegment::LineTo ((x, y)) => {
                cur_x = x;
                cur_y = y;

                match state {
                    SegmentState::Start => {
                        segments.last_mut().expect("LineTo without a segment!").is_degenerate = false;
                        state = SegmentState::End;
                    },

                    SegmentState::End => {
                        let seg = Segment {
                            is_degenerate: false,
                            p1x: last_x,
                            p1y: last_y,
                            p2x: 0.0, p2y: 0.0, p3x: 0.0, p3y: 0.0, p4x: 0.0, p4y: 0.0  // these are set below
                        };

                        segments.push (seg);
                    }
                }

                let segment = segments.last_mut().expect("No segment after LineTo");
                segment.p2x = cur_x;
                segment.p2y = cur_y;

                segment.p3x = last_x;
                segment.p3y = last_y;

                segment.p4x = cur_x;
                segment.p4y = cur_y;
            },

            cairo::PathSegment::CurveTo ((p2x, p2y), (p3x, p3y), (p4x, p4y)) => {
                cur_x = p4x;
                cur_y = p4y;

                match state {
                    SegmentState::Start => {
                        segments.last_mut().expect("CurveTo without a segment!").is_degenerate = false;
                        state = SegmentState::End;
                    },

                    SegmentState::End => {
                        let seg = Segment {
                            is_degenerate: false,
                            p1x: last_x,
                            p1y: last_y,
                            p2x: 0.0, p2y: 0.0, p3x: 0.0, p3y: 0.0, p4x: 0.0, p4y: 0.0 // these are set below
                        };

                        segments.push (seg);
                    }
                }

                let segment = segments.last_mut().expect("No segment after CurveTo!");
                segment.p2x = p2x;
                segment.p2y = p2y;

                segment.p3x = p3x;
                segment.p3y = p3y;

                segment.p4x = cur_x;
                segment.p4y = cur_y;

                /* Fix the tangents for when the middle control points coincide with their respective endpoints */

                if double_equals (segment.p2x, segment.p1x)
                    && double_equals (segment.p2y, segment.p1y) {
                    segment.p2x = segment.p3x;
                    segment.p2y = segment.p3y;
                }

                if double_equals (segment.p3x, segment.p4x)
                    && double_equals (segment.p3y, segment.p4y) {
                    segment.p3x = segment.p2x;
                    segment.p3y = segment.p2y;
                }
            }

            cairo::PathSegment::ClosePath => {
                cur_x = subpath_start_x;
                cur_y = subpath_start_y;

                match state {
                    SegmentState::Start => {
                        let segment = segments.last_mut().expect("ClosePath without a segment!");
                        segment.is_degenerate = false;

                        segment.p2x = cur_x;
                        segment.p2y = cur_y;

                        segment.p3x = last_x;
                        segment.p3y = last_y;

                        segment.p4x = cur_x;
                        segment.p4y = cur_y;

                        state = SegmentState::End;
                    },

                    SegmentState::End => {
                        /* nothing; closepath after moveto (or a single lone closepath) does nothing */
                    }
                }
            }
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate cairo;

    fn setup_open_path () -> cairo::Path {
        let surf = cairo::ImageSurface::create (cairo::Format::Rgb24, 1, 1);
        let cr = cairo::Context::new (&surf);

        cr.move_to (10.0, 10.0);
        cr.line_to (20.0, 10.0);
        cr.line_to (20.0, 20.0);

        let path = cr.copy_path ();
        path
    }

    #[test]
    fn path_to_segments_handles_open_path () {
        let path = setup_open_path ();
        let segments = path_to_segments (path);

        for (index, seg) in segments.iter ().enumerate () {
            match index {
                0 => {
                    assert_eq! (seg.is_degenerate, false);
                    assert_eq! ((seg.p1x, seg.p1y), (10.0, 10.0));
                    assert_eq! ((seg.p2x, seg.p2y), (20.0, 10.0));
                    assert_eq! ((seg.p3x, seg.p3y), (10.0, 10.0));
                    assert_eq! ((seg.p4x, seg.p4y), (20.0, 10.0));
                },

                1 => {
                    assert_eq! (seg.is_degenerate, false);
                    assert_eq! ((seg.p1x, seg.p1y), (20.0, 10.0));
                    assert_eq! ((seg.p2x, seg.p2y), (20.0, 20.0));
                    assert_eq! ((seg.p3x, seg.p3y), (20.0, 10.0));
                    assert_eq! ((seg.p4x, seg.p4y), (20.0, 20.0));
                },

                _ => { unreachable! (); }
            }
        }
    }
}
