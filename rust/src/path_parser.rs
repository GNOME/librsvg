use path_builder::*;

extern crate cairo;


fn parse_path (path_str: &str) -> RsvgPathBuilder {
    let builder = RsvgPathBuilder::new ();

    builder
}


#[cfg(test)]
mod tests {
    use super::*;
    use path_builder::*;
    extern crate cairo;

    fn path_segment_vectors_are_equal (a: &Vec<cairo::PathSegment>,
                                       b: &Vec<cairo::PathSegment>) -> bool {
        if a.len () == 0 && b.len () == 0 {
            return true;
        }

        let mut iter = a.iter().zip (b);

        loop {
            if let Some ((seg1, seg2)) = iter.next () {
                match *seg1 {
                    cairo::PathSegment::MoveTo ((x, y)) => {
                        if let cairo::PathSegment::MoveTo ((ox, oy)) = *seg2 { return (x, y) == (ox, oy); }
                    },

                    cairo::PathSegment::LineTo ((x, y)) => {
                        if let cairo::PathSegment::LineTo ((ox, oy)) = *seg2 { return (x, y) == (ox, oy); }
                    },

                    cairo::PathSegment::CurveTo ((x2, y2), (x3, y3), (x4, y4)) => {
                        if let cairo::PathSegment::CurveTo ((ox2, oy2), (ox3, oy3), (ox4, oy4)) = *seg2 {
                            return (ox2, oy2, ox3, oy3, ox4, oy4) == (x2, y2, x3, y3, x4, y4);
                        }
                    },

                    cairo::PathSegment::ClosePath => {
                        if let cairo::PathSegment::ClosePath = *seg2 { return true; }
                    }
                }
            } else {
                return false;
            }
        }
    }

    #[test]
    fn path_parser_handles_empty_data () {
        let builder = super::parse_path ("");
        let segments = builder.get_path_segments ();
        let expected_segments = Vec::<cairo::PathSegment>::new ();

        assert! (path_segment_vectors_are_equal (&expected_segments, segments));
    }
}
