extern crate cairo;
extern crate cairo_sys;

use length::*;

use self::cairo::MatrixTrait;

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
    spread:   Option<cairo::enums::Extend>,
    fallback: Option<String>,
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
            spread:   Some (cairo::enums::Extend::Pad),
            fallback: None,
            stops:    Some (Vec::<ColorStop>::new ())        // empty array of color stops
        }
    }

    fn is_resolved (&self) -> bool {
        self.obj_bbox.is_some() && 
            self.affine.is_some () &&
            self.spread.is_some () &&
            self.stops.is_some ()
    }
}

impl LinearGradient {
    fn get_defaults () -> LinearGradient {
        LinearGradient {
            common: GradientCommon::get_defaults (),
            x1:     Some (RsvgLength::parse ("0%", LengthDir::Horizontal)),  // these are per the spec
            y1:     Some (RsvgLength::parse ("0%", LengthDir::Vertical)),
            x2:     Some (RsvgLength::parse ("100%", LengthDir::Horizontal)),
            y2:     Some (RsvgLength::parse ("0%", LengthDir::Vertical))
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
            cx:     Some (RsvgLength::parse ("50%", LengthDir::Horizontal)),
            cy:     Some (RsvgLength::parse ("50%", LengthDir::Vertical)),
            r:      Some (RsvgLength::parse ("50%", LengthDir::Both)),
            fx:     Some (RsvgLength::parse ("50%", LengthDir::Horizontal)), // per the spec, equal to cx
            fy:     Some (RsvgLength::parse ("50%", LengthDir::Vertical))    // per the spec, equal to cy
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
