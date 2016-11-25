extern crate cairo;
extern crate cairo_sys;

use length::*;

use self::cairo::MatrixTrait;

#[derive(Copy, Clone)]
pub struct ColorStop {
    pub offset: f64,
    pub rgba:   u32
}

/* Any of the attributes in gradient elements may be omitted.  In turn, the missing
 * ones can be inherited from the gradient referenced by its "fallback" IRI.  We
 * represent these possibly-missing attributes as Option<foo>.
 */
pub struct GradientCommon {
    pub obj_bbox: Option<bool>,
    pub affine:   Option<cairo::Matrix>,
    pub spread:   Option<cairo::enums::Extend>,
    pub fallback: Option<String>,
    pub stops:    Option<Vec<ColorStop>>
}

#[derive(Copy, Clone)]
pub enum GradientVariant {
    Linear {
        x1: Option<RsvgLength>,
        y1: Option<RsvgLength>,
        x2: Option<RsvgLength>,
        y2: Option<RsvgLength>
    },

    Radial {
        cx: Option<RsvgLength>,
        cy: Option<RsvgLength>,
        r:  Option<RsvgLength>,
        fx: Option<RsvgLength>,
        fy: Option<RsvgLength>,
    }
}

pub struct Gradient {
    pub common: GradientCommon,
    pub variant: GradientVariant
}

impl GradientCommon {
    fn new (obj_bbox: Option<bool>,
            affine:   Option<cairo::Matrix>,
            spread:   Option<cairo::enums::Extend>,
            fallback: Option<String>,
            stops:    Option<Vec<ColorStop>>) -> GradientCommon {
        GradientCommon {
            obj_bbox: obj_bbox,
            affine:   affine,
            spread:   spread,
            fallback: fallback,
            stops:    stops
        }
    }

    fn clone_stops (&self) -> Option<Vec<ColorStop>> {
        if let Some (ref stops) = self.stops {
            Some (stops.clone ())
        } else {
            None
        }
    }

    fn is_resolved (&self) -> bool {
        self.obj_bbox.is_some() && 
            self.affine.is_some () &&
            self.spread.is_some () &&
            self.stops.is_some ()
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        if self.obj_bbox.is_none () { self.obj_bbox = Some (true); }
        if self.affine.is_none ()   { self.affine   = Some (cairo::Matrix::identity ()); }
        if self.spread.is_none ()   { self.spread   = Some (cairo::enums::Extend::Pad); }

        self.fallback = None;

        if self.stops.is_none ()    { self.stops    = Some (Vec::<ColorStop>::new ()); } // empty array of color stops
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientCommon) {
        if self.obj_bbox.is_none () { self.obj_bbox = fallback.obj_bbox; }
        if self.affine.is_none ()   { self.affine   = fallback.affine; }
        if self.spread.is_none ()   { self.spread   = fallback.spread; }
        if self.stops.is_none ()    { self.stops    = fallback.clone_stops (); }

        if self.fallback.is_none () {
            self.fallback = clone_fallback_name (&fallback.fallback);
        }
    }
}

fn clone_fallback_name (fallback: &Option<String>) -> Option<String> {
    if let Some (ref fallback_name) = *fallback {
        Some (fallback_name.clone ())
    } else {
        None
    }
}

impl GradientVariant {
    fn is_resolved (&self) -> bool {
        match *self {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                x1.is_some () &&
                    y1.is_some () &&
                    x2.is_some () &&
                    y2.is_some ()
            },

            GradientVariant::Radial { cx, cy, r, fx, fy } => {
                cx.is_some () && 
                    cy.is_some () && 
                    r.is_some () && 
                    fx.is_some () && 
                    fy.is_some ()
            }
        }
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        match *self {
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                if x1.is_none () { *x1 = Some (RsvgLength::parse ("0%", LengthDir::Horizontal)); }
                if y1.is_none () { *y1 = Some (RsvgLength::parse ("0%", LengthDir::Vertical)); }
                if x2.is_none () { *x2 = Some (RsvgLength::parse ("100%", LengthDir::Horizontal)); }
                if y2.is_none () { *y2 = Some (RsvgLength::parse ("0%", LengthDir::Vertical)); }
            },

            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                if cx.is_none () { *cx = Some (RsvgLength::parse ("50%", LengthDir::Horizontal)); }
                if cy.is_none () { *cy = Some (RsvgLength::parse ("50%", LengthDir::Vertical)); }
                if r.is_none ()  { *r  = Some (RsvgLength::parse ("50%", LengthDir::Both)); }

                /* fx and fy fall back to the presentational value of cx and cy */
                if fx.is_none () { *fx = *cx }
                if fy.is_none () { *fy = *cy }
            }
        }
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientVariant) {
        match *self {
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                if let &GradientVariant::Linear { x1: x1f, y1: y1f, x2: x2f, y2: y2f } = fallback {
                    if x1.is_none () { *x1 = x1f; }
                    if y1.is_none () { *y1 = y1f; }
                    if x2.is_none () { *x2 = x2f; }
                    if y2.is_none () { *y2 = y2f; }
                }
            },

            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                if let &GradientVariant::Radial { cx: cxf, cy: cyf, r: rf, fx: fxf, fy: fyf } = fallback {
                    if cx.is_none () { *cx = cxf; }
                    if cy.is_none () { *cy = cyf; }
                    if r.is_none ()  { *r  = rf;  }
                    if fx.is_none () { *fx = fxf; }
                    if fy.is_none () { *fy = fyf; }
                }
            }
        }
    }
}

impl Gradient {
    fn new (common: GradientCommon, variant: GradientVariant) -> Gradient {
        Gradient {
            common: common,
            variant: variant
        }
    }

    fn is_resolved (&self) -> bool {
        self.common.is_resolved () && self.variant.is_resolved ()
    }

    fn resolve_from_defaults (&mut self) {
        self.common.resolve_from_defaults ();
        self.variant.resolve_from_defaults ();
    }

    fn resolve_from_fallback (&mut self, fallback: &Gradient) {
        self.common.resolve_from_fallback (&fallback.common);
        self.variant.resolve_from_fallback (&fallback.variant);
    }
}

trait FallbackSource {
    fn get_fallback (&self, name: &str) -> Option<Gradient>;
}

fn resolve_gradient (gradient: &Gradient, fallback_source: &FallbackSource) -> Gradient {
    let mut result = Gradient::new (GradientCommon::new (gradient.common.obj_bbox,
                                                         gradient.common.affine,
                                                         gradient.common.spread,
                                                         clone_fallback_name (&gradient.common.fallback),
                                                         gradient.common.clone_stops ()),
                                    gradient.variant);

    while !result.is_resolved () {
        let mut opt_fallback: Option<Gradient> = None;

        if let Some (ref fallback_name) = result.common.fallback {
            opt_fallback = fallback_source.get_fallback (&**fallback_name);
        }

        if let Some (fallback_gradient) = opt_fallback {
            result.resolve_from_fallback (&fallback_gradient);
        } else {
            result.resolve_from_defaults ();
            break;
        }
    }

    result
}
