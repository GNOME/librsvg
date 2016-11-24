extern crate cairo;

use length::*;

struct ColorStop {
    offset: f64,
    rgba:   u32
}

/* Any of the attributes in gradient elements may be omitted.  In turn, the missing
 * ones can be inherited from the gradient referenced by its "fallback" IRI.  We
 * represent these possibly-missing attributes as Option<foo>.
 */
struct GradientCommon {
    obj_bbox: Option<bool>,
    affine:   Option<cairo::Matrix>,
    spread:   Option<cairo::Extend>,
    fallback: Option<&str>,
    stops:    Option<Vec<ColorStop>>
}

struct LinearGradient {
    common: GradientCommon,
    x1:     Option<RsvgLength>,
    y1:     Option<RsvgLength>,
    x2:     Option<RsvgLength>,
    y2:     Option<RsvgLength>
}

struct RadialGradient {
    common: GradientCommon,
    cx:     Option<RsvgLength>,
    cy:     Option<RsvgLength>,
    r:      Option<RsvgLength>,
    fx:     Option<RsvgLength>,
    fy:     Option<RsvgLength>,
}

impl GradientCommon {
    fn get_defaults () -> GradientCommon {
        GradientCommon {
            obj_bbox: Some (true),                         // these are per the spec
            affine:   Some (cairo::Matrix::identity ()),
            spread:   Some (cairo::Extend::Pad),
            fallback: None,
            stops:    Some (Vec<ColorStop>::new ())        // empty array of color stops
        }
    }

    fn is_resolved (&self) -> bool {
        self.obj_bbox.is_some() && 
            self.affine.is_some () &&
            self.spread.is_some () &&
            self.stops.is_some ()
    }
}

fn make_length (value: f64, dir: LengthDir) -> RsvgLength {
    assert! (value >= 0.0 && value <= 1.0);

    RsvgLength {
        length: value,
        unit:   LengthUnit::Default,
        dir:    dir
    }
}

impl LinearGradient {
    fn get_defaults () -> LinearGradient {
        LinearGradient {
            common: GradientCommon::get_defaults (),
            x1:     Some (make_length (0.0, LengthDir::Horizontal)),  // these are per the spec
            y1:     Some (make_length (0.0, LengthDir::Vertical)),
            x2:     Some (make_length (1.0, LengthDir::Horizontal)),
            y2:     Some (make_length (0.0, LengthDir::Vertical))
        }
    }

    fn is_resolved (&self) -> bool {
        self.common.is_resolved () &&
            self.x1.is_some () &&
            self.y1.is_some () &&
            self.x2.is_some () &&
            self.y2.is_some ()
    }
}

impl RadialGradient {
    fn get_defaults () -> RadialGradient {
        RadialGradient {
            common: GradientCommon::get_defaults (),
            cx:     Some (make_length (0.5, LengthDir::Horizontal)),
            cy:     Some (make_length (0.5, LengthDir::Vertical)),
            r:      Some (make_length (0.5, LengthDir::Both)),
            fx:     Some (make_length (0.5, LengthDir::Horizontal)) // per the spec, equal to cx
            fy:     Some (make_length (0.5, LengthDir::Vertical))   // per the spec, equal to cy
        }
    }

    fn is_resolved (&self) -> bool {
        self.common.is_resolved () &&
            self.cx.is_some () && 
            self.cy.is_some () && 
            self.r.is_some () && 
            self.fx.is_some () && 
            self.fy.is_some ()
    }
}
