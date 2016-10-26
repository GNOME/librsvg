extern crate cairo;

pub struct Segment {
    is_degenerate: bool, /* If true, only (x1, y1) are valid.  If false, all are valid */
    x1: f64, y1: f64,
    x2: f64, y2: f64,
    x3: f64, y3: f64,
    x4: f64, y4: f64
}

enum SegmentState {
    Start,
    End
}

/* This converts a cairo_path_t into a list of curveto-like segments.  Each segment can be:
 *
 * 1. segment.is_degenerate = TRUE => the segment is actually a single point (segment.x1, segment.y1)
 *
 * 2. segment.is_degenerate = FALSE => either a lineto or a curveto (or the effective lineto that results from a closepath).
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

pub fn path_to_segments (path: cairo::Path) -> Vec<Segment> {
    let mut last_x: f64;
    let mut last_y: f64;
    let mut cur_x: f64;
    let mut cur_y: f64;
    let mut subpath_start_x: f64;
    let mut subpath_start_y: f64;
    let mut has_first_segment : bool;
    let mut segment_num : usize;
    let mut segments: Vec<Segment>;
    let mut state: SegmentState;

    cur_x = 0.0;
    cur_y = 0.0;
    subpath_start_x = 0.0;
    subpath_start_y = 0.0;

    has_first_segment = false;
    segment_num = 0;
    segments = Vec::new ();
    state = SegmentState::End;

    for cairo_segment in path.iter () {
        last_x = cur_x;
        last_y = cur_y;

        match cairo_segment {
            cairo::PathSegment::MoveTo ((x, y)) => {
                if has_first_segment {
                    segment_num += 1;
                } else {
                    has_first_segment = true;
                }

                cur_x = x;
                cur_y = y;

                subpath_start_x = cur_x;
                subpath_start_y = cur_y;

                let seg = Segment {
                    is_degenerate: true,
                    x1: cur_x,
                    y1: cur_y,
                    x2: 0.0, y2: 0.0, x3: 0.0, y3: 0.0, x4: 0.0, y4: 0.0 // these are set in the next iteration
                };

                segments.push (seg);

                state = SegmentState::Start;
            },

            cairo::PathSegment::LineTo ((x, y)) => {
                cur_x = x;
                cur_y = y;

                match state {
                    SegmentState::Start => {
                        segments[segment_num].is_degenerate = false;
                        state = SegmentState::End;
                    },

                    SegmentState::End => {
                        segment_num += 1;

                        let seg = Segment {
                            is_degenerate: false,
                            x1: last_x,
                            y1: last_y,
                            x2: 0.0, y2: 0.0, x3: 0.0, y3: 0.0, x4: 0.0, y4: 0.0  // these are set below
                        };

                        segments.push (seg);
                    }
                }

                segments[segment_num].x2 = cur_x;
                segments[segment_num].y2 = cur_y;

                segments[segment_num].x3 = last_x;
                segments[segment_num].y3 = last_y;

                segments[segment_num].x4 = cur_x;
                segments[segment_num].y4 = cur_y;
            },

            cairo::PathSegment::CurveTo ((x2, y2), (x3, y3), (x4, y4)) => {
                cur_x = x4;
                cur_y = y4;

                match state {
                    SegmentState::Start => {
                        segments[segment_num as usize].is_degenerate = false;
                        state = SegmentState::End;
                    },

                    SegmentState::End => {
                        segment_num += 1;

                        let seg = Segment {
                            is_degenerate: false,
                            x1: last_x,
                            y1: last_y,
                            x2: 0.0, y2: 0.0, x3: 0.0, y3: 0.0, x4: 0.0, y4: 0.0 // these are set below
                        };

                        segments.push (seg);
                    }
                }

                segments[segment_num].x2 = x2;
                segments[segment_num].y2 = y2;

                segments[segment_num].x3 = x3;
                segments[segment_num].y3 = y3;

                segments[segment_num].x4 = cur_x;
                segments[segment_num].y4 = cur_y;

                /* Fix the tangents for when the middle control points coincide with their respective endpoints */

                if double_equals (segments[segment_num].x2, segments[segment_num].x1)
                    && double_equals (segments[segment_num].y2, segments[segment_num].y1) {
                    segments[segment_num].x2 = segments[segment_num].x3;
                    segments[segment_num].y2 = segments[segment_num].y3;
                }

                if double_equals (segments[segment_num].x3, segments[segment_num].x4)
                    && double_equals (segments[segment_num].y3, segments[segment_num].y4) {
                    segments[segment_num].x3 = segments[segment_num].x2;
                    segments[segment_num].y3 = segments[segment_num].y2;
                }
            }

            cairo::PathSegment::ClosePath => {
                cur_x = subpath_start_x;
                cur_y = subpath_start_y;

                match state {
                    SegmentState::Start => {
                        segments[segment_num].is_degenerate = false;

                        segments[segment_num].x2 = cur_x;
                        segments[segment_num].y2 = cur_y;

                        segments[segment_num].x3 = last_x;
                        segments[segment_num].y3 = last_y;

                        segments[segment_num].x4 = cur_x;
                        segments[segment_num].y4 = cur_y;

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
                    assert_eq! ((seg.x1, seg.y1), (10.0, 10.0));
                    assert_eq! ((seg.x2, seg.y2), (20.0, 10.0));
                    assert_eq! ((seg.x3, seg.y3), (10.0, 10.0));
                    assert_eq! ((seg.x4, seg.y4), (20.0, 10.0));
                },

                1 => {
                    assert_eq! (seg.is_degenerate, false);
                    assert_eq! ((seg.x1, seg.y1), (20.0, 10.0));
                    assert_eq! ((seg.x2, seg.y2), (20.0, 20.0));
                    assert_eq! ((seg.x3, seg.y3), (20.0, 10.0));
                    assert_eq! ((seg.x4, seg.y4), (20.0, 20.0));
                },

                _ => { unreachable! (); }
            }
        }
    }
}
